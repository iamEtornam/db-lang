use async_trait::async_trait;
use mysql_async::prelude::*;
use serde_json::Value;

use crate::connection_pool::get_mysql_pool;
use super::{ColumnInfo, DatabaseDriver, DriverError, PaginatedResult, QueryLanguage, Relationship, TableInfo, strip_pagination};

pub struct MysqlDriver {
    conn: tokio::sync::Mutex<mysql_async::Conn>,
}

impl MysqlDriver {
    pub async fn new(conn_str: &str) -> Result<Self, DriverError> {
        let pool = get_mysql_pool()
            .get_or_create(conn_str)
            .map_err(|e| DriverError::ConnectionFailed(e))?;
        let conn = pool
            .get_conn()
            .await
            .map_err(|e| DriverError::ConnectionFailed(e.to_string()))?;

        Ok(Self { conn: tokio::sync::Mutex::new(conn) })
    }

    async fn query_rows(&self, query: &str) -> Result<Vec<Value>, DriverError> {
        let query_str = query.to_string();
        let mut conn = self.conn.lock().await;
        let result: Vec<mysql_async::Row> = conn
            .query(&query_str)
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

        Self::rows_to_json(result)
    }

    async fn query_rows_params<P: Into<mysql_async::Params> + Send>(
        &self,
        query: &str,
        params: P,
    ) -> Result<Vec<Value>, DriverError> {
        let query_str = query.to_string();
        let mut conn = self.conn.lock().await;
        let result: Vec<mysql_async::Row> = conn
            .exec(&query_str, params)
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

        Self::rows_to_json(result)
    }

    fn rows_to_json(result: Vec<mysql_async::Row>) -> Result<Vec<Value>, DriverError> {
        let mut results: Vec<Value> = Vec::new();

        for row in result {
            let mut map = serde_json::Map::new();
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
                        val.map(|v| Value::Number(v.into())).unwrap_or(Value::Null)
                    }
                    mysql_async::consts::ColumnType::MYSQL_TYPE_LONGLONG => {
                        let val: Option<i64> = row.get(i);
                        val.map(|v| Value::Number(v.into())).unwrap_or(Value::Null)
                    }
                    mysql_async::consts::ColumnType::MYSQL_TYPE_FLOAT => {
                        let val: Option<f32> = row.get(i);
                        val.and_then(|v| serde_json::Number::from_f64(v as f64))
                            .map(Value::Number)
                            .unwrap_or(Value::Null)
                    }
                    mysql_async::consts::ColumnType::MYSQL_TYPE_DOUBLE
                    | mysql_async::consts::ColumnType::MYSQL_TYPE_DECIMAL
                    | mysql_async::consts::ColumnType::MYSQL_TYPE_NEWDECIMAL => {
                        let val: Option<f64> = row.get(i);
                        val.and_then(|v| serde_json::Number::from_f64(v))
                            .map(Value::Number)
                            .unwrap_or(Value::Null)
                    }
                    mysql_async::consts::ColumnType::MYSQL_TYPE_JSON => {
                        let val: Option<String> = row.get(i);
                        match val {
                            Some(s) => serde_json::from_str(&s).unwrap_or(Value::String(s)),
                            None => Value::Null,
                        }
                    }
                    _ => {
                        let val: Option<String> = row.get(i);
                        val.map(Value::String).unwrap_or(Value::Null)
                    }
                };
                map.insert(col_name, value);
            }
            results.push(Value::Object(map));
        }
        Ok(results)
    }
}

fn escape_mysql_identifier(name: &str) -> String {
    format!("`{}`", name.replace('`', "``"))
}

