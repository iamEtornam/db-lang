mod app_db;
mod commands;
mod connection_pool;
mod database;
mod drivers;
mod export;
mod gemini;
mod schema_kb;
mod scripts;

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
fn build_connection_string(conn: &DbConnectionRecord) -> Result<String, String> {
    let encoded_pwd = urlencoding::encode(&conn.password);
    let encoded_user = urlencoding::encode(&conn.username);

    let s = match conn.db_type.as_str() {
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
            blob.encode().map_err(|e| e.to_string())?
        }
        "firebase_rtdb" => {
            let blob = FirebaseConnBlob {
                auth_json: conn.auth_json.clone(),
                project_id: conn.username.clone(),
                database_url: conn.host.clone(),
                firestore_db_id: String::new(),
            };
            blob.encode().map_err(|e| e.to_string())?
        }
        _ => String::new(),
    };
    Ok(s)
}

/// Look up a saved connection and return (engine, connection_string).
fn resolve_connection(connection_id: &str) -> Result<(String, String), String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    let connections = db.get_connections().map_err(|e| e.to_string())?;
    let conn = connections
        .iter()
        .find(|c| c.id == connection_id)
        .ok_or_else(|| format!("Connection '{}' not found", connection_id))?;
    Ok((conn.db_type.clone(), build_connection_string(conn)?))
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

/// Run a saved or built-in script against a connection. Params are a map of
/// `{name -> value}` substituted into the body via `{{name}}` tokens. The
/// resolved body is gated by the same destructive-keyword check used for
/// AI-generated queries, then each statement is run read-only through the
/// driver. Returns a JSON array of all rows produced.
#[tauri::command]
async fn run_script(
    connection_id: &str,
    script_id: &str,
    params: std::collections::HashMap<String, String>,
) -> Result<String, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    let script = db
        .get_script(script_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Script '{}' not found", script_id))?;

    let resolved = scripts::substitute_params(&script.body, &params);

    // Same guard the LLM layer uses for AI-generated queries.
    if gemini::contains_destructive_keywords(&resolved) {
        return Err(format!(
            "DestructiveQuery: this script contains destructive operations: {}",
            resolved
        ));
    }

    let (engine, conn_str) = resolve_connection(connection_id)?;
    let driver = create_driver(&engine, &conn_str)
        .await
        .map_err(|e| e.to_string())?;

    let statements = scripts::split_statements(&resolved, &script.query_language);
    let mut all_rows: Vec<serde_json::Value> = Vec::new();
    for stmt in statements {
        let rows = driver.execute_query(&stmt).await.map_err(|e| e.to_string())?;
        all_rows.extend(rows);
    }
    serde_json::to_string(&all_rows).map_err(|e| e.to_string())
}

/// Insert one MongoDB document into a collection. Returns the inserted
/// `_id` as a JSON-encoded string (matches the read path's serialisation).
#[tauri::command]
async fn mongo_insert_one(
    connection_id: &str,
    collection: &str,
    doc_json: &str,
) -> Result<String, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "mongodb" {
        return Err(format!("mongo_insert_one requires a mongodb connection; got '{}'", engine));
    }
    let driver = drivers::mongodb::MongoDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .insert_one(collection, doc_json)
        .await
        .map_err(|e| e.to_string())
}

/// Replace a single MongoDB document. The replacement always strips `_id`
/// before sending it to the server so an accidentally-edited `_id` field
/// can't trigger Mongo's "the _id field cannot be changed" error.
#[tauri::command]
async fn mongo_replace_one(
    connection_id: &str,
    collection: &str,
    filter_json: &str,
    replacement_json: &str,
) -> Result<u64, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "mongodb" {
        return Err(format!("mongo_replace_one requires a mongodb connection; got '{}'", engine));
    }
    let driver = drivers::mongodb::MongoDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .replace_one(collection, filter_json, replacement_json)
        .await
        .map_err(|e| e.to_string())
}

