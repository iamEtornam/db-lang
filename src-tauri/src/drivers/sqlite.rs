use async_trait::async_trait;
use rusqlite::types::ValueRef;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{ColumnInfo, DatabaseDriver, DriverError, PaginatedResult, QueryLanguage, Relationship, TableInfo, strip_pagination};

pub struct SqliteDriver {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteDriver {
    pub fn new(path: &str) -> Result<Self, DriverError> {
        let conn = rusqlite::Connection::open(path)
            .map_err(|e| DriverError::ConnectionFailed(e.to_string()))?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    async fn query_rows(&self, query: &str) -> Result<Vec<Value>, DriverError> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(query)
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

        let column_count = stmt.column_count();
        let column_names: Vec<String> = stmt
            .column_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        let rows = stmt
            .query_map([], |row| {
                let mut map = serde_json::Map::new();
                for i in 0..column_count {
                    let name = column_names[i].clone();
                    let value = match row.get_ref(i).unwrap_or(ValueRef::Null) {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(i) => Value::Number(i.into()),
                        ValueRef::Real(f) => serde_json::Number::from_f64(f)
                            .map(Value::Number)
                            .unwrap_or(Value::Null),
                        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).to_string()),
                        ValueRef::Blob(b) => Value::String(format!("<blob {} bytes>", b.len())),
                    };
                    map.insert(name, value);
                }
                Ok(map)
            })
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(Value::Object(
                row.map_err(|e| DriverError::QueryFailed(e.to_string()))?,
            ));
        }
        Ok(results)
    }
}

#[async_trait]
impl DatabaseDriver for SqliteDriver {
    async fn execute_query(&self, query: &str) -> Result<Vec<Value>, DriverError> {
        self.query_rows(query).await
    }

    async fn execute_query_paginated(
        &self,
        query: &str,
        page: i32,
        page_size: i32,
    ) -> Result<PaginatedResult, DriverError> {
        let base_query = strip_pagination(query);
        let is_select = base_query.trim().to_uppercase().starts_with("SELECT");

        let total_count = if is_select {
            let count_query = format!("SELECT COUNT(*) as count FROM ({}) as subquery", base_query);
            self.query_rows(&count_query)
                .await
                .ok()
                .and_then(|rows| rows.first().cloned())
                .and_then(|row| row.get("count").and_then(|v| v.as_i64()))
        } else {
            None
        };

        let offset = (page - 1) * page_size;
        let paginated_query = format!("{} LIMIT {} OFFSET {}", base_query, page_size, offset);
        let rows = self.query_rows(&paginated_query).await?;
        let has_more = match total_count {
            Some(total) => (page * page_size) < total as i32,
            None => rows.len() == page_size as usize,
        };
        let data = serde_json::to_string(&rows)
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

        Ok(PaginatedResult { data, total_count, page, page_size, has_more })
    }

    async fn test_connection(&self) -> Result<bool, DriverError> {
        self.query_rows("SELECT 1").await.map(|_| true)
    }

    async fn get_tables(&self) -> Result<Vec<TableInfo>, DriverError> {
        let rows = self.query_rows(
            "SELECT name, NULL as schema, type as table_type
             FROM sqlite_master
             WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        ).await?;

        Ok(rows.iter().map(|row| TableInfo {
            name: row["name"].as_str().unwrap_or("").to_string(),
            schema: None,
            table_type: row["table_type"].as_str().unwrap_or("table").to_uppercase(),
        }).collect())
    }

    async fn get_table_columns(&self, table: &str, _schema: Option<&str>) -> Result<Vec<ColumnInfo>, DriverError> {
        let rows = self.query_rows(&format!("PRAGMA table_info('{}')", table)).await?;

        Ok(rows.iter().map(|row| ColumnInfo {
            name: row["name"].as_str().unwrap_or("").to_string(),
            data_type: row["type"].as_str().unwrap_or("TEXT").to_string(),
            is_nullable: row["notnull"].as_i64().map(|v| v == 0).unwrap_or(true),
            column_default: row["dflt_value"].as_str().map(|s| s.to_string()),
            is_primary_key: row["pk"].as_i64().map(|v| v == 1).unwrap_or(false),
            is_foreign_key: false,
            referenced_table: None,
            referenced_column: None,
        }).collect())
    }

    async fn get_relationships(&self) -> Result<Vec<Relationship>, DriverError> {
        let tables = self.get_tables().await?;
        let mut relationships = Vec::new();

        for table in &tables {
            let fk_rows = self.query_rows(&format!("PRAGMA foreign_key_list('{}')", table.name))
                .await
                .unwrap_or_default();

            for row in fk_rows {
                if let (Some(target_table), Some(from_col), Some(to_col)) = (
                    row["table"].as_str(),
                    row["from"].as_str(),
                    row["to"].as_str(),
                ) {
                    relationships.push(Relationship {
                        source_table: table.name.clone(),
                        source_column: from_col.to_string(),
                        target_table: target_table.to_string(),
                        target_column: to_col.to_string(),
                        relationship_type: Some("many-to-one".to_string()),
                    });
                }
            }
        }
        Ok(relationships)
    }

    async fn preview_table_data(&self, table: &str, _schema: Option<&str>, limit: i32) -> Result<Vec<Value>, DriverError> {
        self.query_rows(&format!("SELECT * FROM \"{}\" LIMIT {}", table, limit)).await
    }

    fn engine_name(&self) -> &str {
        "sqlite"
    }

    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::Sql
    }
}
