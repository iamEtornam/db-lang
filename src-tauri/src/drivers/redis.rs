use async_trait::async_trait;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use super::{ColumnInfo, DatabaseDriver, DriverError, PaginatedResult, QueryLanguage, Relationship, TableInfo};

pub struct RedisDriver {
    client: redis::Client,
    conn: tokio::sync::Mutex<redis::aio::MultiplexedConnection>,
}

/// Type-aware snapshot of a Redis key. Returned by `read_value` and exposed
/// to the frontend so the editor knows what shape to render.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisKeyView {
    pub key: String,
    /// Redis TYPE: "string" | "hash" | "list" | "set" | "zset" | "none".
    /// "none" means the key doesn't exist (or has expired).
    #[serde(rename = "type")]
    pub key_type: String,
    /// Type-shaped JSON value:
    ///   string -> JSON string
    ///   hash   -> JSON object  { field: value }
    ///   list   -> JSON array of strings
    ///   set    -> JSON array of strings (no guaranteed order)
    ///   zset   -> JSON object  { member: score }
    pub value: Value,
    /// -1  = no expiry (persistent)
    /// -2  = key does not exist
    /// >=0 = seconds remaining
    pub ttl_seconds: i64,
}

impl RedisDriver {
    pub async fn new(conn_str: &str) -> Result<Self, DriverError> {
        let client = redis::Client::open(conn_str)
            .map_err(|e| DriverError::ConnectionFailed(e.to_string()))?;
        let conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| DriverError::ConnectionFailed(e.to_string()))?;

