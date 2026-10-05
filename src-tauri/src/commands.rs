use crate::app_db::{
    get_app_database, Chart, DbConnectionRecord, LlmConfig, QueryHistory, Script, Snippet,
    UserSettings,
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
    #[serde(default)]
    pub auth_json: String,
}

/// The public view never returns saved credential material to the webview.
#[derive(Serialize)]
pub struct ConnectionView {
    #[serde(flatten)]
    connection: DbConnectionRecord,
    has_password: bool,
    has_auth_json: bool,
}

fn public_endpoint(value: &str) -> String {
    if !value.contains("://") { return value.to_owned(); }
    // Mongo replica-set URLs are not accepted by url::Url. Work on the URI
    // authority instead, and hide query parameters which can contain tokens.
    let (scheme, rest) = value.split_once("://").unwrap();
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let authority_end = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let authority = authority.rsplit_once('@').map(|(_, host)| host).unwrap_or(authority);
    format!("{scheme}://{authority}{}", &rest[authority_end..])
}

fn public_connection(mut connection: DbConnectionRecord) -> ConnectionView {
    let has_password = !connection.password.is_empty();
    let has_auth_json = !connection.auth_json.is_empty();
    connection.password.clear();
    connection.auth_json.clear();
    connection.host = public_endpoint(&connection.host);
    ConnectionView { connection, has_password, has_auth_json }
}

#[derive(Debug, Deserialize)]
pub struct UpdateConnectionRequest {
    pub id: String,
    #[serde(flatten)]
    pub details: CreateConnectionRequest,
    #[serde(default)]
    pub clear_password: bool,
    #[serde(default)]
    pub clear_auth_json: bool,
    #[serde(default)]
    pub replace_host: bool,
}

pub fn connection_draft(
    details: CreateConnectionRequest,
    original: Option<&DbConnectionRecord>,
    clear_password: bool,
    clear_auth_json: bool,
    replace_host: bool,
) -> Result<DbConnectionRecord, String> {
    if !["postgres", "mysql", "mariadb", "sqlite", "mongodb", "redis", "firestore", "firebase_rtdb"].contains(&details.db_type.as_str()) {
        return Err("Unsupported database engine".into());
    }
    if details.host.trim().is_empty() && details.db_type != "firestore" {
        return Err("Provide a database host, URL, or file path".into());
    }
    let same_engine = original.filter(|c| c.db_type == details.db_type);
    let keep = |value: String, existing: &str, clear: bool| {
        if clear { String::new() } else if value.is_empty() { existing.to_owned() } else { value }
    };
    let password = keep(details.password, same_engine.map(|c| c.password.as_str()).unwrap_or(""), clear_password);
    let auth_json = keep(details.auth_json, same_engine.map(|c| c.auth_json.as_str()).unwrap_or(""), clear_auth_json);
    if ["firestore", "firebase_rtdb"].contains(&details.db_type.as_str()) {
        crate::drivers::firebase_auth::ServiceAccount::from_json(&auth_json).map_err(|_| "Provide valid service-account JSON".to_string())?;
    }
    let host = same_engine.filter(|c| !replace_host && public_endpoint(&c.host) == details.host)
        .map(|c| c.host.clone()).unwrap_or(details.host);
    let now = chrono::Utc::now().to_rfc3339();
    Ok(DbConnectionRecord {
        id: original.map(|c| c.id.clone()).unwrap_or_else(|| Uuid::new_v4().to_string()),
        name: details.name, db_type: details.db_type, host,
        port: details.port, database: details.database, username: details.username,
        password, ssl_enabled: details.ssl_enabled, auth_json,
        created_at: original.map(|c| c.created_at.clone()).unwrap_or_else(|| now.clone()),
        updated_at: now,
    })
}

#[command]
pub async fn save_connection(connection: CreateConnectionRequest) -> Result<ConnectionView, String> {
    if connection.name.trim().is_empty() || connection.name.len() > 128 { return Err("Provide a connection name of at most 128 bytes".into()); }
    let db = get_app_database().map_err(|e| e.to_string())?;
    let record = connection_draft(connection, None, false, false, false)?;
    db.create_connection(&record).map_err(|e| e.to_string())?;
    Ok(public_connection(record))
}

