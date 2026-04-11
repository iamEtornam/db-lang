use crate::app_db::{
    get_app_database, DbConnectionRecord, LlmConfig, QueryHistory, Snippet, UserSettings,
};
use serde::{Deserialize, Serialize};
use tauri::command;
use uuid::Uuid;

// ============ Connection Commands ============

#[derive(Debug, Deserialize)]
pub struct CreateConnectionRequest {
    pub name: String,
    pub db_type: String,
    pub host: String,
    pub port: String,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ssl_enabled: bool,
}

#[command]
pub async fn save_connection(
    connection: CreateConnectionRequest,
) -> Result<DbConnectionRecord, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let conn_record = DbConnectionRecord {
        id: Uuid::new_v4().to_string(),
        name: connection.name,
        db_type: connection.db_type,
        host: connection.host,
        port: connection.port,
        database: connection.database,
        username: connection.username,
        password: connection.password,
        ssl_enabled: connection.ssl_enabled,
        created_at: now.clone(),
        updated_at: now,
    };

    db.create_connection(&conn_record)
        .map_err(|e| e.to_string())?;

    Ok(conn_record)
}

#[command]
pub async fn get_connections() -> Result<Vec<DbConnectionRecord>, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    db.get_connections().map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct UpdateConnectionRequest {
    pub id: String,
    pub name: String,
    pub db_type: String,
    pub host: String,
    pub port: String,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ssl_enabled: bool,
}

#[command]
pub async fn update_connection(connection: UpdateConnectionRequest) -> Result<DbConnectionRecord, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;

    let existing = db.get_connections().map_err(|e| e.to_string())?;
    let original = existing.iter().find(|c| c.id == connection.id)
        .ok_or_else(|| "Connection not found".to_string())?;

    let conn_record = DbConnectionRecord {
        id: connection.id,
        name: connection.name,
        db_type: connection.db_type,
        host: connection.host,
        port: connection.port,
        database: connection.database,
        username: connection.username,
        password: connection.password,
        ssl_enabled: connection.ssl_enabled,
        created_at: original.created_at.clone(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    db.update_connection(&conn_record).map_err(|e| e.to_string())?;
    Ok(conn_record)
}

#[command]
pub async fn delete_connection_record(connection_id: String) -> Result<bool, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    db.delete_connection(&connection_id)
        .map_err(|e| e.to_string())
}

// ============ Query History Commands ============

#[derive(Debug, Deserialize)]
pub struct AddHistoryRequest {
    pub connection_id: String,
    pub natural_query: String,
    pub sql_query: String,
    pub result_count: Option<i32>,
    pub execution_time_ms: Option<i32>,
    pub status: String,
    pub error_message: Option<String>,
}

#[command]
pub async fn add_to_history(history: AddHistoryRequest) -> Result<QueryHistory, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let history_record = QueryHistory {
        id: Uuid::new_v4().to_string(),
        connection_id: history.connection_id,
        natural_query: history.natural_query,
        sql_query: history.sql_query,
        result_count: history.result_count,
        execution_time_ms: history.execution_time_ms,
        status: history.status,
        error_message: history.error_message,
        created_at: now,
    };

    db.add_query_history(&history_record)
        .map_err(|e| e.to_string())?;

    Ok(history_record)
}

#[command]
pub async fn get_history(
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<Vec<QueryHistory>, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    db.get_query_history(limit, offset)
        .map_err(|e| e.to_string())
}

#[command]
pub async fn search_history(
    search_term: String,
    limit: Option<i32>,
) -> Result<Vec<QueryHistory>, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    let limit = limit.unwrap_or(50);
    db.search_query_history(&search_term, limit)
        .map_err(|e| e.to_string())
}

#[command]
pub async fn clear_old_history(days_to_keep: Option<i32>) -> Result<i32, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    let days = days_to_keep.unwrap_or(30);
    db.clear_old_history(days).map_err(|e| e.to_string())
}

// ============ Snippet Commands ============

