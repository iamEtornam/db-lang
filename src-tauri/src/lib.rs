mod app_db;
mod commands;
mod connection_pool;
mod database;
mod export;
mod gemini;

// Note: auth module removed - no authentication system

use app_db::init_app_database;
use std::path::PathBuf;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn translate_to_sql(query: &str) -> Result<String, String> {
    gemini::translate_to_sql(query)
        .await
        .map_err(|e| e.to_string())
}

fn get_app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("QueryStudio")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    dotenvy::dotenv().ok();

    // Initialize the app database
    let app_data_dir = get_app_data_dir();
    if let Err(e) = init_app_database(app_data_dir) {
        eprintln!("Failed to initialize app database: {}", e);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            // Database operations
            database::query_db,
            database::query_db_paginated,
            database::test_connection,
            // Schema exploration
            database::get_tables,
            database::get_table_columns,
            database::preview_table_data,
            // AI translation & explanation
            translate_to_sql,
            gemini::explain_query,
            gemini::suggest_query_improvements,
            gemini::generate_sample_queries,
            // Connection management
            commands::save_connection,
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