#[command]
pub async fn get_connections() -> Result<Vec<ConnectionView>, String> {
    get_app_database().map_err(|e| e.to_string())?.get_connections()
        .map(|records| records.into_iter().map(public_connection).collect()).map_err(|e| e.to_string())
}

#[command]
pub async fn update_connection(connection: UpdateConnectionRequest) -> Result<ConnectionView, String> {
    if connection.details.name.trim().is_empty() || connection.details.name.len() > 128 { return Err("Provide a connection name of at most 128 bytes".into()); }
    let db = get_app_database().map_err(|e| e.to_string())?;
    let records = db.get_connections().map_err(|e| e.to_string())?;
    let original = records.iter().find(|c| c.id == connection.id).ok_or("Connection not found")?;
    let record = connection_draft(connection.details, Some(original), connection.clear_password, connection.clear_auth_json, connection.replace_host)?;
    db.update_connection(&record).map_err(|e| e.to_string())?;
    Ok(public_connection(record))
}

#[command]
pub async fn test_connection_draft(connection: CreateConnectionRequest, connection_id: Option<String>, clear_password: Option<bool>, clear_auth_json: Option<bool>, replace_host: Option<bool>) -> Result<bool, String> {
    let records = if connection_id.is_some() { get_app_database().map_err(|e| e.to_string())?.get_connections().map_err(|e| e.to_string())? } else { Vec::new() };
    let original = connection_id.as_ref().map(|id| records.iter().find(|c| &c.id == id).ok_or("Connection not found")).transpose()?;
    let record = connection_draft(connection, original, clear_password.unwrap_or(false), clear_auth_json.unwrap_or(false), replace_host.unwrap_or(false))?;
    let conn_str = crate::build_connection_string(&record)?;
    let driver = crate::drivers::create_driver(&record.db_type, &conn_str).await
        .map_err(|_| "Could not connect. Check the address, credentials, TLS settings, and network access.".to_string())?;
    driver.test_connection().await.map_err(|_| "Connection test failed. Check credentials, permissions, and network access.".to_string())
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

// ============ Chart Commands (issue #10) ============

#[derive(Debug, Deserialize)]
pub struct SaveChartRequest {
    /// When present, updates an existing chart; otherwise a new one is created.
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub connection_id: Option<String>,
    pub engine: String,
    pub query: String,
    pub chart_type: String,
    pub config_json: String,
}

#[command]
pub async fn list_charts() -> Result<Vec<Chart>, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    db.get_charts().map_err(|e| e.to_string())
}

#[command]
pub async fn get_chart(chart_id: String) -> Result<Option<Chart>, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    db.get_chart(&chart_id).map_err(|e| e.to_string())
}

/// Upsert a chart. New charts get a generated id; existing charts keep their id
/// and created_at. Returns the persisted record.
#[command]
pub async fn save_chart(chart: SaveChartRequest) -> Result<Chart, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    match chart.id {
        Some(id) => {
            // Preserve original created_at if the record still exists.
            let created_at = db
                .get_chart(&id)
                .map_err(|e| e.to_string())?
                .map(|c| c.created_at)
                .unwrap_or_else(|| now.clone());
            let record = Chart {
                id,
                name: chart.name,
                description: chart.description,
                connection_id: chart.connection_id,
                engine: chart.engine,
                query: chart.query,
                chart_type: chart.chart_type,
                config_json: chart.config_json,
                created_at,
                updated_at: now,
            };
            let updated = db.update_chart(&record).map_err(|e| e.to_string())?;
            if !updated {
                return Err(format!("Chart with ID '{}' not found", record.id));
            }
            Ok(record)
        }
        None => {
            let record = Chart {
                id: Uuid::new_v4().to_string(),
                name: chart.name,
                description: chart.description,
                connection_id: chart.connection_id,
                engine: chart.engine,
                query: chart.query,
                chart_type: chart.chart_type,
                config_json: chart.config_json,
                created_at: now.clone(),
                updated_at: now,
            };
            db.create_chart(&record).map_err(|e| e.to_string())?;
            Ok(record)
        }
    }
}

#[command]
pub async fn delete_chart(chart_id: String) -> Result<bool, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    db.delete_chart(&chart_id).map_err(|e| e.to_string())
}

// ============ Script Commands ============