/// Partial-update a single Mongo document with $set (changed fields) and
/// $unset (removed fields). Frontend computes the diff against the
/// pre-edit document and sends only what changed.
#[tauri::command]
async fn mongo_update_one(
    connection_id: &str,
    collection: &str,
    filter_json: &str,
    set_json: &str,
    unset_fields: Vec<String>,
) -> Result<u64, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "mongodb" {
        return Err(format!("mongo_update_one requires a mongodb connection; got '{}'", engine));
    }
    let driver = drivers::mongodb::MongoDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .update_one_fields(collection, filter_json, set_json, &unset_fields)
        .await
        .map_err(|e| e.to_string())
}

/// Bulk-delete N MongoDB documents matching a filter. The filter is
/// free-form so callers can target by `_id $in [...]` (the bulk-select
/// UI's default) or by any other criteria.
#[tauri::command]
async fn mongo_delete_many(
    connection_id: &str,
    collection: &str,
    filter_json: &str,
) -> Result<u64, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "mongodb" {
        return Err(format!("mongo_delete_many requires a mongodb connection; got '{}'", engine));
    }
    let driver = drivers::mongodb::MongoDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .delete_many(collection, filter_json)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn mongo_delete_one(
    connection_id: &str,
    collection: &str,
    filter_json: &str,
) -> Result<u64, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "mongodb" {
        return Err(format!("mongo_delete_one requires a mongodb connection; got '{}'", engine));
    }
    let driver = drivers::mongodb::MongoDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .delete_one(collection, filter_json)
        .await
        .map_err(|e| e.to_string())
}

// ============ Redis writes ============

/// Read a single Redis key with its TYPE and current value. The frontend
/// re-reads via this command at the moment the Edit dialog opens so the
/// editor reflects the freshest server state, not a possibly-stale preview.
#[tauri::command]
async fn redis_get_key(
    connection_id: &str,
    key: &str,
) -> Result<drivers::redis::RedisKeyView, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "redis" {
        return Err(format!("redis_get_key requires a redis connection; got '{}'", engine));
    }
    let driver = drivers::redis::RedisDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver.read_value(key).await.map_err(|e| e.to_string())
}

/// Write a Redis key. The frontend tags the type so we route to the right
/// driver method; for non-string types the driver does DEL + recreate so
/// the resulting key exactly matches the user's edited value (no leftover
/// fields / list entries the new value didn't mention).
///
/// `value_json` shape per type:
///   string -> JSON string
///   hash   -> JSON object  { field: <string-coerced value> }
///   list   -> JSON array of strings
///   set    -> JSON array of strings
///   zset   -> JSON object  { member: <number score> }
#[tauri::command]
async fn redis_set_key(
    connection_id: &str,
    key: &str,
    key_type: &str,
    value_json: &str,
    ttl_seconds: Option<i64>,
) -> Result<(), String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "redis" {
        return Err(format!("redis_set_key requires a redis connection; got '{}'", engine));
    }
    let driver = drivers::redis::RedisDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;

    let parsed: serde_json::Value = serde_json::from_str(value_json)
        .map_err(|e| format!("Invalid JSON value: {}", e))?;

    match key_type {
        "string" => {
            // Accept any scalar — JSON number/bool/null get stringified, which
            // matches what Redis stores (everything is bytes anyway).
            let s = match parsed {
                serde_json::Value::String(s) => s,
                serde_json::Value::Null => String::new(),
                other => other.to_string(),
            };
            driver.set_string(key, &s, ttl_seconds).await.map_err(|e| e.to_string())
        }
        "hash" => {
            let obj = parsed.as_object().ok_or_else(||
                "Hash value must be a JSON object: { field: value }".to_string()
            )?;
            let fields: Vec<(String, String)> = obj
                .iter()
                .map(|(k, v)| (k.clone(), match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Null => String::new(),
                    other => other.to_string(),
                }))
                .collect();
            driver.set_hash(key, fields, ttl_seconds).await.map_err(|e| e.to_string())
        }
        "list" => {
            let arr = parsed.as_array().ok_or_else(||
                "List value must be a JSON array of strings".to_string()
            )?;
            let items: Vec<String> = arr
                .iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Null => String::new(),
                    other => other.to_string(),
                })
                .collect();
            driver.set_list(key, items, ttl_seconds).await.map_err(|e| e.to_string())
        }
        "set" => {
            let arr = parsed.as_array().ok_or_else(||
                "Set value must be a JSON array of strings".to_string()
            )?;
            let members: Vec<String> = arr
                .iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Null => String::new(),
                    other => other.to_string(),
                })
                .collect();
            driver.set_set(key, members, ttl_seconds).await.map_err(|e| e.to_string())
        }
        "zset" => {
            let obj = parsed.as_object().ok_or_else(||
                "Sorted-set value must be a JSON object: { member: numeric_score }".to_string()
            )?;
            let mut members = Vec::with_capacity(obj.len());
            for (m, s) in obj {
                let score = s
                    .as_f64()
                    .ok_or_else(|| format!("zset member '{}' has non-numeric score: {}", m, s))?;
                members.push((m.clone(), score));
            }
            driver.set_zset(key, members, ttl_seconds).await.map_err(|e| e.to_string())
        }
        other => Err(format!("Unsupported Redis key type: '{}'", other)),
    }
}

