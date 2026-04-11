use async_trait::async_trait;
use serde_json::Value;

use super::{ColumnInfo, DatabaseDriver, DriverError, PaginatedResult, QueryLanguage, Relationship, TableInfo, strip_pagination};

pub struct PostgresDriver {
    client: tokio_postgres::Client,
}

impl PostgresDriver {
    pub async fn new(conn_str: &str) -> Result<Self, DriverError> {
        let (client, connection) = tokio_postgres::connect(conn_str, tokio_postgres::NoTls)
            .await
            .map_err(|e| DriverError::ConnectionFailed(e.to_string()))?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Postgres connection error: {}", e);
            }
        });

        Ok(Self { client })
    }

    async fn query_rows(&self, query: &str) -> Result<Vec<Value>, DriverError> {
        let rows = self
            .client
            .query(query, &[])
            .await
            .map_err(|e| {
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
                DriverError::QueryFailed(detail)
            })?;

        let mut results = Vec::new();
        for row in rows {
            let mut map = serde_json::Map::new();
            for (i, column) in row.columns().iter().enumerate() {
                let value: Value = match column.type_() {
                    &tokio_postgres::types::Type::VARCHAR
                    | &tokio_postgres::types::Type::TEXT
                    | &tokio_postgres::types::Type::NAME
                    | &tokio_postgres::types::Type::BPCHAR => {
                        row.try_get::<_, Option<String>>(i)
                            .ok()
                            .flatten()
                            .map(Value::String)
                            .unwrap_or(Value::Null)
                    }
                    &tokio_postgres::types::Type::INT2 => {
                        row.try_get::<_, Option<i16>>(i)
                            .ok()
                            .flatten()
                            .map(|v| Value::Number(v.into()))
                            .unwrap_or(Value::Null)
                    }
                    &tokio_postgres::types::Type::INT4 => {
                        row.try_get::<_, Option<i32>>(i)
                            .ok()
                            .flatten()
                            .map(|v| Value::Number(v.into()))
                            .unwrap_or(Value::Null)
                    }
                    &tokio_postgres::types::Type::INT8 => {
                        row.try_get::<_, Option<i64>>(i)
                            .ok()
                            .flatten()
                            .map(|v| Value::Number(v.into()))
                            .unwrap_or(Value::Null)
                    }
                    &tokio_postgres::types::Type::FLOAT4 => {
                        row.try_get::<_, Option<f32>>(i)
                            .ok()
                            .flatten()
                            .and_then(|v| serde_json::Number::from_f64(v as f64))
                            .map(Value::Number)
                            .unwrap_or(Value::Null)
                    }
                    &tokio_postgres::types::Type::FLOAT8 => {
                        row.try_get::<_, Option<f64>>(i)
                            .ok()
                            .flatten()
                            .and_then(|v| serde_json::Number::from_f64(v))
                            .map(Value::Number)
                            .unwrap_or(Value::Null)
                    }
                    &tokio_postgres::types::Type::BOOL => {
                        row.try_get::<_, Option<bool>>(i)
                            .ok()
                            .flatten()
                            .map(Value::Bool)
                            .unwrap_or(Value::Null)
                    }
                    &tokio_postgres::types::Type::JSON | &tokio_postgres::types::Type::JSONB => {
                        row.try_get::<_, Option<serde_json::Value>>(i)
                            .ok()
                            .flatten()
                            .unwrap_or(Value::Null)
                    }
                    &tokio_postgres::types::Type::TIMESTAMP
                    | &tokio_postgres::types::Type::TIMESTAMPTZ
                    | &tokio_postgres::types::Type::DATE
                    | &tokio_postgres::types::Type::TIME => {
                        let s = row
                            .try_get::<_, chrono::NaiveDateTime>(i)
                            .ok()
                            .map(|dt| dt.to_string())
                            .or_else(|| {
                                row.try_get::<_, chrono::NaiveDate>(i)
                                    .ok()
                                    .map(|d| d.to_string())
                            });
                        s.map(Value::String).unwrap_or(Value::Null)
                    }
                    _ => {
                        row.try_get::<_, Option<String>>(i)
                            .ok()
                            .flatten()
                            .map(Value::String)
                            .unwrap_or(Value::Null)
                    }
                };
                map.insert(column.name().to_string(), value);
            }
            results.push(Value::Object(map));
        }
        Ok(results)
    }
}