#[derive(Debug, Deserialize)]
pub struct CreateScriptRequest {
    pub name: String,
    pub description: Option<String>,
    pub engine: String,
    pub query_language: String,
    pub body: String,
    pub params_json: Option<String>,
    pub tags: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateScriptRequest {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub engine: String,
    pub query_language: String,
    pub body: String,
    pub params_json: Option<String>,
    pub tags: Option<String>,
}

#[command]
pub async fn get_scripts() -> Result<Vec<Script>, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    db.get_scripts().map_err(|e| e.to_string())
}

#[command]
pub async fn create_script(script: CreateScriptRequest) -> Result<Script, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let record = Script {
        id: Uuid::new_v4().to_string(),
        name: script.name,
        description: script.description,
        engine: script.engine,
        query_language: script.query_language,
        body: script.body,
        params_json: script.params_json.unwrap_or_else(|| "[]".to_string()),
        tags: script.tags.unwrap_or_default(),
        is_builtin: false,
        created_at: now.clone(),
        updated_at: now,
    };

    db.create_script(&record).map_err(|e| e.to_string())?;
    Ok(record)
}

#[command]
pub async fn update_script(script: UpdateScriptRequest) -> Result<bool, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let record = Script {
        id: script.id,
        name: script.name,
        description: script.description,
        engine: script.engine,
        query_language: script.query_language,
        body: script.body,
        params_json: script.params_json.unwrap_or_else(|| "[]".to_string()),
        tags: script.tags.unwrap_or_default(),
        is_builtin: false,
        created_at: String::new(),
        updated_at: now,
    };

    db.update_script(&record).map_err(|e| e.to_string())
}

#[command]
pub async fn delete_script(script_id: String) -> Result<bool, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    db.delete_script(&script_id).map_err(|e| e.to_string())
}

// ============ Settings Commands ============

/// Default cap on rows sent to the LLM when explaining a result set.
pub const DEFAULT_EXPLAIN_MAX_ROWS: i32 = 50;

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub theme: Option<String>,
    pub default_page_size: Option<i32>,
    pub query_timeout_seconds: Option<i32>,
    pub auto_save_history: Option<bool>,
    pub explain_max_rows: Option<i32>,
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
            explain_max_rows: DEFAULT_EXPLAIN_MAX_ROWS,
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
            if let Some(max_rows) = settings.explain_max_rows {
                s.explain_max_rows = max_rows.clamp(1, 1000);
            }
            s.updated_at = now;
            s
        }
        None => UserSettings {
            theme: settings.theme.unwrap_or_else(|| "dark".to_string()),
            default_page_size: settings.default_page_size.unwrap_or(50),
            query_timeout_seconds: settings.query_timeout_seconds.unwrap_or(30),
            auto_save_history: settings.auto_save_history.unwrap_or(true),
            explain_max_rows: settings
                .explain_max_rows
                .unwrap_or(DEFAULT_EXPLAIN_MAX_ROWS)
                .clamp(1, 1000),
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
    #[serde(default)]
    pub clear_api_key: bool,
}

#[command]
pub async fn get_llm_config() -> Result<LlmConfigView, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;

    let config = db.get_llm_config().map_err(|e| e.to_string())?;

    if let Some(c) = config {
        Ok(public_llm_config(c))
    } else {
        let now = chrono::Utc::now().to_rfc3339();
        Ok(public_llm_config(LlmConfig {
            provider: "gemini".to_string(),
            model: "gemini-2.5-flash".to_string(),
            api_key: String::new(),
            api_url: None,
            created_at: now.clone(),
            updated_at: now,
        }))
    }
}

#[derive(Serialize)]
pub struct LlmConfigView {
    #[serde(flatten)]
    config: LlmConfig,
    has_api_key: bool,
}

fn public_llm_config(mut config: LlmConfig) -> LlmConfigView {
    let has_api_key = !config.api_key.is_empty();
    config.api_key.clear();
    config.api_url = config.api_url.map(|url| public_endpoint(&url));
    LlmConfigView { config, has_api_key }
}