// ===== Redis per-type patch commands =====

/// Patch a hash: HSET changed fields, HDEL removed fields. TTL optional —
/// only touched when explicitly provided so partial updates don't reset
/// the existing expiry.
#[tauri::command]
async fn redis_hash_patch(
    connection_id: &str,
    key: &str,
    set_fields: std::collections::HashMap<String, String>,
    unset_fields: Vec<String>,
    ttl_seconds: Option<i64>,
) -> Result<(), String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "redis" {
        return Err(format!("redis_hash_patch requires a redis connection; got '{}'", engine));
    }
    let driver = drivers::redis::RedisDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    let set_vec: Vec<(String, String)> = set_fields.into_iter().collect();
    driver
        .hash_patch(key, set_vec, unset_fields, ttl_seconds)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn redis_set_patch(
    connection_id: &str,
    key: &str,
    add: Vec<String>,
    remove: Vec<String>,
    ttl_seconds: Option<i64>,
) -> Result<(), String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "redis" {
        return Err(format!("redis_set_patch requires a redis connection; got '{}'", engine));
    }
    let driver = drivers::redis::RedisDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .set_patch(key, add, remove, ttl_seconds)
        .await
        .map_err(|e| e.to_string())
}

/// Patch a sorted set. `set_members` maps member -> score (numeric); members
/// in the map get ZADD'd. `remove_members` get ZREM'd.
#[tauri::command]
async fn redis_zset_patch(
    connection_id: &str,
    key: &str,
    set_members: std::collections::HashMap<String, f64>,
    remove_members: Vec<String>,
    ttl_seconds: Option<i64>,
) -> Result<(), String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "redis" {
        return Err(format!("redis_zset_patch requires a redis connection; got '{}'", engine));
    }
    let driver = drivers::redis::RedisDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    let set_vec: Vec<(String, f64)> = set_members.into_iter().collect();
    driver
        .zset_patch(key, set_vec, remove_members, ttl_seconds)
        .await
        .map_err(|e| e.to_string())
}

/// LSET each (index, value) pair in a single MULTI/EXEC transaction. Used
/// when the user edited a list in place without changing its length;
/// length-changing edits fall back to redis_set_key with the full list.
#[tauri::command]
async fn redis_list_set_indices(
    connection_id: &str,
    key: &str,
    changes: Vec<(i64, String)>,
    ttl_seconds: Option<i64>,
) -> Result<(), String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "redis" {
        return Err(format!("redis_list_set_indices requires a redis connection; got '{}'", engine));
    }
    let driver = drivers::redis::RedisDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .list_set_indices(key, changes, ttl_seconds)
        .await
        .map_err(|e| e.to_string())
}