#[derive(Debug, Deserialize)]
pub struct CreateSnippetRequest {
    pub name: String,
    pub description: Option<String>,
    pub natural_query: String,
    pub sql_query: String,
    pub tags: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSnippetRequest {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub natural_query: String,
    pub sql_query: String,
    pub tags: Option<String>,
}

#[command]
pub async fn create_snippet(snippet: CreateSnippetRequest) -> Result<Snippet, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let snippet_record = Snippet {
        id: Uuid::new_v4().to_string(),
        name: snippet.name,
        description: snippet.description,
        natural_query: snippet.natural_query,
        sql_query: snippet.sql_query,
        tags: snippet.tags.unwrap_or_default(),
        created_at: now.clone(),
        updated_at: now,
    };

    db.create_snippet(&snippet_record)
        .map_err(|e| e.to_string())?;

    Ok(snippet_record)
}

#[command]
pub async fn get_snippets() -> Result<Vec<Snippet>, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    db.get_snippets().map_err(|e| e.to_string())
}

#[command]
pub async fn update_snippet(snippet: UpdateSnippetRequest) -> Result<bool, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let snippet_record = Snippet {
        id: snippet.id,
        name: snippet.name,
        description: snippet.description,
        natural_query: snippet.natural_query,
        sql_query: snippet.sql_query,
        tags: snippet.tags.unwrap_or_default(),
        created_at: String::new(),
        updated_at: now,
    };

    db.update_snippet(&snippet_record)
        .map_err(|e| e.to_string())
}

#[command]
pub async fn delete_snippet(snippet_id: String) -> Result<bool, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    db.delete_snippet(&snippet_id)
        .map_err(|e| e.to_string())
}

// ============ Settings Commands ============

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub theme: Option<String>,
    pub default_page_size: Option<i32>,
    pub query_timeout_seconds: Option<i32>,
    pub auto_save_history: Option<bool>,
}

#[command]
pub async fn get_settings() -> Result<UserSettings, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;

    let settings = db.get_user_settings().map_err(|e| e.to_string())?;

    if let Some(s) = settings {
        Ok(s)
    } else {
        let now = chrono::Utc::now().to_rfc3339();
        Ok(UserSettings {
            theme: "dark".to_string(),
            default_page_size: 50,
            query_timeout_seconds: 30,
            auto_save_history: true,
            created_at: now.clone(),
            updated_at: now,
        })
    }
}

#[command]
pub async fn update_settings(settings: UpdateSettingsRequest) -> Result<UserSettings, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;

    let current = db.get_user_settings().map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let updated_settings = match current {
        Some(mut s) => {
            if let Some(theme) = settings.theme {
                s.theme = theme;
            }
            if let Some(page_size) = settings.default_page_size {
                s.default_page_size = page_size;
            }
            if let Some(timeout) = settings.query_timeout_seconds {
                s.query_timeout_seconds = timeout;
            }
            if let Some(auto_save) = settings.auto_save_history {
                s.auto_save_history = auto_save;
            }
            s.updated_at = now;
            s
        }
        None => UserSettings {
            theme: settings.theme.unwrap_or_else(|| "dark".to_string()),
            default_page_size: settings.default_page_size.unwrap_or(50),
            query_timeout_seconds: settings.query_timeout_seconds.unwrap_or(30),
            auto_save_history: settings.auto_save_history.unwrap_or(true),
            created_at: now.clone(),
            updated_at: now,
        },
    };

    db.upsert_user_settings(&updated_settings)
        .map_err(|e| e.to_string())?;

    Ok(updated_settings)
}

// ============ LLM Configuration Commands ============

#[derive(Debug, Deserialize)]
pub struct UpdateLlmConfigRequest {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub api_url: Option<String>,
}

#[command]
pub async fn get_llm_config() -> Result<LlmConfig, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;

    let config = db.get_llm_config().map_err(|e| e.to_string())?;

    if let Some(c) = config {
        Ok(c)
    } else {
        let now = chrono::Utc::now().to_rfc3339();
        Ok(LlmConfig {
            provider: "gemini".to_string(),
            model: "gemini-2.5-flash".to_string(),
            api_key: String::new(),
            api_url: None,
            created_at: now.clone(),
            updated_at: now,
        })
    }
}

#[command]
pub async fn update_llm_config(config: UpdateLlmConfigRequest) -> Result<LlmConfig, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let llm_config = LlmConfig {
        provider: config.provider,
        model: config.model,
        api_key: config.api_key,
        api_url: config.api_url,
        created_at: now.clone(),
        updated_at: now,
    };

    db.upsert_llm_config(&llm_config)
        .map_err(|e| e.to_string())?;

    Ok(llm_config)
}