        Ok(Self { client, conn: tokio::sync::Mutex::new(conn) })
    }

    /// Read a key with its type and current value. Routes to the right
    /// Redis command per type so non-string keys come back with meaningful
    /// content instead of the `nil` GET would return.
    pub async fn read_value(&self, key: &str) -> Result<RedisKeyView, DriverError> {
        let mut conn = self.conn.lock().await;
        let key_type: String = conn
            .key_type(key)
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

        let ttl_seconds: i64 = conn
            .ttl(key)
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

        let value: Value = match key_type.as_str() {
            "string" => {
                let s: Option<String> = conn
                    .get(key)
                    .await
                    .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
                s.map(Value::String).unwrap_or(Value::Null)
            }
            "hash" => {
                let pairs: Vec<(String, String)> = conn
                    .hgetall(key)
                    .await
                    .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
                let mut map = Map::new();
                for (k, v) in pairs {
                    map.insert(k, Value::String(v));
                }
                Value::Object(map)
            }
            "list" => {
                let items: Vec<String> = conn
                    .lrange(key, 0, -1)
                    .await
                    .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
                Value::Array(items.into_iter().map(Value::String).collect())
            }
            "set" => {
                let members: Vec<String> = conn
                    .smembers(key)
                    .await
                    .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
                Value::Array(members.into_iter().map(Value::String).collect())
            }
            "zset" => {
                // ZRANGE WITHSCORES -> Vec<(member, score)>
                let pairs: Vec<(String, f64)> = conn
                    .zrange_withscores(key, 0, -1)
                    .await
                    .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
                let mut map = Map::new();
                for (member, score) in pairs {
                    map.insert(
                        member,
                        serde_json::Number::from_f64(score)
                            .map(Value::Number)
                            .unwrap_or(Value::Null),
                    );
                }
                Value::Object(map)
            }
            "stream" => {
                // XRANGE key - +  returns Vec<(id, Vec<(field, value)>)>.
                // We surface each entry as {id, ...fields} so the preview can
                // render it tabularly. Entries are immutable in Redis — the
                // edit path is "add new, delete old", never modify-in-place.
                let result: redis::RedisResult<redis::Value> = redis::cmd("XRANGE")
                    .arg(key)
                    .arg("-")
                    .arg("+")
                    .query_async(&mut *conn)
                    .await;
                let raw = result.map_err(|e| DriverError::QueryFailed(e.to_string()))?;
                Value::Array(parse_xrange_entries(raw))
            }
            "none" => Value::Null,
            _ => Value::Null,
        };

        Ok(RedisKeyView {
            key: key.to_string(),
            key_type,
            value,
            ttl_seconds,
        })
    }

    /// Apply optional TTL after a value is written. `None` leaves the key
    /// without an expiry; `Some(secs)` sets it; secs <= 0 is treated as "no
    /// expiry" rather than "expire immediately" since we don't want a key
    /// the user just wrote to disappear before they can see it.
    async fn apply_ttl(
        conn: &mut redis::aio::MultiplexedConnection,
        key: &str,
        ttl_seconds: Option<i64>,
    ) -> Result<(), DriverError> {
        match ttl_seconds {
            Some(s) if s > 0 => {
                let _: () = conn
                    .expire(key, s)
                    .await
                    .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
            }
            _ => {
                // PERSIST is safe whether or not the key currently has a TTL.
                let _: bool = conn
                    .persist(key)
                    .await
                    .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
            }
        }
        Ok(())
    }

    pub async fn set_string(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: Option<i64>,
    ) -> Result<(), DriverError> {
        let mut conn = self.conn.lock().await;
        let _: () = conn
            .set(key, value)
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        Self::apply_ttl(&mut conn, key, ttl_seconds).await
    }

    pub async fn set_hash(
        &self,
        key: &str,
        fields: Vec<(String, String)>,
        ttl_seconds: Option<i64>,
    ) -> Result<(), DriverError> {
        let mut conn = self.conn.lock().await;
        // Delete first so the resulting hash matches the user's edit exactly
        // (no stale fields the new doc doesn't mention).
        let _: i64 = conn
            .del(key)
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        if !fields.is_empty() {
            // HSET key f1 v1 f2 v2 ...  — variadic form across redis-rs versions.
            let mut cmd = redis::cmd("HSET");
            cmd.arg(key);
            for (f, v) in &fields {
                cmd.arg(f).arg(v);
            }
            let _: () = cmd
                .query_async(&mut *conn)
                .await
                .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        }
        Self::apply_ttl(&mut conn, key, ttl_seconds).await
    }

    pub async fn set_list(
        &self,
        key: &str,
        items: Vec<String>,
        ttl_seconds: Option<i64>,
    ) -> Result<(), DriverError> {
        let mut conn = self.conn.lock().await;
        let _: i64 = conn
            .del(key)
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        if !items.is_empty() {
            // RPUSH preserves the user's order; LPUSH would reverse it.
            let _: i64 = conn
                .rpush(key, items)
                .await
                .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        }
        Self::apply_ttl(&mut conn, key, ttl_seconds).await
    }

    pub async fn set_set(
        &self,
        key: &str,
        members: Vec<String>,
        ttl_seconds: Option<i64>,
    ) -> Result<(), DriverError> {
        let mut conn = self.conn.lock().await;
        let _: i64 = conn
            .del(key)
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        if !members.is_empty() {
            let _: i64 = conn
                .sadd(key, members)
                .await
                .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        }
        Self::apply_ttl(&mut conn, key, ttl_seconds).await
    }

    pub async fn set_zset(
        &self,
        key: &str,
        members: Vec<(String, f64)>,
        ttl_seconds: Option<i64>,
    ) -> Result<(), DriverError> {
        let mut conn = self.conn.lock().await;
        let _: i64 = conn
            .del(key)
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        if !members.is_empty() {
            // ZADD takes (score, member) pairs — variadic form.
            let mut cmd = redis::cmd("ZADD");
            cmd.arg(key);
            for (member, score) in &members {
                cmd.arg(score).arg(member);
            }
            let _: () = cmd
                .query_async(&mut *conn)
                .await
                .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        }
        Self::apply_ttl(&mut conn, key, ttl_seconds).await
    }

    /// Patch a hash: HSET the changed fields, HDEL the removed fields. TTL
    /// applies only if explicitly provided — partial updates shouldn't
    /// reset a key's expiry unless the user asks.
    pub async fn hash_patch(
        &self,
        key: &str,
        set_fields: Vec<(String, String)>,
        unset_fields: Vec<String>,
        ttl_seconds: Option<i64>,
    ) -> Result<(), DriverError> {
        let mut conn = self.conn.lock().await;
        if !set_fields.is_empty() {
            let mut cmd = redis::cmd("HSET");
            cmd.arg(key);
            for (f, v) in &set_fields {
                cmd.arg(f).arg(v);
            }
            let _: () = cmd
                .query_async(&mut *conn)
                .await
                .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        }
        if !unset_fields.is_empty() {
            let _: i64 = conn
                .hdel(key, unset_fields)
                .await
                .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        }
        if ttl_seconds.is_some() {
            Self::apply_ttl(&mut conn, key, ttl_seconds).await?;
        }
        Ok(())
    }

    /// Patch a set: SADD additions, SREM removals. TTL optional.
    pub async fn set_patch(
        &self,
        key: &str,
        add: Vec<String>,
        remove: Vec<String>,
        ttl_seconds: Option<i64>,
    ) -> Result<(), DriverError> {
        let mut conn = self.conn.lock().await;
        if !add.is_empty() {
            let _: i64 = conn
                .sadd(key, add)
                .await
                .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        }
        if !remove.is_empty() {
            let _: i64 = conn
                .srem(key, remove)
                .await
                .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        }
        if ttl_seconds.is_some() {
            Self::apply_ttl(&mut conn, key, ttl_seconds).await?;
        }
        Ok(())
    }

    /// Patch a sorted set: ZADD the changed/added (member, score) pairs and
    /// ZREM the removed members. TTL optional.
    pub async fn zset_patch(
        &self,
        key: &str,
        set_members: Vec<(String, f64)>,
        remove_members: Vec<String>,
        ttl_seconds: Option<i64>,
    ) -> Result<(), DriverError> {
        let mut conn = self.conn.lock().await;
        if !set_members.is_empty() {
            let mut cmd = redis::cmd("ZADD");
            cmd.arg(key);
            for (member, score) in &set_members {
                cmd.arg(score).arg(member);
            }
            let _: () = cmd
                .query_async(&mut *conn)
                .await
                .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        }
        if !remove_members.is_empty() {
            let _: i64 = conn
                .zrem(key, remove_members)
                .await
                .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        }
        if ttl_seconds.is_some() {
            Self::apply_ttl(&mut conn, key, ttl_seconds).await?;
        }
        Ok(())
    }

    /// Patch a list by LSET-ing specific indices in a single MULTI / EXEC
    /// transaction. Caller must ensure list length is unchanged — frontend
    /// falls back to full DEL+RPUSH replace when the length differs since
    /// LSET can't grow or shrink a list.
    pub async fn list_set_indices(
        &self,
        key: &str,
        changes: Vec<(i64, String)>,
        ttl_seconds: Option<i64>,
    ) -> Result<(), DriverError> {
        if changes.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().await;
        let mut pipe = redis::pipe();
        pipe.atomic();  // MULTI / EXEC wrapper
        for (idx, val) in &changes {
            pipe.cmd("LSET").arg(key).arg(*idx).arg(val).ignore();
        }
        let _: () = pipe
            .query_async(&mut *conn)
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        if ttl_seconds.is_some() {
            Self::apply_ttl(&mut conn, key, ttl_seconds).await?;
        }
        Ok(())
    }

    /// DEL N keys in a single command. Returns the number of keys that
    /// actually existed and were deleted (so trying to bulk-delete a key
    /// that's already gone doesn't inflate the count).
    pub async fn delete_keys(&self, keys: &[String]) -> Result<u64, DriverError> {
        if keys.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn.lock().await;
        let removed: i64 = conn
            .del(keys)
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        Ok(removed.max(0) as u64)
    }

    /// XADD a new entry to a stream. `entry_id` of `*` (or empty) lets Redis
    /// generate the timestamp-based ID. Returns the assigned entry ID. TTL
    /// optional — when None the stream's existing expiry (if any) is left
    /// alone.
    pub async fn stream_add(
        &self,
        key: &str,
        entry_id: &str,
        fields: Vec<(String, String)>,
        ttl_seconds: Option<i64>,
    ) -> Result<String, DriverError> {
        if fields.is_empty() {
            return Err(DriverError::QueryFailed(
                "XADD requires at least one field/value pair".into(),
            ));
        }
        let mut conn = self.conn.lock().await;
        let mut cmd = redis::cmd("XADD");
        cmd.arg(key);
        cmd.arg(if entry_id.is_empty() { "*" } else { entry_id });
        for (f, v) in &fields {
            cmd.arg(f).arg(v);
        }
        let id: String = cmd
            .query_async(&mut *conn)
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        if ttl_seconds.is_some() {
            Self::apply_ttl(&mut conn, key, ttl_seconds).await?;
        }
        Ok(id)
    }

    /// XDEL one or more entries by their IDs. Returns the number of entries
    /// actually removed (Redis ignores IDs that don't exist).
    pub async fn stream_delete_entries(
        &self,
        key: &str,
        entry_ids: Vec<String>,
    ) -> Result<u64, DriverError> {
        if entry_ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn.lock().await;
        let mut cmd = redis::cmd("XDEL");
        cmd.arg(key);
        for id in &entry_ids {
            cmd.arg(id);
        }
        let removed: i64 = cmd
            .query_async(&mut *conn)
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        Ok(removed.max(0) as u64)
    }

    pub async fn delete_key(&self, key: &str) -> Result<u64, DriverError> {
        let mut conn = self.conn.lock().await;
        let removed: i64 = conn
            .del(key)
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        Ok(removed.max(0) as u64)
    }

    async fn execute_redis_command(&self, command: &str) -> Result<Value, DriverError> {
        let mut conn = self.conn.lock().await;
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(DriverError::QueryFailed("Empty command".to_string()));
        }

        let cmd_name = parts[0].to_uppercase();
        let args = &parts[1..];

        let result: redis::RedisResult<redis::Value> = match cmd_name.as_str() {
            "GET" => {
                let val: redis::RedisResult<Option<String>> = conn.get(args[0]).await;
                val.map(|v| redis::Value::BulkString(v.unwrap_or_default().into_bytes()))
            }
            "SET" => {
                let val: redis::RedisResult<()> = conn.set(args[0], args.get(1).copied().unwrap_or("")).await;
                val.map(|_| redis::Value::SimpleString("OK".to_string()))
            }
            "KEYS" => {
                let pattern = args.get(0).copied().unwrap_or("*");
                let val: redis::RedisResult<Vec<String>> = conn.keys(pattern).await;
                val.map(|v| redis::Value::Array(v.into_iter().map(|s| redis::Value::BulkString(s.into_bytes())).collect()))
            }
            "HGETALL" => {
                let val: redis::RedisResult<Vec<(String, String)>> = conn.hgetall(args[0]).await;
                val.map(|v| {
                    let mut pairs = Vec::new();
                    for (k, vv) in v {
                        pairs.push(redis::Value::BulkString(k.into_bytes()));
                        pairs.push(redis::Value::BulkString(vv.into_bytes()));
                    }
                    redis::Value::Array(pairs)
                })
            }
            "LRANGE" => {
                let start = args.get(1).and_then(|s| s.parse::<isize>().ok()).unwrap_or(0);
                let stop = args.get(2).and_then(|s| s.parse::<isize>().ok()).unwrap_or(-1);
                let val: redis::RedisResult<Vec<String>> = conn.lrange(args[0], start, stop).await;
                val.map(|v| redis::Value::Array(v.into_iter().map(|s| redis::Value::BulkString(s.into_bytes())).collect()))
            }
            "SMEMBERS" => {
                let val: redis::RedisResult<Vec<String>> = conn.smembers(args[0]).await;
                val.map(|v| redis::Value::Array(v.into_iter().map(|s| redis::Value::BulkString(s.into_bytes())).collect()))
            }
            "TTL" => {
                let val: redis::RedisResult<i64> = conn.ttl(args[0]).await;
                val.map(|v| redis::Value::Int(v))
            }
            "TYPE" => {
                let val: redis::RedisResult<String> = conn.key_type(args[0]).await;
                val.map(|v| redis::Value::SimpleString(v))
            }
            "DBSIZE" => {
                let val: redis::RedisResult<redis::Value> = redis::cmd("DBSIZE").query_async(&mut *conn).await;
                val
            }
            "INFO" => {
                let val: redis::RedisResult<redis::Value> = redis::cmd("INFO").query_async(&mut *conn).await;
                val
            }
            _ => {
                return Err(DriverError::QueryFailed(format!("Unsupported Redis command: {}", cmd_name)))
            }
        };

        redis_value_to_json(result.map_err(|e| DriverError::QueryFailed(e.to_string()))?)
    }
}