/// XADD a new entry to a stream. `entry_id` of "" lets Redis generate the
/// timestamp-based ID; returns the assigned ID.
#[tauri::command]
async fn redis_stream_add(
    connection_id: &str,
    key: &str,
    entry_id: &str,
    fields: std::collections::HashMap<String, String>,
    ttl_seconds: Option<i64>,
) -> Result<String, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "redis" {
        return Err(format!("redis_stream_add requires a redis connection; got '{}'", engine));
    }
    let driver = drivers::redis::RedisDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    let pairs: Vec<(String, String)> = fields.into_iter().collect();
    driver
        .stream_add(key, entry_id, pairs, ttl_seconds)
        .await
        .map_err(|e| e.to_string())
}

/// XDEL one or more stream entries by their IDs.
#[tauri::command]
async fn redis_stream_delete_entries(
    connection_id: &str,
    key: &str,
    entry_ids: Vec<String>,
) -> Result<u64, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "redis" {
        return Err(format!("redis_stream_delete_entries requires a redis connection; got '{}'", engine));
    }
    let driver = drivers::redis::RedisDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .stream_delete_entries(key, entry_ids)
        .await
        .map_err(|e| e.to_string())
}

/// DEL N keys in a single Redis command. Returns the number of keys that
/// actually existed and got deleted (so deleting an already-gone key
/// doesn't inflate the count).
#[tauri::command]
async fn redis_delete_keys(connection_id: &str, keys: Vec<String>) -> Result<u64, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "redis" {
        return Err(format!("redis_delete_keys requires a redis connection; got '{}'", engine));
    }
    let driver = drivers::redis::RedisDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver.delete_keys(&keys).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn redis_delete_key(connection_id: &str, key: &str) -> Result<u64, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "redis" {
        return Err(format!("redis_delete_key requires a redis connection; got '{}'", engine));
    }
    let driver = drivers::redis::RedisDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver.delete_key(key).await.map_err(|e| e.to_string())
}

// ============ Realtime Database writes ============

/// PUT a JSON value at the given RTDB path. Replaces whatever was there.
#[tauri::command]
async fn rtdb_set(connection_id: &str, path: &str, value_json: &str) -> Result<(), String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "firebase_rtdb" {
        return Err(format!("rtdb_set requires a firebase_rtdb connection; got '{}'", engine));
    }
    let driver = drivers::firebase_rtdb::RtdbDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .set_value(path, value_json)
        .await
        .map_err(|e| e.to_string())
}

/// POST a JSON value to an RTDB node, letting the server generate a push
/// key. Returns the generated key.
#[tauri::command]
async fn rtdb_push(connection_id: &str, path: &str, value_json: &str) -> Result<String, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "firebase_rtdb" {
        return Err(format!("rtdb_push requires a firebase_rtdb connection; got '{}'", engine));
    }
    let driver = drivers::firebase_rtdb::RtdbDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .push_value(path, value_json)
        .await
        .map_err(|e| e.to_string())
}

/// Partial-update an RTDB node — PATCH merges the JSON object at `path`,
/// null values delete that key. Frontend computes the diff against the
/// pre-edit value and sends only what changed.
#[tauri::command]
async fn rtdb_patch(
    connection_id: &str,
    path: &str,
    partial_json: &str,
) -> Result<(), String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "firebase_rtdb" {
        return Err(format!("rtdb_patch requires a firebase_rtdb connection; got '{}'", engine));
    }
    let driver = drivers::firebase_rtdb::RtdbDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .patch_value(path, partial_json)
        .await
        .map_err(|e| e.to_string())
}

