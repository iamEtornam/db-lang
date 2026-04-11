use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub mod mongodb;
pub mod mysql;
pub mod postgres;
pub mod redis;
pub mod sqlite;

#[derive(Error, Debug)]
pub enum DriverError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Query failed: {0}")]
    QueryFailed(String),
    #[error("Unsupported engine: {0}")]
    UnsupportedEngine(String),
    #[error("Schema introspection failed: {0}")]
    SchemaError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QueryLanguage {
    Sql,
    Mql,
    Redis,
}

impl std::fmt::Display for QueryLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryLanguage::Sql => write!(f, "sql"),
            QueryLanguage::Mql => write!(f, "mql"),
            QueryLanguage::Redis => write!(f, "redis"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub schema: Option<String>,
    pub table_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub source_table: String,
    pub source_column: String,
    pub target_table: String,
    pub target_column: String,
    pub relationship_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedResult {
    pub data: String,
    pub total_count: Option<i64>,
    pub page: i32,
    pub page_size: i32,
    pub has_more: bool,
}

#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    async fn execute_query(&self, query: &str) -> Result<Vec<Value>, DriverError>;

    async fn execute_query_paginated(
        &self,
        query: &str,
        page: i32,
        page_size: i32,
    ) -> Result<PaginatedResult, DriverError>;

    async fn test_connection(&self) -> Result<bool, DriverError>;

    async fn get_tables(&self) -> Result<Vec<TableInfo>, DriverError>;

    async fn get_table_columns(
        &self,
        table: &str,
        schema: Option<&str>,
    ) -> Result<Vec<ColumnInfo>, DriverError>;

    async fn get_relationships(&self) -> Result<Vec<Relationship>, DriverError>;

    async fn preview_table_data(
        &self,
        table: &str,
        schema: Option<&str>,
        limit: i32,
    ) -> Result<Vec<Value>, DriverError>;

    fn engine_name(&self) -> &str;

    fn query_language(&self) -> QueryLanguage;
}

/// Factory: create the right driver for a given engine string
pub async fn create_driver(
    engine: &str,
    conn_str: &str,
) -> Result<Box<dyn DatabaseDriver>, DriverError> {
    match engine {
        "postgres" => {
            let driver = postgres::PostgresDriver::new(conn_str).await?;
            Ok(Box::new(driver))
        }
        "mysql" | "mariadb" => {
            let driver = mysql::MysqlDriver::new(conn_str).await?;
            Ok(Box::new(driver))
        }
        "sqlite" => {
            let driver = sqlite::SqliteDriver::new(conn_str)?;
            Ok(Box::new(driver))
        }
        "mongodb" => {
            let driver = mongodb::MongoDriver::new(conn_str).await?;
            Ok(Box::new(driver))
        }
        "redis" => {
            let driver = redis::RedisDriver::new(conn_str).await?;
            Ok(Box::new(driver))
        }
        _ => Err(DriverError::UnsupportedEngine(engine.to_string())),
    }
}

pub fn quote_identifier(engine: &str, name: &str, schema: Option<&str>) -> String {
    match engine {
        "postgres" | "mssql" => {
            let escaped_name = name.replace('"', "\"\"");
            match schema {
                Some(s) => format!("\"{}\".\"{}\"", s.replace('"', "\"\""), escaped_name),
                None => format!("\"{}\"", escaped_name),
            }
        }
        "mysql" | "mariadb" => {
            let escaped_name = name.replace('`', "``");
            match schema {
                Some(s) => format!("`{}`.`{}`", s.replace('`', "``"), escaped_name),
                None => format!("`{}`", escaped_name),
            }
        }
        _ => {
            let escaped_name = name.replace('"', "\"\"");
            match schema {
                Some(s) => format!("\"{}\".\"{}\"", s.replace('"', "\"\""), escaped_name),
                None => format!("\"{}\"", escaped_name),
            }
        }
    }
}

pub fn strip_pagination(query: &str) -> String {
    let trimmed = query.trim().trim_end_matches(';').trim();
    let upper = trimmed.to_uppercase();

    let without_offset = if let Some(pos) = upper.rfind(" OFFSET ") {
        let after = &upper[pos..];
        if !after.contains(')') || after.matches(')').count() <= after.matches('(').count() {
            trimmed[..pos].trim().to_string()
        } else {
            trimmed.to_string()
        }
    } else {
        trimmed.to_string()
    };

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