/// XRANGE returns a `Value::Array` of `Value::Array(entry_id, field_value_array)`.
/// Flatten into `[{id, field1, value1, field2, value2, ...}]` so the preview
/// table can render entries as ordinary rows.
fn parse_xrange_entries(raw: redis::Value) -> Vec<Value> {
    let outer = match raw {
        redis::Value::Array(items) => items,
        // Empty stream returns Nil in some redis-rs versions; treat as no rows.
        _ => return Vec::new(),
    };
    let mut rows = Vec::with_capacity(outer.len());
    for entry in outer {
        let parts = match entry {
            redis::Value::Array(p) => p,
            _ => continue,
        };
        if parts.len() < 2 { continue; }
        let id = match &parts[0] {
            redis::Value::BulkString(b) => String::from_utf8_lossy(b).into_owned(),
            redis::Value::SimpleString(s) => s.clone(),
            _ => continue,
        };
        let fields_raw = match &parts[1] {
            redis::Value::Array(f) => f,
            _ => continue,
        };
        let mut row = Map::new();
        row.insert("id".to_string(), Value::String(id));
        // Field/value pairs are interleaved: [f1, v1, f2, v2, ...].
        let mut i = 0;
        while i + 1 < fields_raw.len() {
            let field = match &fields_raw[i] {
                redis::Value::BulkString(b) => String::from_utf8_lossy(b).into_owned(),
                redis::Value::SimpleString(s) => s.clone(),
                _ => { i += 2; continue; }
            };
            let value = match &fields_raw[i + 1] {
                redis::Value::BulkString(b) => Value::String(String::from_utf8_lossy(b).into_owned()),
                redis::Value::SimpleString(s) => Value::String(s.clone()),
                redis::Value::Int(n) => Value::Number((*n).into()),
                redis::Value::Nil => Value::Null,
                _ => Value::Null,
            };
            row.insert(field, value);
            i += 2;
        }
        rows.push(Value::Object(row));
    }
    rows
}