/// Atomic bulk-delete of N RTDB children under a single parent node. One
/// PATCH with {key1: null, key2: null, ...} — all-or-nothing per Firebase
/// semantics. Returns the number of paths submitted (not necessarily the
/// number that existed beforehand).
#[tauri::command]
async fn rtdb_delete_many(
    connection_id: &str,
    parent_path: &str,
    child_keys: Vec<String>,
) -> Result<u64, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "firebase_rtdb" {
        return Err(format!("rtdb_delete_many requires a firebase_rtdb connection; got '{}'", engine));
    }
    let driver = drivers::firebase_rtdb::RtdbDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .delete_many_children(parent_path, &child_keys)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn rtdb_delete(connection_id: &str, path: &str) -> Result<(), String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "firebase_rtdb" {
        return Err(format!("rtdb_delete requires a firebase_rtdb connection; got '{}'", engine));
    }
    let driver = drivers::firebase_rtdb::RtdbDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .delete_value(path)
        .await
        .map_err(|e| e.to_string())
}

// ============ Firestore writes ============

/// Create a Firestore document. `doc_id` is optional — when empty the
/// server generates an ID. Returns the (server- or user-supplied) document
/// ID so the UI can refresh and reflect the new row.
#[tauri::command]
async fn firestore_create_document(
    connection_id: &str,
    collection: &str,
    doc_id: Option<&str>,
    doc_json: &str,
) -> Result<String, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "firestore" {
        return Err(format!("firestore_create_document requires a firestore connection; got '{}'", engine));
    }
    let driver = drivers::firestore::FirestoreDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    let id = doc_id.filter(|s| !s.is_empty());
    driver
        .create_document(collection, id, doc_json)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn firestore_patch_document(
    connection_id: &str,
    collection: &str,
    doc_id: &str,
    doc_json: &str,
) -> Result<(), String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "firestore" {
        return Err(format!("firestore_patch_document requires a firestore connection; got '{}'", engine));
    }
    let driver = drivers::firestore::FirestoreDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .patch_document(collection, doc_id, doc_json)
        .await
        .map_err(|e| e.to_string())
}

/// Partial-update a Firestore document. Only the field paths the caller
/// lists get touched (set or removed); other fields remain. Frontend
/// computes the diff against the pre-edit document.
#[tauri::command]
async fn firestore_patch_document_fields(
    connection_id: &str,
    collection: &str,
    doc_id: &str,
    field_paths: Vec<String>,
    fields_subset_json: &str,
) -> Result<(), String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "firestore" {
        return Err(format!("firestore_patch_document_fields requires a firestore connection; got '{}'", engine));
    }
    let driver = drivers::firestore::FirestoreDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .patch_document_fields(collection, doc_id, &field_paths, fields_subset_json)
        .await
        .map_err(|e| e.to_string())
}

/// List subcollection IDs under a Firestore document. `doc_path` is the
/// path relative to `documents`, e.g. `users/abc` or `users/abc/posts/xyz`.
/// Empty path returns top-level collections (same as `get_tables`).
#[tauri::command]
async fn firestore_list_subcollections(
    connection_id: &str,
    doc_path: &str,
) -> Result<Vec<String>, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "firestore" {
        return Err(format!("firestore_list_subcollections requires a firestore connection; got '{}'", engine));
    }
    let driver = drivers::firestore::FirestoreDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .list_subcollections(doc_path)
        .await
        .map_err(|e| e.to_string())
}

/// Bulk-delete N top-level Firestore documents from a single collection
/// via :commit batched writes. Returns the count of attempted deletes.
#[tauri::command]
async fn firestore_delete_many_documents(
    connection_id: &str,
    collection: &str,
    doc_ids: Vec<String>,
) -> Result<u64, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "firestore" {
        return Err(format!("firestore_delete_many_documents requires a firestore connection; got '{}'", engine));
    }
    let driver = drivers::firestore::FirestoreDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .delete_many_documents(collection, &doc_ids)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn firestore_delete_document(
    connection_id: &str,
    collection: &str,
    doc_id: &str,
) -> Result<(), String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    if engine != "firestore" {
        return Err(format!("firestore_delete_document requires a firestore connection; got '{}'", engine));
    }
    let driver = drivers::firestore::FirestoreDriver::new(&conn_str)
        .await
        .map_err(|e| e.to_string())?;
    driver
        .delete_document(collection, doc_id)
        .await
        .map_err(|e| e.to_string())
}

