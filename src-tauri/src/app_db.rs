use rusqlite::{Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::command;
use thiserror::Error;

/// Default local user ID (no auth system)
pub const LOCAL_USER_ID: &str = "local_user";

#[derive(Error, Debug)]
pub enum AppDbError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Connection not found")]
    ConnectionNotFound,
    #[error("Query not found")]
    QueryNotFound,
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

impl From<rusqlite::Error> for AppDbError {
    fn from(err: rusqlite::Error) -> Self {
        AppDbError::DatabaseError(err.to_string())
    }
}

/// Database connection model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConnectionRecord {
    pub id: String,
    pub name: String,
    pub db_type: String,
    pub host: String,
    pub port: String,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ssl_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Query history model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryHistory {
    pub id: String,
    pub connection_id: String,
    pub natural_query: String,
    pub sql_query: String,
    pub result_count: Option<i32>,
    pub execution_time_ms: Option<i32>,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
}

/// Saved snippet model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub natural_query: String,
    pub sql_query: String,
    pub tags: String,
    pub created_at: String,
    pub updated_at: String,
}

/// User settings model (no auth - single local user)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    pub theme: String,
    pub default_page_size: i32,
    pub query_timeout_seconds: i32,
    pub auto_save_history: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// LLM provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,         // "gemini", "openai", "anthropic", "ollama", "custom"
    pub model: String,            // e.g. "gemini-pro", "gpt-4", "claude-3-sonnet", "llama3"
    pub api_key: String,          // the user's own API key
    pub api_url: Option<String>,  // custom endpoint URL (for Ollama, custom providers, etc.)
    pub created_at: String,
    pub updated_at: String,
}

pub struct AppDatabase {
    conn: Mutex<Connection>,
}

impl AppDatabase {
    pub fn new(app_data_dir: PathBuf) -> Result<Self, AppDbError> {
        std::fs::create_dir_all(&app_data_dir).map_err(|e| {
            AppDbError::DatabaseError(format!("Failed to create app data directory: {}", e))
        })?;

        let db_path = app_data_dir.join("query_studio.db");
        let conn = Connection::open(&db_path)?;

        let db = AppDatabase {
            conn: Mutex::new(conn),
        };
        db.run_migrations()?;
        Ok(db)
    }