#[command]
pub async fn update_llm_config(config: UpdateLlmConfigRequest) -> Result<LlmConfigView, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    let original = db.get_llm_config().map_err(|e| e.to_string())?;
    let same_provider = original.as_ref().filter(|c| c.provider == config.provider);
    let now = chrono::Utc::now().to_rfc3339();
    let api_key = if config.clear_api_key { String::new() }
        else if config.api_key.is_empty() { same_provider.map(|c| c.api_key.clone()).unwrap_or_default() }
        else { config.api_key };
    let api_url = match (config.api_url, same_provider.and_then(|c| c.api_url.as_ref())) {
        (Some(url), Some(original)) if public_endpoint(original) == url => Some(original.clone()),
        (url, _) => url,
    };
    let llm_config = LlmConfig {
        provider: config.provider, model: config.model, api_key, api_url,
        created_at: original.map(|c| c.created_at).unwrap_or_else(|| now.clone()), updated_at: now,
    };
    db.upsert_llm_config(&llm_config).map_err(|e| e.to_string())?;
    Ok(public_llm_config(llm_config))
}

#[command]
pub async fn list_gemini_models(api_key: Option<String>) -> Result<Vec<String>, String> {
    let key = match api_key.filter(|k| !k.trim().is_empty()) {
        Some(key) => key,
        None => get_app_database().map_err(|e| e.to_string())?.get_llm_config().map_err(|e| e.to_string())?
            .filter(|c| c.provider == "gemini" && !c.api_key.is_empty()).map(|c| c.api_key).ok_or("Enter or save a Gemini API key first")?,
    };
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build().map_err(|_| "Could not initialize the model request")?;
    let response = client.get("https://generativelanguage.googleapis.com/v1beta/models?pageSize=1000")
        .header("x-goog-api-key", key).send().await.map_err(|_| "Could not reach Gemini. Check network access.")?;
    if !response.status().is_success() { return Err(format!("Gemini model request failed (HTTP {}). Check your API key and permissions.", response.status().as_u16())); }
    let body: serde_json::Value = response.json().await.map_err(|_| "Gemini returned invalid model data")?;
    let models = body.get("models").and_then(|v| v.as_array()).ok_or("Gemini returned no model catalog")?;
    Ok(models.iter().filter(|m| m.get("supportedGenerationMethods").and_then(|v| v.as_array()).is_some_and(|methods| methods.iter().any(|v| v.as_str() == Some("generateContent"))))
        .filter_map(|m| m.get("name").and_then(|v| v.as_str()).map(|name| name.trim_start_matches("models/").to_owned())).collect())
}

#[command]
pub async fn list_ollama_models(api_url: Option<String>) -> Result<Vec<String>, String> {
    let original = get_app_database().map_err(|e| e.to_string())?.get_llm_config().map_err(|e| e.to_string())?
        .filter(|c| c.provider == "ollama").and_then(|c| c.api_url);
    let endpoint = match (api_url.filter(|url| !url.trim().is_empty()), original) {
        (Some(url), Some(original)) if public_endpoint(&original) == url => original,
        (Some(url), _) => url,
        (None, Some(original)) => original,
        (None, None) => "http://localhost:11434".into(),
    };
    let mut endpoint = url::Url::parse(&endpoint).map_err(|_| "Provide a valid Ollama URL")?;
    if !["http", "https"].contains(&endpoint.scheme()) { return Err("Use an HTTP or HTTPS Ollama URL".into()); }
    let path = format!("{}/api/tags", endpoint.path().trim_end_matches('/'));
    endpoint.set_path(&path);
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).build().map_err(|_| "Could not initialize the model request")?;
    let response = client.get(endpoint).send().await.map_err(|_| "Could not reach Ollama. Check the endpoint and network access.")?;
    if !response.status().is_success() { return Err(format!("Ollama model request failed (HTTP {})", response.status().as_u16())); }
    let body: serde_json::Value = response.json().await.map_err(|_| "Ollama returned invalid model data")?;
    let models = body.get("models").and_then(|v| v.as_array()).ok_or("Ollama returned no model catalog")?;
    Ok(models.iter().filter_map(|m| m.get("name").and_then(|v| v.as_str()).map(str::to_owned)).collect())
}