/// Run N SQL statements atomically inside a single transaction. Used by
/// the inline-cell-editing batch save flow. SQL engines only.
#[tauri::command]
async fn execute_sql_batch(
    connection_id: &str,
    statements: Vec<String>,
) -> Result<u64, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    match engine.as_str() {
        "postgres" => {
            let driver = drivers::postgres::PostgresDriver::new(&conn_str)
                .await
                .map_err(|e| e.to_string())?;
            driver
                .execute_batch(&statements)
                .await
                .map_err(|e| e.to_string())
        }
        "mysql" | "mariadb" => {
            let driver = drivers::mysql::MysqlDriver::new(&conn_str)
                .await
                .map_err(|e| e.to_string())?;
            driver
                .execute_batch(&statements)
                .await
                .map_err(|e| e.to_string())
        }
        "sqlite" => {
            let driver = drivers::sqlite::SqliteDriver::new(&conn_str)
                .map_err(|e| e.to_string())?;
            driver
                .execute_batch(&statements)
                .await
                .map_err(|e| e.to_string())
        }
        other => Err(format!(
            "execute_sql_batch is only supported for SQL engines; got '{}'",
            other
        )),
    }
}

/// Execute a non-row-returning SQL statement (INSERT / UPDATE / DELETE) and
/// return the number of affected rows. SQL engines only — NoSQL engines
/// each need their own write paths with different semantics.
#[tauri::command]
async fn execute_sql_statement(connection_id: &str, sql: &str) -> Result<u64, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;
    match engine.as_str() {
        "postgres" => {
            let driver = drivers::postgres::PostgresDriver::new(&conn_str)
                .await
                .map_err(|e| e.to_string())?;
            driver.execute_statement(sql).await.map_err(|e| e.to_string())
        }
        "mysql" | "mariadb" => {
            let driver = drivers::mysql::MysqlDriver::new(&conn_str)
                .await
                .map_err(|e| e.to_string())?;
            driver.execute_statement(sql).await.map_err(|e| e.to_string())
        }
        "sqlite" => {
            let driver = drivers::sqlite::SqliteDriver::new(&conn_str)
                .map_err(|e| e.to_string())?;
            driver.execute_statement(sql).await.map_err(|e| e.to_string())
        }
        other => Err(format!(
            "execute_sql_statement is only supported for SQL engines; got '{}'",
            other
        )),
    }
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

// ============ Custom Chart Commands (issue #10) ============

/// Re-run a saved chart's stored query against its connection and return the
/// rows as a JSON-encoded array (same `Vec<serde_json::Value>` shape the table
/// view consumes). Errors if the chart has no connection or it was deleted.
#[tauri::command]
async fn run_chart(chart_id: &str) -> Result<String, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    let chart = db
        .get_chart(chart_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Chart '{}' not found", chart_id))?;

    let connection_id = chart
        .connection_id
        .ok_or_else(|| "This chart has no connection; pick one to re-run it".to_string())?;

    let (engine, conn_str) = resolve_connection(&connection_id)?;
    let driver = create_driver(&engine, &conn_str)
        .await
        .map_err(|e| e.to_string())?;
    let rows = driver
        .execute_query(&chart.query)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&rows).map_err(|e| e.to_string())
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

/// Roughly 8 KB JSON cap on the downsampled sample shipped to the LLM. Paired
/// with the user-configurable `explain_max_rows`; whichever bites first wins.
const EXPLAIN_MAX_BYTES: usize = 8 * 1024;

