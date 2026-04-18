mod app_db;
mod commands;
mod connection_pool;
mod database;
mod drivers;
mod export;
mod gemini;
mod schema_kb;

use app_db::{init_app_database, get_app_database, DbConnectionRecord};
use drivers::{create_driver, TableInfo, ColumnInfo, PaginatedResult};
use drivers::firebase_auth::FirebaseConnBlob;
use std::path::PathBuf;

/// Replace the Atlas-style `<db_password>` (and legacy `<password>`)
/// placeholders in a MongoDB URI with the URL-encoded password. If no
/// password is provided the URI is returned unchanged.
fn substitute_mongo_password_placeholder(uri: &str, password: &str) -> String {
    if password.is_empty() {
        return uri.to_string();
    }
    let encoded = urlencoding::encode(password).into_owned();
    let mut out = String::with_capacity(uri.len());
    let mut rest = uri;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        if let Some(end) = tail.find('>') {
            let token = &tail[1..end];
            let lower = token.to_ascii_lowercase();
            if lower == "db_password" || lower == "password" {
                out.push_str(&encoded);
            } else {
                out.push_str(&tail[..=end]);
            }
            rest = &tail[end + 1..];
        } else {
            out.push_str(tail);
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}

/// Build a connection string from stored connection details.
/// Keeps credentials on the Rust side so they never transit through the frontend.
fn build_connection_string(conn: &DbConnectionRecord) -> String {
    let encoded_pwd = urlencoding::encode(&conn.password);
    let encoded_user = urlencoding::encode(&conn.username);

    match conn.db_type.as_str() {
        "postgres" => format!(
            "postgresql://{}:{}@{}:{}/{}",
            encoded_user, encoded_pwd, conn.host, conn.port,
            if conn.database.is_empty() { "postgres" } else { &conn.database }
        ),
        "mysql" | "mariadb" => format!(
            "mysql://{}:{}@{}:{}/{}",
            encoded_user, encoded_pwd, conn.host, conn.port,
            if conn.database.is_empty() { "mysql" } else { &conn.database }
        ),
        "sqlite" => conn.host.clone(),
        "mssql" => format!(
            "mssql://{}:{}@{}:{}/{}",
            encoded_user, encoded_pwd, conn.host, conn.port,
            if conn.database.is_empty() { "master" } else { &conn.database }
        ),
        "mongodb" => {
            // If the user pasted a full URI (`mongodb://` or `mongodb+srv://`),
            // pass it through verbatim and only substitute the `<db_password>`
            // placeholder Atlas embeds in copied connection strings.
            let trimmed = conn.host.trim();
            let lower = trimmed.to_lowercase();
            if lower.starts_with("mongodb://") || lower.starts_with("mongodb+srv://") {
                substitute_mongo_password_placeholder(trimmed, &conn.password)
            } else if !conn.username.is_empty() && !conn.password.is_empty() {
                format!(
                    "mongodb://{}:{}@{}:{}/{}",
                    encoded_user, encoded_pwd, conn.host, conn.port,
                    if conn.database.is_empty() { "test" } else { &conn.database }
                )
            } else {
                format!(
                    "mongodb://{}:{}/{}",
                    conn.host, conn.port,
                    if conn.database.is_empty() { "test" } else { &conn.database }
                )
            }
        }
        "redis" => {
            if !conn.password.is_empty() {
                format!(
                    "redis://:{}@{}:{}/{}",
                    encoded_pwd, conn.host, conn.port,
                    if conn.database.is_empty() { "0" } else { &conn.database }
                )
            } else {
                format!(
                    "redis://{}:{}/{}",
                    conn.host, conn.port,
                    if conn.database.is_empty() { "0" } else { &conn.database }
                )
            }
        }
        "firestore" => {
            let blob = FirebaseConnBlob {
                auth_json: conn.auth_json.clone(),
                project_id: conn.username.clone(),
                database_url: String::new(),
                firestore_db_id: if conn.database.is_empty() {
                    "(default)".to_string()
                } else {
                    conn.database.clone()
                },
            };
            blob.encode()
        }
        "firebase_rtdb" => {
            let blob = FirebaseConnBlob {
                auth_json: conn.auth_json.clone(),
                project_id: conn.username.clone(),
                database_url: conn.host.clone(),
                firestore_db_id: String::new(),
            };
            blob.encode()
        }
        _ => String::new(),
    }
}

/// Look up a saved connection and return (engine, connection_string).
fn resolve_connection(connection_id: &str) -> Result<(String, String), String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    let connections = db.get_connections().map_err(|e| e.to_string())?;
    let conn = connections
        .iter()
        .find(|c| c.id == connection_id)
        .ok_or_else(|| format!("Connection '{}' not found", connection_id))?;
    Ok((conn.db_type.clone(), build_connection_string(conn)))
}