#[command]
pub async fn export_connection_profiles(connection_ids: Vec<String>, passphrase: String) -> Result<String, String> {
    if connection_ids.is_empty() || connection_ids.len() > 1000 {
        return Err("Select between 1 and 1000 connections".into());
    }
    let db = get_app_database().map_err(|e| e.to_string())?;
    let records = db.get_connections().map_err(|e| e.to_string())?;
    let selected: Vec<_> = records.into_iter().filter(|r| connection_ids.contains(&r.id)).collect();
    if selected.len() != connection_ids.len() { return Err("Selection contains missing or duplicate connections".into()); }
    let plaintext = serde_json::to_string(&selected).map_err(|_| "Could not encode connection profiles")?;
    tokio::task::spawn_blocking(move || crate::credential_vault::encrypt_backup(&plaintext, &passphrase))
        .await.map_err(|_| "Connection export worker failed")?
}

#[command]
pub async fn import_connection_profiles(backup: String, passphrase: String) -> Result<usize, String> {
    let records = tokio::task::spawn_blocking(move || {
        let plaintext = crate::credential_vault::decrypt_backup(&backup, &passphrase)?;
        serde_json::from_str::<Vec<DbConnectionRecord>>(&plaintext).map_err(|_| "Backup contains invalid connection profiles".to_string())
    }).await.map_err(|_| "Connection import worker failed")??;
    get_app_database().map_err(|e| e.to_string())?.import_connections(records).map_err(|e| e.to_string())
}

#[cfg(test)]
mod credential_boundary_tests {
    use super::*;

    fn record() -> DbConnectionRecord {
        DbConnectionRecord {
            id: "original".into(), name: "test".into(), db_type: "mongodb".into(),
            host: "mongodb://user:uri-secret@one:27017,two:27017/db?authMechanismProperties=token-secret".into(),
            port: "27017".into(), database: "db".into(), username: "user".into(),
            password: "password-secret".into(), auth_json: "service-secret".into(), ssl_enabled: false,
            created_at: "original-date".into(), updated_at: String::new(),
        }
    }

    fn details(host: String) -> CreateConnectionRequest {
        CreateConnectionRequest { name: "renamed".into(), db_type: "mongodb".into(), host,
            port: "27017".into(), database: "db".into(), username: "user".into(),
            password: String::new(), auth_json: String::new(), ssl_enabled: false }
    }

    #[test]
    fn public_views_exclude_saved_secrets_and_uri_authentication() {
        let view = public_connection(record());
        let json = serde_json::to_string(&view).unwrap();
        for secret in ["uri-secret", "token-secret", "password-secret", "service-secret"] { assert!(!json.contains(secret)); }
        assert!(view.has_password && view.has_auth_json);
        assert_eq!(view.connection.host, "mongodb://one:27017,two:27017/db");
        let config = public_llm_config(LlmConfig { provider: "custom".into(), model: "test".into(), api_key: "api-key-secret".into(), api_url: Some("https://user:url-secret@example.com/v1?key=query-secret".into()), created_at: String::new(), updated_at: String::new() });
        let json = serde_json::to_string(&config).unwrap();
        for secret in ["api-key-secret", "url-secret", "query-secret"] { assert!(!json.contains(secret)); }
        assert!(config.has_api_key);
    }

    #[test]
    fn blank_edits_keep_credentials_explicit_replacement_and_clear_work() {
        let original = record();
        let draft = connection_draft(details(public_endpoint(&original.host)), Some(&original), false, false, false).unwrap();
        assert_eq!(draft.host, original.host);
        assert_eq!(draft.password, original.password);
        assert_eq!(draft.auth_json, original.auth_json);
        assert_eq!(draft.id, original.id);
        assert_eq!(draft.created_at, original.created_at);
        let draft = connection_draft(details(public_endpoint(&original.host)), Some(&original), true, true, true).unwrap();
        assert_eq!(draft.host, public_endpoint(&original.host));
        assert!(draft.password.is_empty() && draft.auth_json.is_empty());
        let mut edit = details(public_endpoint(&original.host));
        edit.password = "replacement".into();
        assert_eq!(connection_draft(edit, Some(&original), false, false, false).unwrap().password, "replacement");
        let mut edit = details("localhost".into()); edit.db_type = "postgres".into();
        let draft = connection_draft(edit, Some(&original), false, false, false).unwrap();
        assert!(draft.password.is_empty() && draft.auth_json.is_empty());
        assert!(connection_draft(details(String::new()), None, false, false, false).is_err());
    }
}
