use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use serde::Serialize;
use tauri::command;

// ============ Query Result Cache ============

/// Simple query result cache for improving performance
pub struct QueryCache {
    cache: Mutex<HashMap<String, CacheEntry>>,
    max_entries: usize,
    max_age: Duration,
}

struct CacheEntry {
    result: String,
    created_at: Instant,
    hits: usize,
}

impl QueryCache {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            max_entries: 100,
            max_age: Duration::from_secs(60), // 1 minute cache
        }
    }

    /// Generate cache key from query and connection
    fn cache_key(conn_str: &str, query: &str) -> String {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        conn_str.hash(&mut hasher);
        query.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Get cached result if available and not expired
    pub fn get(&self, conn_str: &str, query: &str) -> Option<String> {
        let key = Self::cache_key(conn_str, query);
        
        let mut cache = self.cache.lock().unwrap();
        if let Some(entry) = cache.get_mut(&key) {
            if entry.created_at.elapsed() < self.max_age {
                entry.hits += 1;
                return Some(entry.result.clone());
            } else {
                cache.remove(&key);
            }
        }
        None
    }

    /// Store result in cache
    pub fn set(&self, conn_str: &str, query: &str, result: String) {
        let key = Self::cache_key(conn_str, query);
        
        let mut cache = self.cache.lock().unwrap();
        
        if cache.len() >= self.max_entries {
            let oldest_key = cache.iter()
                .min_by_key(|(_, e)| e.created_at)
                .map(|(k, _)| k.clone());
            
            if let Some(k) = oldest_key {
                cache.remove(&k);
            }
        }
        
        cache.insert(key, CacheEntry {
            result,
            created_at: Instant::now(),
            hits: 0,
        });
    }

    /// Clear all cached entries
    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }

    /// Remove expired entries
    pub fn cleanup(&self) {
        let mut cache = self.cache.lock().unwrap();
        let max_age = self.max_age;
        cache.retain(|_, entry| entry.created_at.elapsed() < max_age);
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStats {
        let cache = self.cache.lock().unwrap();
        let total_hits: usize = cache.values().map(|e| e.hits).sum();
        
        CacheStats {
            entries: cache.len(),
            total_hits,
            max_entries: self.max_entries,
            max_age_secs: self.max_age.as_secs(),
        }
    }

    /// Invalidate cache for a specific connection
    pub fn invalidate_connection(&self, conn_str: &str) {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        conn_str.hash(&mut hasher);
        let conn_hash = format!("{:x}", hasher.finish());
        
        let mut cache = self.cache.lock().unwrap();
        cache.retain(|key, _| !key.starts_with(&conn_hash[..8]));
    }
}

// ============ MySQL Connection Pool ============

struct MysqlPoolEntry {
    pool: mysql_async::Pool,
    created_at: Instant,
    last_used: Instant,
}

pub struct MysqlConnectionPool {
    pools: Mutex<HashMap<String, MysqlPoolEntry>>,
    max_pools: usize,
    pool_idle_timeout: Duration,
}

impl MysqlConnectionPool {
    pub fn new() -> Self {
        Self {
            pools: Mutex::new(HashMap::new()),
            max_pools: 10,
            pool_idle_timeout: Duration::from_secs(300), // 5 minutes
        }
    }

    pub fn get_or_create(&self, conn_str: &str) -> Result<mysql_async::Pool, String> {
        let mut pools = self.pools.lock().unwrap();
        
        // Cleanup old pools first
        let now = Instant::now();
        let timeout = self.pool_idle_timeout;
        pools.retain(|_, entry| now.duration_since(entry.last_used) < timeout);
        
        // Check if pool exists
        if let Some(entry) = pools.get_mut(conn_str) {
            entry.last_used = Instant::now();
            return Ok(entry.pool.clone());
        }
        
        // Create new pool if under limit
        if pools.len() >= self.max_pools {
            // Remove least recently used pool
            let lru_key = pools.iter()
                .min_by_key(|(_, e)| e.last_used)
                .map(|(k, _)| k.clone());
            
            if let Some(k) = lru_key {
                pools.remove(&k);
            }
        }
        
        // Create new pool
        let opts = mysql_async::Opts::from_url(conn_str)
            .map_err(|e| format!("Invalid MySQL connection string: {}", e))?;
        
        let pool = mysql_async::Pool::new(opts);
        
        pools.insert(conn_str.to_string(), MysqlPoolEntry {
            pool: pool.clone(),
            created_at: Instant::now(),
            last_used: Instant::now(),
        });
        
        Ok(pool)
    }

    pub fn remove(&self, conn_str: &str) {
        let mut pools = self.pools.lock().unwrap();
        pools.remove(conn_str);
    }

    pub fn clear(&self) {
        let mut pools = self.pools.lock().unwrap();
        pools.clear();
    }

    pub fn stats(&self) -> PoolStats {
        let pools = self.pools.lock().unwrap();
        PoolStats {
            active_pools: pools.len(),
            max_pools: self.max_pools,
            pool_idle_timeout_secs: self.pool_idle_timeout.as_secs(),
        }
    }
}

// ============ Stats ============

#[derive(Serialize)]
pub struct CacheStats {
    pub entries: usize,
    pub total_hits: usize,
    pub max_entries: usize,
    pub max_age_secs: u64,
}

#[derive(Serialize)]
pub struct PoolStats {
    pub active_pools: usize,
    pub max_pools: usize,
    pub pool_idle_timeout_secs: u64,
}

#[derive(Serialize)]
pub struct ConnectionPoolStats {
    pub cache: CacheStats,
    pub mysql_pools: PoolStats,
}

// ============ Global Instances ============

static QUERY_CACHE: OnceLock<QueryCache> = OnceLock::new();
static MYSQL_POOL: OnceLock<MysqlConnectionPool> = OnceLock::new();

pub fn get_cache() -> &'static QueryCache {
    QUERY_CACHE.get_or_init(|| QueryCache::new())
}

pub fn get_mysql_pool() -> &'static MysqlConnectionPool {
    MYSQL_POOL.get_or_init(|| MysqlConnectionPool::new())
}

// ============ Tauri Commands ============

#[command]
pub fn get_cache_stats() -> CacheStats {
    get_cache().get_stats()
}

#[command]
pub fn get_pool_stats() -> ConnectionPoolStats {
    ConnectionPoolStats {
        cache: get_cache().get_stats(),
        mysql_pools: get_mysql_pool().stats(),
    }
}

#[command]
pub fn clear_query_cache() {
    get_cache().clear();
}

#[command]
pub fn clear_connection_pools() {
    get_mysql_pool().clear();
    get_cache().clear();
}

#[command]
pub fn cleanup_cache() {
    get_cache().cleanup();
}