#[async_trait]
impl DatabaseDriver for MysqlDriver {
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
            "SELECT table_name as name, table_schema as `schema`, table_type
             FROM information_schema.tables
             WHERE table_schema = DATABASE()
             ORDER BY table_name",
        ).await?;

        Ok(rows.iter().map(|row| TableInfo {
            name: row["name"].as_str().unwrap_or("").to_string(),
            schema: row["schema"].as_str().map(|s| s.to_string()),
            table_type: row["table_type"].as_str().unwrap_or("TABLE").to_string(),
        }).collect())
    }

    async fn get_table_columns(&self, table: &str, _schema: Option<&str>) -> Result<Vec<ColumnInfo>, DriverError> {
        let query =
            "SELECT
                c.COLUMN_NAME as name,
                c.DATA_TYPE as data_type,
                (c.IS_NULLABLE = 'YES') as is_nullable,
                c.COLUMN_DEFAULT as column_default,
                (c.COLUMN_KEY = 'PRI') as is_primary_key,
                (kcu.REFERENCED_TABLE_NAME IS NOT NULL) as is_foreign_key,
                kcu.REFERENCED_TABLE_NAME as referenced_table,
                kcu.REFERENCED_COLUMN_NAME as referenced_column
            FROM information_schema.columns c
            LEFT JOIN information_schema.key_column_usage kcu
                ON c.TABLE_NAME = kcu.TABLE_NAME
                AND c.COLUMN_NAME = kcu.COLUMN_NAME
                AND c.TABLE_SCHEMA = kcu.TABLE_SCHEMA
                AND kcu.REFERENCED_TABLE_NAME IS NOT NULL
            WHERE c.TABLE_NAME = ? AND c.TABLE_SCHEMA = DATABASE()
            ORDER BY c.ORDINAL_POSITION";

        let rows = self.query_rows_params(query, (table,)).await?;
        Ok(rows.iter().map(|row| ColumnInfo {
            name: row["name"].as_str().unwrap_or("").to_string(),
            data_type: row["data_type"].as_str().unwrap_or("").to_string(),
            is_nullable: row["is_nullable"].as_bool()
                .unwrap_or_else(|| row["is_nullable"].as_i64().map(|v| v == 1).unwrap_or(true)),
            column_default: row["column_default"].as_str().map(|s| s.to_string()),
            is_primary_key: row["is_primary_key"].as_bool()
                .unwrap_or_else(|| row["is_primary_key"].as_i64().map(|v| v == 1).unwrap_or(false)),
            is_foreign_key: row["is_foreign_key"].as_bool()
                .unwrap_or_else(|| row["is_foreign_key"].as_i64().map(|v| v == 1).unwrap_or(false)),
            referenced_table: row["referenced_table"].as_str().map(|s| s.to_string()),
            referenced_column: row["referenced_column"].as_str().map(|s| s.to_string()),
        }).collect())
    }

    async fn get_relationships(&self) -> Result<Vec<Relationship>, DriverError> {
        let rows = self.query_rows(
            "SELECT
                kcu.TABLE_NAME as source_table,
                kcu.COLUMN_NAME as source_column,
                kcu.REFERENCED_TABLE_NAME as target_table,
                kcu.REFERENCED_COLUMN_NAME as target_column
            FROM information_schema.key_column_usage kcu
            JOIN information_schema.table_constraints tc
                ON kcu.CONSTRAINT_NAME = tc.CONSTRAINT_NAME
                AND kcu.TABLE_SCHEMA = tc.TABLE_SCHEMA
            WHERE tc.CONSTRAINT_TYPE = 'FOREIGN KEY'
              AND kcu.TABLE_SCHEMA = DATABASE()"
        ).await?;

        Ok(rows.iter().filter_map(|row| {
            let target_table = row["target_table"].as_str()?.to_string();
            let target_column = row["target_column"].as_str()?.to_string();
            Some(Relationship {
                source_table: row["source_table"].as_str().unwrap_or("").to_string(),
                source_column: row["source_column"].as_str().unwrap_or("").to_string(),
                target_table,
                target_column,
                relationship_type: Some("many-to-one".to_string()),
            })
        }).collect())
    }

    async fn preview_table_data(&self, table: &str, _schema: Option<&str>, limit: i32) -> Result<Vec<Value>, DriverError> {
        self.query_rows(&format!("SELECT * FROM {} LIMIT {}", escape_mysql_identifier(table), limit)).await
    }

    fn engine_name(&self) -> &str {
        "mysql"
    }

    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::Sql
    }
}