/// Interpret a result set with schema-KB + query context and return a
/// structured explanation. `result_summary` is a JSON array of result rows (the
/// frontend's downsampled view); it is re-downsampled on the Rust side. An
/// optional `question` enables follow-ups without re-running the query.
#[tauri::command]
async fn explain_query_result(
    connection_id: &str,
    query: &str,
    result_summary: &str,
    question: Option<String>,
    force_refresh: Option<bool>,
) -> Result<gemini::ResultExplanation, String> {
    let (engine, conn_str) = resolve_connection(connection_id)?;

    // The result rows arrive as a JSON array string from the frontend.
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(result_summary).map_err(|e| format!("Invalid result_summary JSON: {e}"))?;

    // Identify the engine's query language without opening a DB connection.
    let query_language = drivers::query_language_for_engine(&engine).to_string();

    // Schema KB context for the referenced tables/collections, if generated.
    let schema_context = match schema_kb::get_schema_kb(connection_id) {
        Ok(Some(kb)) => schema_kb::build_schema_context(&kb),
        _ => String::new(),
    };

    // Configurable row cap (defaults to 50; clamped on write).
    let max_rows = get_app_database()
        .ok()
        .and_then(|db| db.get_user_settings().ok().flatten())
        .map(|s| s.explain_max_rows.max(1) as usize)
        .unwrap_or(commands::DEFAULT_EXPLAIN_MAX_ROWS as usize);

    // Reuse the in-process cache so identical (query, result, question) triples
    // don't re-bill the LLM. Key shape mirrors the query cache (conn_str + key).
    let cache = connection_pool::get_cache();
    let cache_payload = format!(
        "explain_result\x1f{}\x1f{}\x1f{}",
        query,
        result_summary,
        question.as_deref().unwrap_or("")
    );
    // The "Refresh" button passes force_refresh=true to skip the cache and
    // re-bill the LLM; otherwise an identical triple returns the cached result.
    if !force_refresh.unwrap_or(false) {
        if let Some(hit) = cache.get(&conn_str, &cache_payload) {
            if let Ok(parsed) = serde_json::from_str::<gemini::ResultExplanation>(&hit) {
                return Ok(parsed);
            }
        }
    }

    let explanation = gemini::explain_query_result(
        query,
        &engine,
        &query_language,
        &schema_context,
        &rows,
        question.as_deref(),
        max_rows,
        EXPLAIN_MAX_BYTES,
    )
    .await
    .map_err(|e| e.to_string())?;

    if let Ok(serialized) = serde_json::to_string(&explanation) {
        cache.set(&conn_str, &cache_payload, serialized);
    }

    Ok(explanation)
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
    blob.encode().map_err(|e| e.to_string())
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
    let auth = std::sync::Arc::new(
        drivers::firebase_auth::FirebaseAuth::new(sa).map_err(|e| e.to_string())?,
    );

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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Reseed the built-in script library from bundled resources on every
            // launch so shipped updates take effect. User scripts are untouched.
            use tauri::Manager;
            if let Ok(resource_dir) = app.path().resource_dir() {
                let builtins = scripts::load_builtin_scripts(&resource_dir);
                if let Ok(db) = get_app_database() {
                    if let Err(e) = db.reseed_builtin_scripts(&builtins) {
                        eprintln!("Failed to seed built-in scripts: {}", e);
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Database operations
            query_db,
            query_db_paginated,
            execute_sql_statement,
            execute_sql_batch,
            mongo_insert_one,
            mongo_replace_one,
            mongo_update_one,
            mongo_delete_one,
            mongo_delete_many,
            firestore_create_document,
            firestore_patch_document,
            firestore_patch_document_fields,
            firestore_delete_document,
            firestore_delete_many_documents,
            firestore_list_subcollections,
            rtdb_set,
            rtdb_push,
            rtdb_patch,
            rtdb_delete,
            rtdb_delete_many,
            redis_get_key,
            redis_set_key,
            redis_hash_patch,
            redis_set_patch,
            redis_zset_patch,
            redis_list_set_indices,
            redis_stream_add,
            redis_stream_delete_entries,
            redis_delete_key,
            redis_delete_keys,
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
            explain_query_result,
            // Custom charts (issue #10)
            run_chart,
            commands::list_charts,
            commands::get_chart,
            commands::save_chart,
            commands::delete_chart,
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
            // Scripts
            commands::get_scripts,
            commands::create_script,
            commands::update_script,
            commands::delete_script,
            run_script,
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