// ============ Database Commands ============

/// Execute a query using a saved connection ID (credentials stay on the backend).
#[tauri::command]
async fn query_db(connection_id: &str, query: &str) -> Result<String, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    let driver = create_driver(&engine, &conn_str).await.map_err(|e| e.to_string())?;
    let rows = driver.execute_query(query).await.map_err(|e| e.to_string())?;
    serde_json::to_string(&rows).map_err(|e| e.to_string())
}

#[tauri::command]
async fn query_db_paginated(
    connection_id: &str,
    query: &str,
    page: i32,
    page_size: i32,
) -> Result<PaginatedResult, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    let driver = create_driver(&engine, &conn_str).await.map_err(|e| e.to_string())?;
    driver.execute_query_paginated(query, page, page_size).await.map_err(|e| e.to_string())
}

/// Test connection using raw parameters (for new unsaved connections).
#[tauri::command]
async fn test_connection(engine: &str, conn_str: &str) -> Result<bool, String> {
    let driver = create_driver(engine, conn_str).await.map_err(|e| e.to_string())?;
    driver.test_connection().await.map_err(|e| e.to_string())
}

/// Test connection using a saved connection ID.
#[tauri::command]
async fn test_connection_by_id(connection_id: &str) -> Result<bool, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    let driver = create_driver(&engine, &conn_str).await.map_err(|e| e.to_string())?;
    driver.test_connection().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_tables(connection_id: &str) -> Result<Vec<TableInfo>, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    let driver = create_driver(&engine, &conn_str).await.map_err(|e| e.to_string())?;
    driver.get_tables().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_table_columns(
    connection_id: &str,
    table_name: &str,
    schema_name: Option<&str>,
) -> Result<Vec<ColumnInfo>, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    let driver = create_driver(&engine, &conn_str).await.map_err(|e| e.to_string())?;
    driver.get_table_columns(table_name, schema_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn preview_table_data(
    connection_id: &str,
    table_name: &str,
    schema_name: Option<&str>,
    limit: Option<i32>,
) -> Result<String, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    let driver = create_driver(&engine, &conn_str).await.map_err(|e| e.to_string())?;
    let rows = driver
        .preview_table_data(table_name, schema_name, limit.unwrap_or(100))
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&rows).map_err(|e| e.to_string())
}

// ============ AI Translation ============