#[async_trait]
impl DatabaseDriver for PostgresDriver {
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
        self.client
            .query_one("SELECT 1", &[])
            .await
            .map(|_| true)
            .map_err(|e| DriverError::ConnectionFailed(e.to_string()))
    }

    async fn get_tables(&self) -> Result<Vec<TableInfo>, DriverError> {
        let rows = self.query_rows(
            "SELECT table_name as name, table_schema as schema, table_type
             FROM information_schema.tables
             WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
             ORDER BY table_schema, table_name",
        ).await?;

        Ok(rows.iter().map(|row| TableInfo {
            name: row["name"].as_str().unwrap_or("").to_string(),
            schema: row["schema"].as_str().map(|s| s.to_string()),
            table_type: row["table_type"].as_str().unwrap_or("TABLE").to_string(),
        }).collect())
    }

    async fn get_table_columns(&self, table: &str, schema: Option<&str>) -> Result<Vec<ColumnInfo>, DriverError> {
        let schema = schema.unwrap_or("public");
        let query = format!(
            "SELECT
                c.column_name as name,
                c.data_type,
                (c.is_nullable = 'YES') as is_nullable,
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
            table, schema, table, schema, table, schema
        );

        let rows = self.query_rows(&query).await?;
        Ok(rows.iter().map(|row| ColumnInfo {
            name: row["name"].as_str().unwrap_or("").to_string(),
            data_type: row["data_type"].as_str().unwrap_or("").to_string(),
            is_nullable: row["is_nullable"].as_bool().unwrap_or(true),
            column_default: row["column_default"].as_str().map(|s| s.to_string()),
            is_primary_key: row["is_primary_key"].as_bool().unwrap_or(false),
            is_foreign_key: row["is_foreign_key"].as_bool().unwrap_or(false),
            referenced_table: row["referenced_table"].as_str().map(|s| s.to_string()),
            referenced_column: row["referenced_column"].as_str().map(|s| s.to_string()),
        }).collect())
    }

    async fn get_relationships(&self) -> Result<Vec<Relationship>, DriverError> {
        let rows = self.query_rows(
            "SELECT
                kcu.table_name as source_table,
                kcu.column_name as source_column,
                ccu.table_name as target_table,
                ccu.column_name as target_column
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
                ON tc.constraint_name = kcu.constraint_name
            JOIN information_schema.constraint_column_usage ccu
                ON tc.constraint_name = ccu.constraint_name
            WHERE tc.constraint_type = 'FOREIGN KEY'
              AND tc.table_schema NOT IN ('pg_catalog', 'information_schema')"
        ).await?;

        Ok(rows.iter().map(|row| Relationship {
            source_table: row["source_table"].as_str().unwrap_or("").to_string(),
            source_column: row["source_column"].as_str().unwrap_or("").to_string(),
            target_table: row["target_table"].as_str().unwrap_or("").to_string(),
            target_column: row["target_column"].as_str().unwrap_or("").to_string(),
            relationship_type: Some("many-to-one".to_string()),
        }).collect())
    }

    async fn preview_table_data(&self, table: &str, schema: Option<&str>, limit: i32) -> Result<Vec<Value>, DriverError> {
        let full_name = match schema {
            Some(s) => format!("\"{}\".\"{}\"", s, table),
            None => format!("\"{}\"", table),
        };
        self.query_rows(&format!("SELECT * FROM {} LIMIT {}", full_name, limit)).await
    }

    fn engine_name(&self) -> &str {
        "postgres"
    }

    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::Sql
    }
}
