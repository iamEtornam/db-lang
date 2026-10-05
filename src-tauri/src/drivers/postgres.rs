use async_trait::async_trait;
use serde_json::Value;

use super::{ColumnInfo, DatabaseDriver, DriverError, PaginatedResult, QueryLanguage, Relationship, TableInfo, strip_pagination};

pub struct PostgresDriver {
    read_only: bool,
    query_lock: tokio::sync::Mutex<()>,
    client: tokio_postgres::Client,
}

impl PostgresDriver {
    pub async fn new(conn_str: &str) -> Result<Self, DriverError> {
        Self::new_with_policy(conn_str, false).await
    }
    pub async fn new_with_policy(conn_str: &str, read_only: bool) -> Result<Self, DriverError> {
        let (client, connection) = tokio_postgres::connect(conn_str, tokio_postgres::NoTls)
            .await
            .map_err(|e| DriverError::ConnectionFailed(e.to_string()))?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Postgres connection error: {}", e);
            }
        });

        Ok(Self { client, read_only, query_lock: tokio::sync::Mutex::new(()) })
    }

    async fn query_rows(&self, query: &str) -> Result<Vec<Value>, DriverError> {
        self.query_rows_params(query, &[]).await
    }

    async fn query_rows_params(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> Result<Vec<Value>, DriverError> {
        let _guard = self.query_lock.lock().await;
        if self.read_only { self.client.batch_execute("BEGIN READ ONLY").await.map_err(|e| DriverError::QueryFailed(e.to_string()))?; }
        let rows_result = self
            .client
            .query(query, params)
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
            });
        if self.read_only { self.client.batch_execute("ROLLBACK").await.map_err(|e| DriverError::QueryFailed(e.to_string()))?; }
        let rows = rows_result?;

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

    /// Run N statements atomically inside a single transaction. If any
    /// statement fails the whole batch rolls back per Postgres semantics.
    /// Returns the number of statements actually run (not row counts —
    /// tokio_postgres::batch_execute doesn't surface per-statement counts).
    pub async fn execute_batch(&self, statements: &[String]) -> Result<u64, DriverError> {
        if statements.is_empty() {
            return Ok(0);
        }
        let mut sql = String::from("BEGIN;\n");
        for s in statements {
            sql.push_str(s.trim());
            if !s.trim().ends_with(';') {
                sql.push(';');
            }
            sql.push('\n');
        }
        sql.push_str("COMMIT;");

        self.client
            .batch_execute(&sql)
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
        Ok(statements.len() as u64)
    }

    /// Execute a non-row-returning statement (INSERT/UPDATE/DELETE) and
    /// report how many rows it affected. Used by the schema-page write
    /// flow; not part of the cross-engine DatabaseDriver trait.
    pub async fn execute_statement(&self, sql: &str) -> Result<u64, DriverError> {
        self.client
            .execute(sql, &[])
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
            })
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
        let query =
            "SELECT c.column_name AS name, c.data_type,
                    (c.is_nullable = 'YES') AS is_nullable, c.column_default,
                    EXISTS (SELECT 1 FROM pg_constraint pk
                            WHERE pk.conrelid = source.oid AND pk.contype = 'p'
                              AND attribute.attnum = ANY(pk.conkey)) AS is_primary_key,
                    (fk.referenced_table IS NOT NULL) AS is_foreign_key,
                    fk.referenced_table, fk.referenced_column
             FROM information_schema.columns c
             JOIN pg_namespace ns ON ns.nspname = c.table_schema
             JOIN pg_class source ON source.relnamespace = ns.oid AND source.relname = c.table_name
             JOIN pg_attribute attribute ON attribute.attrelid = source.oid AND attribute.attname = c.column_name
             LEFT JOIN LATERAL (
                 SELECT target.relname AS referenced_table, target_column.attname AS referenced_column
                 FROM pg_constraint constraint_row
                 JOIN pg_class target ON target.oid = constraint_row.confrelid
                 JOIN pg_attribute target_column ON target_column.attrelid = target.oid
                     AND target_column.attnum = constraint_row.confkey[array_position(constraint_row.conkey, attribute.attnum)]
                 WHERE constraint_row.conrelid = source.oid AND constraint_row.contype = 'f'
                   AND attribute.attnum = ANY(constraint_row.conkey)
                 ORDER BY constraint_row.oid LIMIT 1
             ) fk ON true
             WHERE c.table_name = $1 AND c.table_schema = $2
             ORDER BY c.ordinal_position";

        let rows = self.query_rows_params(query, &[&table, &schema]).await?;
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
            "SELECT source_ns.nspname AS source_schema, source.relname AS source_table,
                    source_col.attname AS source_column,
                    target_ns.nspname AS target_schema, target.relname AS target_table,
                    target_col.attname AS target_column
             FROM pg_constraint fk
             JOIN pg_class source ON source.oid = fk.conrelid
             JOIN pg_namespace source_ns ON source_ns.oid = source.relnamespace
             JOIN pg_class target ON target.oid = fk.confrelid
             JOIN pg_namespace target_ns ON target_ns.oid = target.relnamespace
             CROSS JOIN LATERAL unnest(fk.conkey, fk.confkey) AS cols(source_num, target_num)
             JOIN pg_attribute source_col ON source_col.attrelid = source.oid AND source_col.attnum = cols.source_num
             JOIN pg_attribute target_col ON target_col.attrelid = target.oid AND target_col.attnum = cols.target_num
             WHERE fk.contype = 'f'
               AND source_ns.nspname NOT IN ('pg_catalog', 'information_schema')
             ORDER BY source_ns.nspname, source.relname, fk.oid, source_col.attnum"
        ).await?;

        Ok(rows.iter().map(|row| Relationship {
            source_schema: row["source_schema"].as_str().map(str::to_owned),
            target_schema: row["target_schema"].as_str().map(str::to_owned),
            source_table: row["source_table"].as_str().unwrap_or("").to_string(),
            source_column: row["source_column"].as_str().unwrap_or("").to_string(),
            target_table: row["target_table"].as_str().unwrap_or("").to_string(),
            target_column: row["target_column"].as_str().unwrap_or("").to_string(),
            relationship_type: Some("many-to-one".to_string()),
        }).collect())
    }

    async fn preview_table_data(&self, table: &str, schema: Option<&str>, limit: i32) -> Result<Vec<Value>, DriverError> {
        let full_name = match schema {
            Some(s) => format!("{}.{}", escape_pg_identifier(s), escape_pg_identifier(table)),
            None => escape_pg_identifier(table),
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

fn escape_pg_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