fn redis_value_to_json(val: redis::Value) -> Result<Value, DriverError> {
    match val {
        redis::Value::Nil => Ok(Value::Null),
        redis::Value::Int(i) => Ok(json!(i)),
        redis::Value::BulkString(b) => Ok(Value::String(String::from_utf8_lossy(&b).to_string())),
        redis::Value::Array(arr) => {
            let values: Result<Vec<Value>, _> = arr.into_iter().map(redis_value_to_json).collect();
            Ok(Value::Array(values?))
        }
        redis::Value::SimpleString(s) => Ok(Value::String(s)),
        redis::Value::Boolean(b) => Ok(Value::Bool(b)),
        redis::Value::Double(f) => Ok(json!(f)),
        redis::Value::BigNumber(n) => Ok(Value::String(n.to_string())),
        redis::Value::Map(pairs) => {
            let mut map = serde_json::Map::new();
            for (k, v) in pairs {
                let key = match redis_value_to_json(k)? {
                    Value::String(s) => s,
                    v => v.to_string(),
                };
                map.insert(key, redis_value_to_json(v)?);
            }
            Ok(Value::Object(map))
        }
        redis::Value::Set(members) => {
            let values: Result<Vec<Value>, _> = members.into_iter().map(redis_value_to_json).collect();
            Ok(Value::Array(values?))
        }
        redis::Value::Okay => Ok(Value::String("OK".to_string())),
        _ => Ok(Value::Null),
    }
}

