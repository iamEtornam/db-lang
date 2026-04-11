mod app_db;
mod commands;
mod connection_pool;
mod database;
mod drivers;
mod export;
mod gemini;
mod schema_kb;

use app_db::init_app_database;
use drivers::{create_driver, TableInfo, ColumnInfo, PaginatedResult};
use std::path::PathBuf;

// ============ Database Commands (new driver-based) ============

#[tauri::command]
async fn query_db(engine: &str, conn_str: &str, query: &str) -> Result<String, String> {
    let driver = create_driver(engine, conn_str).await.map_err(|e| e.to_string())?;
    let rows = driver.execute_query(query).await.map_err(|e| e.to_string())?;
    serde_json::to_string(&rows).map_err(|e| e.to_string())
}

#[tauri::command]
async fn query_db_paginated(
    engine: &str,
    conn_str: &str,
    query: &str,
    page: i32,
    page_size: i32,
) -> Result<PaginatedResult, String> {
    let driver = create_driver(engine, conn_str).await.map_err(|e| e.to_string())?;
    driver.execute_query_paginated(query, page, page_size).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn test_connection(engine: &str, conn_str: &str) -> Result<bool, String> {
    let driver = create_driver(engine, conn_str).await.map_err(|e| e.to_string())?;
    driver.test_connection().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_tables(engine: &str, conn_str: &str) -> Result<Vec<TableInfo>, String> {
    let driver = create_driver(engine, conn_str).await.map_err(|e| e.to_string())?;
    driver.get_tables().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_table_columns(
    engine: &str,
    conn_str: &str,
    table_name: &str,
    schema_name: Option<&str>,
) -> Result<Vec<ColumnInfo>, String> {
    let driver = create_driver(engine, conn_str).await.map_err(|e| e.to_string())?;
    driver.get_table_columns(table_name, schema_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn preview_table_data(
    engine: &str,
    conn_str: &str,
    table_name: &str,
    schema_name: Option<&str>,
    limit: Option<i32>,
) -> Result<String, String> {
    let driver = create_driver(engine, conn_str).await.map_err(|e| e.to_string())?;
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
    engine: &str,
    conn_str: &str,
    app: tauri::AppHandle,
) -> Result<String, String> {
    schema_kb::generate_schema_kb(connection_id, engine, conn_str, &app)
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
    engine: &str,
    conn_str: &str,
    app: tauri::AppHandle,
) -> Result<String, String> {
    schema_kb::refresh_schema_kb(connection_id, engine, conn_str, &app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_table_description(table_desc_id: &str, description: &str) -> Result<(), String> {
    schema_kb::update_table_description(table_desc_id, description).map_err(|e| e.to_string())
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
