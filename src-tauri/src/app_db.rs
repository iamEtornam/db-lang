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
    pub auth_json: String,
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
    pub model: String,            // e.g. "gemini-2.5-flash", "gpt-4o", "claude-sonnet-4-20250514"
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

        let has_auth_json_col: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('connections') WHERE name='auth_json'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i32>(0)))
            .unwrap_or(0) > 0;
        if !has_auth_json_col {
            conn.execute("ALTER TABLE connections ADD COLUMN auth_json TEXT NOT NULL DEFAULT ''", []).ok();
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
                model TEXT NOT NULL DEFAULT 'gemini-2.5-flash',
                api_key TEXT NOT NULL DEFAULT '',
                api_url TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // ---- Schema Knowledge Base tables ----

        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_snapshots (
                id TEXT PRIMARY KEY,
                connection_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'generating',
                summary TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS table_descriptions (
                id TEXT PRIMARY KEY,
                snapshot_id TEXT NOT NULL,
                table_name TEXT NOT NULL,
                schema_name TEXT,
                table_type TEXT NOT NULL DEFAULT 'table',
                ai_description TEXT,
                column_metadata TEXT NOT NULL DEFAULT '[]',
                sample_data TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (snapshot_id) REFERENCES schema_snapshots(id) ON DELETE CASCADE
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS relationship_descriptions (
                id TEXT PRIMARY KEY,
                snapshot_id TEXT NOT NULL,
                source_table TEXT NOT NULL,
                source_column TEXT NOT NULL,
                target_table TEXT NOT NULL,
                target_column TEXT NOT NULL,
                relationship_type TEXT,
                ai_description TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (snapshot_id) REFERENCES schema_snapshots(id) ON DELETE CASCADE
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_snapshots_connection ON schema_snapshots(connection_id)",
            [],
        )?;

        // Create indexes
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_query_history_created_at ON query_history(created_at DESC)",
            [],
        )?;

        // Drop old tables that are no longer needed (auth-related)
        conn.execute("DROP TABLE IF EXISTS sessions", [])?;
        conn.execute("DROP TABLE IF EXISTS users", [])?;

        Ok(())
    }

    // ============ Connection operations ============

    pub fn create_connection(&self, conn_record: &DbConnectionRecord) -> Result<(), AppDbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO connections (id, name, db_type, host, port, database_name, username, password, ssl_enabled, auth_json, created_at, updated_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
                &conn_record.auth_json,
                &conn_record.created_at,
                &conn_record.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_connections(&self) -> Result<Vec<DbConnectionRecord>, AppDbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, db_type, host, port, database_name, username, password, ssl_enabled, auth_json, created_at, updated_at 
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
                    auth_json: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(connections)
    }

    pub fn update_connection(&self, conn_record: &DbConnectionRecord) -> Result<bool, AppDbError> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE connections SET name = ?1, db_type = ?2, host = ?3, port = ?4, database_name = ?5, username = ?6, password = ?7, ssl_enabled = ?8, auth_json = ?9, updated_at = ?10 WHERE id = ?11",
            rusqlite::params![
                &conn_record.name,
                &conn_record.db_type,
                &conn_record.host,
                &conn_record.port,
                &conn_record.database,
                &conn_record.username,
                &conn_record.password,
                conn_record.ssl_enabled as i32,
                &conn_record.auth_json,
                now,
                &conn_record.id,
            ],
        )?;
        Ok(rows > 0)
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

    // ============ Schema Knowledge Base operations ============

    pub fn upsert_schema_snapshot(&self, snap: &SchemaSnapshot) -> Result<(), AppDbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO schema_snapshots (id, connection_id, status, summary, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
               status = excluded.status,
               summary = excluded.summary,
               updated_at = excluded.updated_at",
            rusqlite::params![
                &snap.id, &snap.connection_id, &snap.status,
                &snap.summary, &snap.created_at, &snap.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_latest_snapshot(&self, connection_id: &str) -> Result<Option<SchemaSnapshot>, AppDbError> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT id, connection_id, status, summary, created_at, updated_at
             FROM schema_snapshots WHERE connection_id = ?1
             ORDER BY created_at DESC LIMIT 1",
            [connection_id],
            |row| Ok(SchemaSnapshot {
                id: row.get(0)?,
                connection_id: row.get(1)?,
                status: row.get(2)?,
                summary: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            }),
        ).ok();
        Ok(result)
    }

    pub fn upsert_table_description(&self, td: &TableDescriptionRecord) -> Result<(), AppDbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO table_descriptions (id, snapshot_id, table_name, schema_name, table_type, ai_description, column_metadata, sample_data, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
               ai_description = excluded.ai_description,
               column_metadata = excluded.column_metadata,
               sample_data = excluded.sample_data,
               updated_at = excluded.updated_at",
            rusqlite::params![
                &td.id, &td.snapshot_id, &td.table_name, &td.schema_name,
                &td.table_type, &td.ai_description, &td.column_metadata,
                &td.sample_data, &td.created_at, &td.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_table_description_text(&self, id: &str, description: &str) -> Result<(), AppDbError> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE table_descriptions SET ai_description = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![description, now, id],
        )?;
        Ok(())
    }

    pub fn get_table_descriptions(&self, snapshot_id: &str) -> Result<Vec<TableDescriptionRecord>, AppDbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, snapshot_id, table_name, schema_name, table_type, ai_description, column_metadata, sample_data, created_at, updated_at
             FROM table_descriptions WHERE snapshot_id = ?1 ORDER BY table_name",
        )?;
        let records = stmt.query_map([snapshot_id], |row| {
            Ok(TableDescriptionRecord {
                id: row.get(0)?,
                snapshot_id: row.get(1)?,
                table_name: row.get(2)?,
                schema_name: row.get(3)?,
                table_type: row.get(4)?,
                ai_description: row.get(5)?,
                column_metadata: row.get(6)?,
                sample_data: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(records)
    }

    pub fn upsert_relationship_description(&self, rel: &RelationshipDescriptionRecord) -> Result<(), AppDbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO relationship_descriptions
             (id, snapshot_id, source_table, source_column, target_table, target_column, relationship_type, ai_description, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                &rel.id, &rel.snapshot_id, &rel.source_table, &rel.source_column,
                &rel.target_table, &rel.target_column, &rel.relationship_type,
                &rel.ai_description, &rel.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_relationship_descriptions(&self, snapshot_id: &str) -> Result<Vec<RelationshipDescriptionRecord>, AppDbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, snapshot_id, source_table, source_column, target_table, target_column, relationship_type, ai_description, created_at
             FROM relationship_descriptions WHERE snapshot_id = ?1",
        )?;
        let records = stmt.query_map([snapshot_id], |row| {
            Ok(RelationshipDescriptionRecord {
                id: row.get(0)?,
                snapshot_id: row.get(1)?,
                source_table: row.get(2)?,
                source_column: row.get(3)?,
                target_table: row.get(4)?,
                target_column: row.get(5)?,
                relationship_type: row.get(6)?,
                ai_description: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(records)
    }

    pub fn delete_snapshot_for_connection(&self, connection_id: &str) -> Result<(), AppDbError> {
        let conn = self.conn.lock().unwrap();
        // Cascades to table_descriptions and relationship_descriptions
        conn.execute(
            "DELETE FROM schema_snapshots WHERE connection_id = ?1",
            [connection_id],
        )?;
        Ok(())
    }
}

/// Schema Snapshot model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaSnapshot {
    pub id: String,
    pub connection_id: String,
    pub status: String,
    pub summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Table Description model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDescriptionRecord {
    pub id: String,
    pub snapshot_id: String,
    pub table_name: String,
    pub schema_name: Option<String>,
    pub table_type: String,
    pub ai_description: Option<String>,
    pub column_metadata: String, // JSON
    pub sample_data: Option<String>, // JSON
    pub created_at: String,
    pub updated_at: String,
}

/// Relationship Description model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipDescriptionRecord {
    pub id: String,
    pub snapshot_id: String,
    pub source_table: String,
    pub source_column: String,
    pub target_table: String,
    pub target_column: String,
    pub relationship_type: Option<String>,
    pub ai_description: Option<String>,
    pub created_at: String,
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