#[tauri::command]
async fn translate_to_sql(query: &str) -> Result<String, String> {
    gemini::translate_to_sql(query)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn translate_with_schema(
    query: &str,
    schema_context: &str,
    table_names: Vec<String>,
    engine: &str,
) -> Result<String, String> {
    gemini::translate_with_schema(query, schema_context, &table_names, engine)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_llm_configured() -> Result<bool, String> {
    Ok(gemini::is_llm_configured())
}

#[tauri::command]
async fn translate_to_query(
    natural_language: &str,
    connection_id: &str,
    engine: &str,
) -> Result<gemini::TranslationResult, String> {
    gemini::translate_to_query_with_kb(natural_language, connection_id, engine)
        .await
        .map_err(|e| e.to_string())
}

// ============ AI Data Commands ============

#[tauri::command]
async fn generate_chart_config(
    data: &str,
    columns: Vec<String>,
    query: &str,
    engine: &str,
) -> Result<gemini::ChartConfig, String> {
    gemini::generate_chart_config(data, &columns, query, engine)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn explain_data(
    data: &str,
    columns: Vec<String>,
    query: &str,
) -> Result<String, String> {
    gemini::explain_data(data, &columns, query)
        .await
        .map_err(|e| e.to_string())
}

// ============ Schema Knowledge Base Commands ============

#[tauri::command]
async fn generate_schema_kb(
    connection_id: &str,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    schema_kb::generate_schema_kb(connection_id, &engine, &conn_str, &app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_schema_kb(connection_id: &str) -> Result<Option<schema_kb::SchemaKnowledgeBase>, String> {
    schema_kb::get_schema_kb(connection_id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn refresh_schema_kb(
    connection_id: &str,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    schema_kb::refresh_schema_kb(connection_id, &engine, &conn_str, &app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_table_description(table_desc_id: &str, description: &str) -> Result<(), String> {
    schema_kb::update_table_description(table_desc_id, description).map_err(|e| e.to_string())
}

// ============ Firebase helpers ============

/// Build the base64-encoded `firebase://...` connection string used by the
/// firestore / firebase_rtdb drivers. Called from the frontend "Test" button
/// before invoking `test_connection`, so that unsaved Firebase connections can
/// be exercised without first persisting them.
#[tauri::command]
fn build_firebase_conn_str(
    auth_json: &str,
    database_url: Option<&str>,
    firestore_db_id: Option<&str>,
) -> Result<String, String> {
    let project_id = serde_json::from_str::<serde_json::Value>(auth_json)
        .ok()
        .and_then(|v| v.get("project_id").and_then(|p| p.as_str()).map(|s| s.to_string()))
        .unwrap_or_default();

    let blob = FirebaseConnBlob {
        auth_json: auth_json.to_string(),
        project_id,
        database_url: database_url.unwrap_or("").to_string(),
        firestore_db_id: firestore_db_id.unwrap_or("").to_string(),
    };
    Ok(blob.encode())
}

// ============ Realtime Database Streaming ============

#[tauri::command]
async fn rtdb_subscribe(
    connection_id: &str,
    path: &str,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "firebase_rtdb" {
        return Err("rtdb_subscribe is only supported for firebase_rtdb connections".into());
    }

    let blob = FirebaseConnBlob::decode(&conn_str).map_err(|e| e.to_string())?;
    let sa = drivers::firebase_auth::ServiceAccount::from_json(&blob.auth_json)
        .map_err(|e| e.to_string())?;
    let auth = std::sync::Arc::new(drivers::firebase_auth::FirebaseAuth::new(sa));

    drivers::firebase_rtdb::subscribe_to_path(&blob.database_url, &auth, path, app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn rtdb_unsubscribe(sub_id: &str) -> Result<(), String> {
    drivers::firebase_rtdb::unsubscribe(sub_id).map_err(|e| e.to_string())
}

// ============ App Setup ============

fn get_app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("QueryStudio")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    dotenvy::dotenv().ok();

    let app_data_dir = get_app_data_dir();
    if let Err(e) = init_app_database(app_data_dir) {
        eprintln!("Failed to initialize app database: {}", e);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // Database operations
            query_db,
            query_db_paginated,
            test_connection,
            test_connection_by_id,
            // Schema exploration
            get_tables,
            get_table_columns,
            preview_table_data,
            // AI translation & explanation
            translate_to_sql,
            translate_with_schema,
            check_llm_configured,
            translate_to_query,
            gemini::explain_query,
            gemini::suggest_query_improvements,
            gemini::generate_sample_queries,
            // AI chart & data
            generate_chart_config,
            explain_data,
            // Schema Knowledge Base
            generate_schema_kb,
            get_schema_kb,
            refresh_schema_kb,
            update_table_description,
            // Connection management
            commands::save_connection,
            commands::update_connection,
            commands::get_connections,
            commands::delete_connection_record,
            // Query history
            commands::add_to_history,
            commands::get_history,
            commands::search_history,
            commands::clear_old_history,
            // Snippets
            commands::create_snippet,
            commands::get_snippets,
            commands::update_snippet,
            commands::delete_snippet,
            // Settings
            commands::get_settings,
            commands::update_settings,
            // LLM configuration
            commands::get_llm_config,
            commands::update_llm_config,
            // Export
            export::export_data,
            export::get_export_columns,
            // Firebase helpers
            build_firebase_conn_str,
            // Realtime Database streaming
            rtdb_subscribe,
            rtdb_unsubscribe,
            // Cache and pool management
            connection_pool::get_cache_stats,
            connection_pool::get_pool_stats,
            connection_pool::clear_query_cache,
            connection_pool::clear_connection_pools,
            connection_pool::cleanup_cache,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