#[async_trait]
impl DatabaseDriver for RedisDriver {
    async fn execute_query(&self, query: &str) -> Result<Vec<Value>, DriverError> {
        let commands: Vec<&str> = query.lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();

        let mut results = Vec::new();
        for cmd in commands {
            let result = self.execute_redis_command(cmd).await?;
            results.push(json!({
                "command": cmd,
                "result": result
            }));
        }
        Ok(results)
    }

    async fn execute_query_paginated(
        &self,
        query: &str,
        page: i32,
        page_size: i32,
    ) -> Result<PaginatedResult, DriverError> {
        let rows = self.execute_query(query).await?;
        let total = rows.len() as i64;
        let data = serde_json::to_string(&rows)
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

        Ok(PaginatedResult {
            data,
            total_count: Some(total),
            page,
            page_size,
            has_more: false,
        })
    }

    async fn test_connection(&self) -> Result<bool, DriverError> {
        let mut conn = self.conn.lock().await;
        let _: () = redis::cmd("PING")
            .query_async(&mut *conn)
            .await
            .map_err(|e| DriverError::ConnectionFailed(e.to_string()))?;
        Ok(true)
    }

    async fn get_tables(&self) -> Result<Vec<TableInfo>, DriverError> {
        let result = self.execute_redis_command("KEYS *").await?;
        let keys: Vec<String> = match result {
            Value::Array(arr) => arr.into_iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect(),
            _ => vec![],
        };

        // Group by prefix (first segment before :)
        let mut prefixes: HashMap<String, usize> = HashMap::new();
        for key in &keys {
            let prefix = key.split(':').next().unwrap_or(key).to_string();
            *prefixes.entry(prefix).or_insert(0) += 1;
        }

        let mut tables: Vec<TableInfo> = prefixes.into_iter().map(|(prefix, count)| TableInfo {
            name: prefix,
            schema: None,
            table_type: format!("KEY_GROUP ({} keys)", count),
        }).collect();
        tables.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tables)
    }

    async fn get_table_columns(&self, _table: &str, _schema: Option<&str>) -> Result<Vec<ColumnInfo>, DriverError> {
        // Synthetic columns for the schema-page row view.
        Ok(vec![
            ColumnInfo {
                name: "key".to_string(),
                data_type: "string".to_string(),
                is_nullable: false,
                column_default: None,
                is_primary_key: true,
                is_foreign_key: false,
                referenced_table: None,
                referenced_column: None,
            },
            ColumnInfo {
                name: "type".to_string(),
                data_type: "string".to_string(),
                is_nullable: false,
                column_default: None,
                is_primary_key: false,
                is_foreign_key: false,
                referenced_table: None,
                referenced_column: None,
            },
            ColumnInfo {
                name: "value".to_string(),
                data_type: "mixed".to_string(),
                is_nullable: true,
                column_default: None,
                is_primary_key: false,
                is_foreign_key: false,
                referenced_table: None,
                referenced_column: None,
            },
            ColumnInfo {
                name: "ttl".to_string(),
                data_type: "integer".to_string(),
                is_nullable: false,
                column_default: None,
                is_primary_key: false,
                is_foreign_key: false,
                referenced_table: None,
                referenced_column: None,
            },
        ])
    }

    async fn get_relationships(&self) -> Result<Vec<Relationship>, DriverError> {
        Ok(vec![])
    }

    async fn preview_table_data(&self, table: &str, _schema: Option<&str>, limit: i32) -> Result<Vec<Value>, DriverError> {
        // Match both `{prefix}:*` (real Redis convention) and the bare prefix
        // itself (so users who don't use the `prefix:rest` convention still
        // see their keys grouped). Bare-key matches are deduped.
        let mut seen = std::collections::HashSet::new();
        let mut keys: Vec<String> = Vec::new();

        for pattern in [format!("{}:*", table), table.to_string()] {
            let result = self.execute_redis_command(&format!("KEYS {}", pattern)).await?;
            if let Value::Array(arr) = result {
                for v in arr {
                    if let Some(s) = v.as_str() {
                        if seen.insert(s.to_string()) {
                            keys.push(s.to_string());
                        }
                    }
                }
            }
            if keys.len() >= limit as usize { break; }
        }

        let mut rows = Vec::new();
        for key in keys.iter().take(limit as usize) {
            // Type-aware read — non-string types come back with real content
            // instead of the `nil` the old GET-everything path returned.
            match self.read_value(key).await {
                Ok(view) => {
                    rows.push(json!({
                        "key": view.key,
                        "type": view.key_type,
                        "value": view.value,
                        "ttl": view.ttl_seconds,
                    }));
                }
                Err(_) => {
                    // Skip keys we can't read (e.g. concurrent deletion) rather
                    // than failing the whole preview.
                    continue;
                }
            }
        }
        Ok(rows)
    }

    fn engine_name(&self) -> &str {
        "redis"
    }

    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::Redis
    }
}
