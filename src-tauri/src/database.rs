use crate::connection_pool::{get_cache, get_mysql_pool};
use mysql_async::prelude::*;
use serde_json::Value;
use std::collections::HashMap;
use tauri::command;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Database connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Query execution failed: {0}")]
    QueryFailed(String),
    #[error("Unsupported database engine: {0}")]
    UnsupportedEngine(String),
}

pub enum DbConnection {
    Postgres(tokio_postgres::Client),
    Mysql(mysql_async::Conn),
    Sqlite(rusqlite::Connection),
}

pub trait Connection {
    fn new(conn_str: &str) -> Result<Self, DbError>
    where
        Self: Sized;
    fn query(&mut self, query: &str) -> Result<String, DbError>;
}

impl DbConnection {
    pub async fn new(engine: &str, conn_str: &str) -> Result<Self, DbError> {
        match engine {
            "postgres" => {
                let (client, connection) =
                    tokio_postgres::connect(conn_str, tokio_postgres::NoTls)
                        .await
                        .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        eprintln!("connection error: {}", e);
                    }
                });
                Ok(DbConnection::Postgres(client))
            }
            "mysql" => {
                // Use pooled connection
                let pool = get_mysql_pool()
                    .get_or_create(conn_str)
                    .map_err(|e| DbError::ConnectionFailed(e))?;
                let conn = pool
                    .get_conn()
                    .await
                    .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
                Ok(DbConnection::Mysql(conn))
            }
            "sqlite" => {
                let conn = rusqlite::Connection::open(conn_str)
                    .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
                Ok(DbConnection::Sqlite(conn))
            }
            _ => Err(DbError::UnsupportedEngine(engine.to_string())),
        }
    }

    pub async fn query(&mut self, query: &str) -> Result<String, DbError> {
        match self {
            DbConnection::Postgres(client) => {
                let rows = client
                    .query(query, &[])
                    .await
                    .map_err(|e| {
                        // Extract detailed Postgres error info
                        let detail = if let Some(db_err) = e.as_db_error() {
                            format!(
                                "{} (SQLSTATE {}{})",
                                db_err.message(),
                                db_err.code().code(),
                                db_err.hint().map(|h| format!(", hint: {}", h)).unwrap_or_default()
                            )
                        } else {
                            e.to_string()
                        };
                        DbError::QueryFailed(detail)
                    })?;
                let mut results = Vec::new();
                for row in rows {
                    let mut map = HashMap::new();
                    for (i, column) in row.columns().iter().enumerate() {
                        let value: Value = match column.type_() {
                            &tokio_postgres::types::Type::VARCHAR
                            | &tokio_postgres::types::Type::TEXT
                            | &tokio_postgres::types::Type::NAME
                            | &tokio_postgres::types::Type::BPCHAR => {
                                let s: Option<String> = row.get(i);
                                match s {
                                    Some(val) => serde_json::to_value(val).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                            &tokio_postgres::types::Type::INT2 => {
                                let s: Option<i16> = row.get(i);
                                match s {
                                    Some(val) => serde_json::to_value(val).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                            &tokio_postgres::types::Type::INT4 => {
                                let s: Option<i32> = row.get(i);
                                match s {
                                    Some(val) => serde_json::to_value(val).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                            &tokio_postgres::types::Type::INT8 => {
                                let s: Option<i64> = row.get(i);
                                match s {
                                    Some(val) => serde_json::to_value(val).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                            &tokio_postgres::types::Type::FLOAT4 => {
                                let s: Option<f32> = row.get(i);
                                match s {
                                    Some(val) => serde_json::to_value(val).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                            &tokio_postgres::types::Type::FLOAT8 => {
                                let s: Option<f64> = row.get(i);
                                match s {
                                    Some(val) => serde_json::to_value(val).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                            &tokio_postgres::types::Type::BOOL => {
                                let s: Option<bool> = row.get(i);
                                match s {
                                    Some(val) => serde_json::to_value(val).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                            &tokio_postgres::types::Type::TIMESTAMP
                            | &tokio_postgres::types::Type::TIMESTAMPTZ
                            | &tokio_postgres::types::Type::DATE
                            | &tokio_postgres::types::Type::TIME => {
                                // Try to get as string representation
                                let s: Option<String> = row
                                    .try_get::<_, chrono::NaiveDateTime>(i)
                                    .ok()
                                    .map(|dt| dt.to_string())
                                    .or_else(|| {
                                        row.try_get::<_, chrono::NaiveDate>(i)
                                            .ok()
                                            .map(|d| d.to_string())
                                    })
                                    .or_else(|| {
                                        row.try_get::<_, chrono::NaiveTime>(i)
                                            .ok()
                                            .map(|t| t.to_string())
                                    });
                                match s {
                                    Some(val) => serde_json::to_value(val).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                            &tokio_postgres::types::Type::JSON
                            | &tokio_postgres::types::Type::JSONB => {
                                let s: Option<serde_json::Value> = row.get(i);
                                s.unwrap_or(Value::Null)
                            }
                            _ => {
                                // Fallback: try to get as string
                                let s: Option<String> = row.try_get(i).ok();
                                match s {
                                    Some(val) => serde_json::to_value(val).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                        };
                        map.insert(column.name().to_string(), value);
                    }
                    results.push(map);
                }
                serde_json::to_string(&results).map_err(|e| DbError::QueryFailed(e.to_string()))
            }
            DbConnection::Mysql(conn) => {
                let query_str = query.to_string();
                let result: Vec<mysql_async::Row> = conn
                    .query(&query_str)
                    .await
                    .map_err(|e| DbError::QueryFailed(e.to_string()))?;

                let mut results: Vec<HashMap<String, Value>> = Vec::new();

                for row in result {
                    let mut map = HashMap::new();
                    let columns = row.columns_ref();

                    for (i, column) in columns.iter().enumerate() {
                        let col_name = column.name_str().to_string();
                        let col_type = column.column_type();

                        let value: Value = match col_type {
                            mysql_async::consts::ColumnType::MYSQL_TYPE_TINY
                            | mysql_async::consts::ColumnType::MYSQL_TYPE_SHORT
                            | mysql_async::consts::ColumnType::MYSQL_TYPE_LONG
                            | mysql_async::consts::ColumnType::MYSQL_TYPE_INT24 => {
                                let val: Option<i32> = row.get(i);
                                match val {
                                    Some(v) => serde_json::to_value(v).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                            mysql_async::consts::ColumnType::MYSQL_TYPE_LONGLONG => {
                                let val: Option<i64> = row.get(i);
                                match val {
                                    Some(v) => serde_json::to_value(v).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                            mysql_async::consts::ColumnType::MYSQL_TYPE_FLOAT => {
                                let val: Option<f32> = row.get(i);
                                match val {
                                    Some(v) => serde_json::to_value(v).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                            mysql_async::consts::ColumnType::MYSQL_TYPE_DOUBLE
                            | mysql_async::consts::ColumnType::MYSQL_TYPE_DECIMAL
                            | mysql_async::consts::ColumnType::MYSQL_TYPE_NEWDECIMAL => {
                                let val: Option<f64> = row.get(i);
                                match val {
                                    Some(v) => serde_json::to_value(v).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                            mysql_async::consts::ColumnType::MYSQL_TYPE_VARCHAR
                            | mysql_async::consts::ColumnType::MYSQL_TYPE_VAR_STRING
                            | mysql_async::consts::ColumnType::MYSQL_TYPE_STRING
                            | mysql_async::consts::ColumnType::MYSQL_TYPE_BLOB
                            | mysql_async::consts::ColumnType::MYSQL_TYPE_TINY_BLOB
                            | mysql_async::consts::ColumnType::MYSQL_TYPE_MEDIUM_BLOB
                            | mysql_async::consts::ColumnType::MYSQL_TYPE_LONG_BLOB => {
                                let val: Option<String> = row.get(i);
                                match val {
                                    Some(v) => serde_json::to_value(v).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                            mysql_async::consts::ColumnType::MYSQL_TYPE_DATE
                            | mysql_async::consts::ColumnType::MYSQL_TYPE_DATETIME
                            | mysql_async::consts::ColumnType::MYSQL_TYPE_TIMESTAMP
                            | mysql_async::consts::ColumnType::MYSQL_TYPE_TIME => {
                                let val: Option<String> = row.get(i);
                                match val {
                                    Some(v) => serde_json::to_value(v).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                            mysql_async::consts::ColumnType::MYSQL_TYPE_JSON => {
                                let val: Option<String> = row.get(i);
                                match val {
                                    Some(v) => {
                                        serde_json::from_str(&v).unwrap_or(Value::String(v))
                                    }
                                    None => Value::Null,
                                }
                            }
                            _ => {
                                // Fallback: try to get as string
                                let val: Option<String> = row.get(i);
                                match val {
                                    Some(v) => serde_json::to_value(v).unwrap_or(Value::Null),
                                    None => Value::Null,
                                }
                            }
                        };
                        map.insert(col_name, value);
                    }
                    results.push(map);
                }

                serde_json::to_string(&results).map_err(|e| DbError::QueryFailed(e.to_string()))
            }
            DbConnection::Sqlite(conn) => {
                let mut stmt = conn
                    .prepare(query)
                    .map_err(|e| DbError::QueryFailed(e.to_string()))?;

                let column_count = stmt.column_count();
                let column_names: Vec<String> = stmt
                    .column_names()
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect();

                let rows = stmt
                    .query_map([], |row| {
                        let mut map = HashMap::new();
                        for i in 0..column_count {
                            let value: Value = match row.get_ref(i).unwrap() {
                                rusqlite::types::ValueRef::Null => Value::Null,
                                rusqlite::types::ValueRef::Integer(i) => {
                                    serde_json::to_value(i).unwrap()
                                }
                                rusqlite::types::ValueRef::Real(f) => {
                                    serde_json::to_value(f).unwrap()
                                }
                                rusqlite::types::ValueRef::Text(t) => {
                                    serde_json::to_value(String::from_utf8_lossy(t)).unwrap()
                                }
                                rusqlite::types::ValueRef::Blob(b) => {
                                    serde_json::to_value(format!("{:?}", b)).unwrap()
                                }
                            };
                            map.insert(column_names[i].clone(), value);
                        }
                        Ok(map)
                    })
                    .map_err(|e| DbError::QueryFailed(e.to_string()))?;

                let mut results = Vec::new();
                for row in rows {
                    results.push(row.map_err(|e| DbError::QueryFailed(e.to_string()))?);
                }

                serde_json::to_string(&results).map_err(|e| DbError::QueryFailed(e.to_string()))
            }
        }
    }

    pub async fn test(engine: &str, conn_str: &str) -> Result<bool, DbError> {
        match engine {
            "postgres" => {
                tokio_postgres::connect(conn_str, tokio_postgres::NoTls)
                    .await
                    .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
                Ok(true)
            }
            "mysql" => {
                // Use pooled connection for test
                let pool = get_mysql_pool()
                    .get_or_create(conn_str)
                    .map_err(|e| DbError::ConnectionFailed(e))?;
                let _conn = pool
                    .get_conn()
                    .await
                    .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
                Ok(true)
            }
            "sqlite" => {
                rusqlite::Connection::open(conn_str)
                    .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
                Ok(true)
            }
            _ => Err(DbError::UnsupportedEngine(engine.to_string())),
        }
    }
}

/// Result with pagination info
#[derive(serde::Serialize)]
pub struct PaginatedResult {
    pub data: String,
    pub total_count: Option<i64>,
    pub page: i32,
    pub page_size: i32,
    pub has_more: bool,
}

/// Schema information structures
#[derive(serde::Serialize, Clone)]
pub struct TableInfo {
    pub name: String,
    pub schema: Option<String>,
    pub table_type: String,
}

#[derive(serde::Serialize, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub column_default: Option<String>,
    pub is_primary_key: bool,
    pub is_foreign_key: bool,
    pub referenced_table: Option<String>,
    pub referenced_column: Option<String>,
}

#[derive(serde::Serialize)]
pub struct TableSchema {
    pub table: TableInfo,
    pub columns: Vec<ColumnInfo>,
}

#[command]
pub async fn query_db(engine: &str, conn_str: &str, query: &str) -> Result<String, String> {
    // Check cache for SELECT queries
    let is_select = query.trim().to_uppercase().starts_with("SELECT");
    if is_select {
        if let Some(cached) = get_cache().get(conn_str, query) {
            return Ok(cached);
        }
    }

    let mut conn = DbConnection::new(engine, conn_str)
        .await
        .map_err(|e| e.to_string())?;

    let result = conn.query(query).await.map_err(|e| e.to_string())?;
    
    // Cache SELECT results
    if is_select {
        get_cache().set(conn_str, query, result.clone());
    }
    
    Ok(result)
}

/// Strip existing LIMIT/OFFSET clauses and trailing semicolons from a query
fn strip_pagination(query: &str) -> String {
    let trimmed = query.trim().trim_end_matches(';').trim();
    
    // Use a case-insensitive approach to remove trailing LIMIT/OFFSET
    let upper = trimmed.to_uppercase();
    
    // Find and remove OFFSET clause first (it comes after LIMIT)
    let without_offset = if let Some(pos) = upper.rfind(" OFFSET ") {
        // Make sure OFFSET is not inside a subquery by checking parentheses depth
        let after = &upper[pos..];
        if !after.contains(')') || after.matches(')').count() <= after.matches('(').count() {
            trimmed[..pos].trim().to_string()
        } else {
            trimmed.to_string()
        }
    } else {
        trimmed.to_string()
    };
    
    // Now remove LIMIT clause
    let upper2 = without_offset.to_uppercase();
    if let Some(pos) = upper2.rfind(" LIMIT ") {
        let after = &upper2[pos..];
        if !after.contains(')') || after.matches(')').count() <= after.matches('(').count() {
            without_offset[..pos].trim().to_string()
        } else {
            without_offset
        }
    } else {
        without_offset
    }
}

#[command]
pub async fn query_db_paginated(
    engine: &str,
    conn_str: &str,
    query: &str,
    page: i32,
    page_size: i32,
) -> Result<PaginatedResult, String> {
    let mut conn = DbConnection::new(engine, conn_str)
        .await
        .map_err(|e| e.to_string())?;

    // Clean the query: remove existing LIMIT/OFFSET and semicolons
    let base_query = strip_pagination(query);

    // Try to get total count for SELECT queries
    let total_count = if base_query.trim().to_uppercase().starts_with("SELECT") {
        let count_query = format!("SELECT COUNT(*) as count FROM ({}) as subquery", base_query);
        match conn.query(&count_query).await {
            Ok(result) => {
                let parsed: Vec<HashMap<String, Value>> = serde_json::from_str(&result).unwrap_or_default();
                parsed.first()
                    .and_then(|row| row.get("count"))
                    .and_then(|v| v.as_i64())
            }
            Err(_) => None
        }
    } else {
        None
    };

    // Apply pagination
    let offset = (page - 1) * page_size;
    let paginated_query = format!("{} LIMIT {} OFFSET {}", base_query, page_size, offset);

    let data = conn.query(&paginated_query).await.map_err(|e| e.to_string())?;
    
    let parsed: Vec<HashMap<String, Value>> = serde_json::from_str(&data).unwrap_or_default();
    let has_more = match total_count {
        Some(total) => (page * page_size) < total as i32,
        None => parsed.len() == page_size as usize,
    };

    Ok(PaginatedResult {
        data,
        total_count,
        page,
        page_size,
        has_more,
    })
}

#[command]
pub async fn test_connection(engine: &str, conn_str: &str) -> Result<bool, String> {
    DbConnection::test(engine, conn_str)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn get_tables(engine: &str, conn_str: &str) -> Result<Vec<TableInfo>, String> {
    let mut conn = DbConnection::new(engine, conn_str)
        .await
        .map_err(|e| e.to_string())?;

    let query = match engine {
        "postgres" => "SELECT table_name as name, table_schema as schema, table_type 
                       FROM information_schema.tables 
                       WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
                       ORDER BY table_schema, table_name",
        "mysql" => "SELECT table_name as name, table_schema as `schema`, table_type 
                    FROM information_schema.tables 
                    WHERE table_schema = DATABASE()
                    ORDER BY table_name",
        "sqlite" => "SELECT name, NULL as schema, type as table_type 
                     FROM sqlite_master 
                     WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'
                     ORDER BY name",
        _ => return Err("Unsupported engine".to_string()),
    };

    let result = conn.query(query).await.map_err(|e| e.to_string())?;
    let parsed: Vec<HashMap<String, Value>> = serde_json::from_str(&result).unwrap_or_default();
    
    let tables: Vec<TableInfo> = parsed.iter().map(|row| {
        TableInfo {
            name: row.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            schema: row.get("schema").and_then(|v| v.as_str()).map(|s| s.to_string()),
            table_type: row.get("table_type").and_then(|v| v.as_str()).unwrap_or("TABLE").to_string(),
        }
    }).collect();

    Ok(tables)
}

#[command]
pub async fn get_table_columns(engine: &str, conn_str: &str, table_name: &str, schema_name: Option<&str>) -> Result<Vec<ColumnInfo>, String> {
    let mut conn = DbConnection::new(engine, conn_str)
        .await
        .map_err(|e| e.to_string())?;

    let query = match engine {
        "postgres" => {
            let schema = schema_name.unwrap_or("public");
            format!(
                "SELECT 
                    c.column_name as name,
                    c.data_type,
                    c.is_nullable = 'YES' as is_nullable,
                    c.column_default,
                    COALESCE(pk.is_pk, false) as is_primary_key,
                    COALESCE(fk.is_fk, false) as is_foreign_key,
                    fk.referenced_table,
                    fk.referenced_column
                FROM information_schema.columns c
                LEFT JOIN (
                    SELECT kcu.column_name, true as is_pk
                    FROM information_schema.table_constraints tc
                    JOIN information_schema.key_column_usage kcu 
                        ON tc.constraint_name = kcu.constraint_name
                    WHERE tc.table_name = '{}' AND tc.table_schema = '{}' AND tc.constraint_type = 'PRIMARY KEY'
                ) pk ON c.column_name = pk.column_name
                LEFT JOIN (
                    SELECT 
                        kcu.column_name, 
                        true as is_fk,
                        ccu.table_name as referenced_table,
                        ccu.column_name as referenced_column
                    FROM information_schema.table_constraints tc
                    JOIN information_schema.key_column_usage kcu 
                        ON tc.constraint_name = kcu.constraint_name
                    JOIN information_schema.constraint_column_usage ccu 
                        ON tc.constraint_name = ccu.constraint_name
                    WHERE tc.table_name = '{}' AND tc.table_schema = '{}' AND tc.constraint_type = 'FOREIGN KEY'
                ) fk ON c.column_name = fk.column_name
                WHERE c.table_name = '{}' AND c.table_schema = '{}'
                ORDER BY c.ordinal_position",
                table_name, schema, table_name, schema, table_name, schema
            )
        }
        "mysql" => format!(
            "SELECT 
                c.COLUMN_NAME as name,
                c.DATA_TYPE as data_type,
                c.IS_NULLABLE = 'YES' as is_nullable,
                c.COLUMN_DEFAULT as column_default,
                c.COLUMN_KEY = 'PRI' as is_primary_key,
                c.COLUMN_KEY = 'MUL' as is_foreign_key,
                kcu.REFERENCED_TABLE_NAME as referenced_table,
                kcu.REFERENCED_COLUMN_NAME as referenced_column
            FROM information_schema.columns c
            LEFT JOIN information_schema.key_column_usage kcu 
                ON c.TABLE_NAME = kcu.TABLE_NAME 
                AND c.COLUMN_NAME = kcu.COLUMN_NAME 
                AND c.TABLE_SCHEMA = kcu.TABLE_SCHEMA
                AND kcu.REFERENCED_TABLE_NAME IS NOT NULL
            WHERE c.TABLE_NAME = '{}' AND c.TABLE_SCHEMA = DATABASE()
            ORDER BY c.ORDINAL_POSITION",
            table_name
        ),
        "sqlite" => format!("PRAGMA table_info('{}')", table_name),
        _ => return Err("Unsupported engine".to_string()),
    };

    let result = conn.query(&query).await.map_err(|e| e.to_string())?;
    let parsed: Vec<HashMap<String, Value>> = serde_json::from_str(&result).unwrap_or_default();

    let columns: Vec<ColumnInfo> = if engine == "sqlite" {
        // SQLite PRAGMA returns different format
        parsed.iter().map(|row| {
            ColumnInfo {
                name: row.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                data_type: row.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                is_nullable: row.get("notnull").and_then(|v| v.as_i64()).map(|v| v == 0).unwrap_or(true),
                column_default: row.get("dflt_value").and_then(|v| v.as_str()).map(|s| s.to_string()),
                is_primary_key: row.get("pk").and_then(|v| v.as_i64()).map(|v| v == 1).unwrap_or(false),
                is_foreign_key: false,
                referenced_table: None,
                referenced_column: None,
            }
        }).collect()
    } else {
        parsed.iter().map(|row| {
            ColumnInfo {
                name: row.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                data_type: row.get("data_type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                is_nullable: row.get("is_nullable").and_then(|v| v.as_bool()).unwrap_or(true),
                column_default: row.get("column_default").and_then(|v| v.as_str()).map(|s| s.to_string()),
                is_primary_key: row.get("is_primary_key").and_then(|v| v.as_bool()).unwrap_or(false),
                is_foreign_key: row.get("is_foreign_key").and_then(|v| v.as_bool()).unwrap_or(false),
                referenced_table: row.get("referenced_table").and_then(|v| v.as_str()).map(|s| s.to_string()),
                referenced_column: row.get("referenced_column").and_then(|v| v.as_str()).map(|s| s.to_string()),
            }
        }).collect()
    };

    Ok(columns)
}

#[command]
pub async fn preview_table_data(
    engine: &str,
    conn_str: &str,
    table_name: &str,
    schema_name: Option<&str>,
    limit: Option<i32>,
) -> Result<String, String> {
    let mut conn = DbConnection::new(engine, conn_str)
        .await
        .map_err(|e| e.to_string())?;

    let limit = limit.unwrap_or(100);
    let full_table_name = quote_table_name(engine, table_name, schema_name);

    let query = format!("SELECT * FROM {} LIMIT {}", full_table_name, limit);
    conn.query(&query).await.map_err(|e| e.to_string())
}

/// Properly quote table names to handle reserved words and special characters
fn quote_table_name(engine: &str, table_name: &str, schema_name: Option<&str>) -> String {
    match engine {
        "postgres" => {
            match schema_name {
                Some(schema) => format!("\"{}\".\"{}\"", schema, table_name),
                None => format!("\"{}\"", table_name),
            }
        }
        "mysql" => {
            match schema_name {
                Some(schema) => format!("`{}`.`{}`", schema, table_name),
                None => format!("`{}`", table_name),
            }
        }
        _ => {
            match schema_name {
                Some(schema) => format!("{}.{}", schema, table_name),
                None => table_name.to_string(),
            }
        }
    }
}