    fn run_migrations(&self) -> Result<(), AppDbError> {
        let conn = self.conn.lock().unwrap();

        // Database connections table (no user_id)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS connections (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                db_type TEXT NOT NULL,
                host TEXT NOT NULL,
                port TEXT NOT NULL,
                database_name TEXT NOT NULL DEFAULT '',
                username TEXT NOT NULL,
                password TEXT NOT NULL,
                ssl_enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // Migration: add database_name column if it doesn't exist
        let has_db_col: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('connections') WHERE name='database_name'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i32>(0)))
            .unwrap_or(0) > 0;
        if !has_db_col {
            conn.execute("ALTER TABLE connections ADD COLUMN database_name TEXT NOT NULL DEFAULT ''", []).ok();
        }

        // Query history table (no user_id)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS query_history (
                id TEXT PRIMARY KEY,
                connection_id TEXT NOT NULL,
                natural_query TEXT NOT NULL,
                sql_query TEXT NOT NULL,
                result_count INTEGER,
                execution_time_ms INTEGER,
                status TEXT NOT NULL,
                error_message TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        // Snippets table (no user_id)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS snippets (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                natural_query TEXT NOT NULL,
                sql_query TEXT NOT NULL,
                tags TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // User settings table (single row, no user_id foreign key)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS user_settings (
                id TEXT PRIMARY KEY DEFAULT 'local',
                theme TEXT NOT NULL DEFAULT 'dark',
                default_page_size INTEGER NOT NULL DEFAULT 50,
                query_timeout_seconds INTEGER NOT NULL DEFAULT 30,
                auto_save_history INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // LLM configuration table (single row)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS llm_config (
                id TEXT PRIMARY KEY DEFAULT 'local',
                provider TEXT NOT NULL DEFAULT 'gemini',
                model TEXT NOT NULL DEFAULT 'gemini-pro',
                api_key TEXT NOT NULL DEFAULT '',
                api_url TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // Create indexes
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_query_history_created_at ON query_history(created_at DESC)",
            [],
        )?;

        // Drop old tables that are no longer needed (auth-related)
        // We use IF EXISTS so this is safe to run even if they don't exist
        conn.execute("DROP TABLE IF EXISTS sessions", [])?;
        conn.execute("DROP TABLE IF EXISTS users", [])?;

        Ok(())
    }

    // ============ Connection operations ============

    pub fn create_connection(&self, conn_record: &DbConnectionRecord) -> Result<(), AppDbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO connections (id, name, db_type, host, port, database_name, username, password, ssl_enabled, created_at, updated_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                &conn_record.id,
                &conn_record.name,
                &conn_record.db_type,
                &conn_record.host,
                &conn_record.port,
                &conn_record.database,
                &conn_record.username,
                &conn_record.password,
                conn_record.ssl_enabled as i32,
                &conn_record.created_at,
                &conn_record.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_connections(&self) -> Result<Vec<DbConnectionRecord>, AppDbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, db_type, host, port, database_name, username, password, ssl_enabled, created_at, updated_at 
             FROM connections ORDER BY created_at DESC",
        )?;

        let connections = stmt
            .query_map([], |row| {
                Ok(DbConnectionRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    db_type: row.get(2)?,
                    host: row.get(3)?,
                    port: row.get(4)?,
                    database: row.get(5)?,
                    username: row.get(6)?,
                    password: row.get(7)?,
                    ssl_enabled: row.get::<_, i32>(8)? == 1,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(connections)
    }

    pub fn delete_connection(&self, id: &str) -> Result<bool, AppDbError> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(
            "DELETE FROM connections WHERE id = ?1",
            [id],
        )?;
        Ok(rows_affected > 0)
    }

    // ============ Query history operations ============

    pub fn add_query_history(&self, history: &QueryHistory) -> Result<(), AppDbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO query_history (id, connection_id, natural_query, sql_query, result_count, execution_time_ms, status, error_message, created_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                &history.id,
                &history.connection_id,
                &history.natural_query,
                &history.sql_query,
                &history.result_count,
                &history.execution_time_ms,
                &history.status,
                &history.error_message,
                &history.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_query_history(&self, limit: i32, offset: i32) -> Result<Vec<QueryHistory>, AppDbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, connection_id, natural_query, sql_query, result_count, execution_time_ms, status, error_message, created_at 
             FROM query_history ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
        )?;

        let history = stmt
            .query_map(rusqlite::params![limit, offset], |row| {
                Ok(QueryHistory {
                    id: row.get(0)?,
                    connection_id: row.get(1)?,
                    natural_query: row.get(2)?,
                    sql_query: row.get(3)?,
                    result_count: row.get(4)?,
                    execution_time_ms: row.get(5)?,
                    status: row.get(6)?,
                    error_message: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(history)
    }

    pub fn search_query_history(&self, search_term: &str, limit: i32) -> Result<Vec<QueryHistory>, AppDbError> {
        let conn = self.conn.lock().unwrap();
        let search_pattern = format!("%{}%", search_term);
        let mut stmt = conn.prepare(
            "SELECT id, connection_id, natural_query, sql_query, result_count, execution_time_ms, status, error_message, created_at 
             FROM query_history 
             WHERE natural_query LIKE ?1 OR sql_query LIKE ?1
             ORDER BY created_at DESC LIMIT ?2",
        )?;

        let history = stmt
            .query_map(rusqlite::params![search_pattern, limit], |row| {
                Ok(QueryHistory {
                    id: row.get(0)?,
                    connection_id: row.get(1)?,
                    natural_query: row.get(2)?,
                    sql_query: row.get(3)?,
                    result_count: row.get(4)?,
                    execution_time_ms: row.get(5)?,
                    status: row.get(6)?,
                    error_message: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(history)
    }

    pub fn clear_old_history(&self, days_to_keep: i32) -> Result<i32, AppDbError> {
        let conn = self.conn.lock().unwrap();
        let cutoff_date = chrono::Utc::now() - chrono::Duration::days(days_to_keep as i64);
        let cutoff_str = cutoff_date.to_rfc3339();
        
        let rows_deleted = conn.execute(
            "DELETE FROM query_history WHERE created_at < ?1",
            rusqlite::params![cutoff_str],
        )?;
        
        Ok(rows_deleted as i32)
    }

    // ============ Snippet operations ============

    pub fn create_snippet(&self, snippet: &Snippet) -> Result<(), AppDbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO snippets (id, name, description, natural_query, sql_query, tags, created_at, updated_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                &snippet.id,
                &snippet.name,
                &snippet.description,
                &snippet.natural_query,
                &snippet.sql_query,
                &snippet.tags,
                &snippet.created_at,
                &snippet.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_snippets(&self) -> Result<Vec<Snippet>, AppDbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, natural_query, sql_query, tags, created_at, updated_at 
             FROM snippets ORDER BY updated_at DESC",
        )?;

        let snippets = stmt
            .query_map([], |row| {
                Ok(Snippet {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    natural_query: row.get(3)?,
                    sql_query: row.get(4)?,
                    tags: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(snippets)
    }

    pub fn update_snippet(&self, snippet: &Snippet) -> Result<bool, AppDbError> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(
            "UPDATE snippets SET name = ?1, description = ?2, natural_query = ?3, sql_query = ?4, tags = ?5, updated_at = ?6 
             WHERE id = ?7",
            rusqlite::params![
                &snippet.name,
                &snippet.description,
                &snippet.natural_query,
                &snippet.sql_query,
                &snippet.tags,
                &snippet.updated_at,
                &snippet.id,
            ],
        )?;
        Ok(rows_affected > 0)
    }

    pub fn delete_snippet(&self, id: &str) -> Result<bool, AppDbError> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(
            "DELETE FROM snippets WHERE id = ?1",
            [id],
        )?;
        Ok(rows_affected > 0)
    }

    // ============ User settings operations ============

    pub fn get_user_settings(&self) -> Result<Option<UserSettings>, AppDbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT theme, default_page_size, query_timeout_seconds, auto_save_history, created_at, updated_at 
             FROM user_settings WHERE id = 'local'",
        )?;

        let settings = stmt
            .query_row([], |row| {
                Ok(UserSettings {
                    theme: row.get(0)?,
                    default_page_size: row.get(1)?,
                    query_timeout_seconds: row.get(2)?,
                    auto_save_history: row.get::<_, i32>(3)? == 1,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .ok();

        Ok(settings)
    }

    pub fn upsert_user_settings(&self, settings: &UserSettings) -> Result<(), AppDbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO user_settings (id, theme, default_page_size, query_timeout_seconds, auto_save_history, created_at, updated_at) 
             VALUES ('local', ?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET 
                theme = excluded.theme,
                default_page_size = excluded.default_page_size,
                query_timeout_seconds = excluded.query_timeout_seconds,
                auto_save_history = excluded.auto_save_history,
                updated_at = excluded.updated_at",
            rusqlite::params![
                &settings.theme,
                settings.default_page_size,
                settings.query_timeout_seconds,
                settings.auto_save_history as i32,
                &settings.created_at,
                &settings.updated_at,
            ],
        )?;
        Ok(())
    }

    // ============ LLM config operations ============

    pub fn get_llm_config(&self) -> Result<Option<LlmConfig>, AppDbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT provider, model, api_key, api_url, created_at, updated_at 
             FROM llm_config WHERE id = 'local'",
        )?;

        let config = stmt
            .query_row([], |row| {
                Ok(LlmConfig {
                    provider: row.get(0)?,
                    model: row.get(1)?,
                    api_key: row.get(2)?,
                    api_url: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .ok();

        Ok(config)
    }

    pub fn upsert_llm_config(&self, config: &LlmConfig) -> Result<(), AppDbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO llm_config (id, provider, model, api_key, api_url, created_at, updated_at) 
             VALUES ('local', ?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET 
                provider = excluded.provider,
                model = excluded.model,
                api_key = excluded.api_key,
                api_url = excluded.api_url,
                updated_at = excluded.updated_at",
            rusqlite::params![
                &config.provider,
                &config.model,
                &config.api_key,
                &config.api_url,
                &config.created_at,
                &config.updated_at,
            ],
        )?;
        Ok(())
    }
}

// Global database instance
static APP_DB: std::sync::OnceLock<AppDatabase> = std::sync::OnceLock::new();

pub fn init_app_database(app_data_dir: PathBuf) -> Result<(), AppDbError> {
    let db = AppDatabase::new(app_data_dir)?;
    APP_DB.set(db).map_err(|_| AppDbError::DatabaseError("Database already initialized".to_string()))?;
    Ok(())
}

pub fn get_app_database() -> Result<&'static AppDatabase, AppDbError> {
    APP_DB.get().ok_or(AppDbError::DatabaseError("Database not initialized".to_string()))
}
