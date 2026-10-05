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
        Self::new_with_policy(path, false)
    }
    pub fn new_with_policy(path: &str, read_only: bool) -> Result<Self, DriverError> {
        let conn = if read_only { rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX) } else { rusqlite::Connection::open(path) }
            .map_err(|e| DriverError::ConnectionFailed(e.to_string()))?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub async fn set_query_deadline(&self, deadline: std::time::Instant) {
        self.conn.lock().await.progress_handler(1000, Some(move || std::time::Instant::now() >= deadline));
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

    /// Run N statements atomically inside a single transaction. Uses
    /// rusqlite's unchecked_transaction to drive an immediate BEGIN. On any
    /// error the drop guard rolls back; we only call commit() on success.
    pub async fn execute_batch(&self, statements: &[String]) -> Result<u64, DriverError> {
        if statements.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock().await;
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        for s in statements {
            tx.execute(s, [])
                .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        }
        tx.commit()
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
        Ok(statements.len() as u64)
    }

    /// Execute a non-row-returning statement (INSERT/UPDATE/DELETE) and
    /// report how many rows it affected. Used by the schema-page write
    /// flow; not part of the cross-engine DatabaseDriver trait.
    pub async fn execute_statement(&self, sql: &str) -> Result<u64, DriverError> {
        let conn = self.conn.lock().await;
        conn.execute(sql, [])
            .map(|n| n as u64)
            .map_err(|e| DriverError::QueryFailed(e.to_string()))
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
        let rows = self.query_rows(&format!("PRAGMA table_info({})", escape_sqlite_identifier(table))).await?;

        let foreign_keys = self.query_rows(&format!("PRAGMA foreign_key_list({})", escape_sqlite_identifier(table))).await?;
        Ok(rows.iter().map(|row| {
            let foreign_key = foreign_keys.iter().find(|fk| fk["from"].as_str().is_some_and(|name| name.eq_ignore_ascii_case(row["name"].as_str().unwrap_or(""))));
            ColumnInfo {
            name: row["name"].as_str().unwrap_or("").to_string(),
            data_type: row["type"].as_str().unwrap_or("TEXT").to_string(),
            is_nullable: row["notnull"].as_i64().map(|v| v == 0).unwrap_or(true),
            column_default: row["dflt_value"].as_str().map(|s| s.to_string()),
            is_primary_key: row["pk"].as_i64().map(|v| v > 0).unwrap_or(false),
            is_foreign_key: foreign_key.is_some(),
            referenced_table: foreign_key.and_then(|fk| fk["table"].as_str()).map(str::to_owned),
            referenced_column: foreign_key.and_then(|fk| fk["to"].as_str()).map(str::to_owned),
        }}).collect())
    }

    async fn get_relationships(&self) -> Result<Vec<Relationship>, DriverError> {
        let tables = self.get_tables().await?;
        let mut relationships = Vec::new();
        let mut table_columns = std::collections::HashMap::<String, Vec<Value>>::new();
        for table in &tables {
            if !table_columns.contains_key(&table.name) {
                let columns = self.query_rows(&format!("PRAGMA table_info({})", escape_sqlite_identifier(&table.name))).await?;
                table_columns.insert(table.name.clone(), columns);
            }
            let fk_rows = self.query_rows(&format!("PRAGMA foreign_key_list({})", escape_sqlite_identifier(&table.name))).await?;
            for row in fk_rows {
                let declared_target = row["table"].as_str().ok_or_else(|| DriverError::QueryFailed("Foreign key has no target table".into()))?;
                let target_table = tables.iter().find(|t| t.name.eq_ignore_ascii_case(declared_target)).map(|t| t.name.as_str()).unwrap_or(declared_target);
                let declared_source = row["from"].as_str().ok_or_else(|| DriverError::QueryFailed("Foreign key has no source column".into()))?;
                let source_column = table_columns[&table.name].iter().find_map(|r| r["name"].as_str().filter(|name| name.eq_ignore_ascii_case(declared_source))).unwrap_or(declared_source).to_owned();
                if !table_columns.contains_key(target_table) {
                    let columns = self.query_rows(&format!("PRAGMA table_info({})", escape_sqlite_identifier(target_table))).await?;
                    table_columns.insert(target_table.to_owned(), columns);
                }
                let target_columns = &table_columns[target_table];
                // REFERENCES parent without a column list targets its ordered PK.
                let target_column = match row["to"].as_str().filter(|s| !s.is_empty()) {
                    Some(column) => target_columns.iter().find_map(|r| r["name"].as_str().filter(|name| name.eq_ignore_ascii_case(column))).unwrap_or(column).to_owned(),
                    None => {
                        let mut keys: Vec<_> = target_columns.iter().filter_map(|r| {
                            let position = r["pk"].as_u64()?;
                            if position == 0 { return None; }
                            Some((position, r["name"].as_str()?))
                        }).collect();
                        keys.sort_by_key(|(position, _)| *position);
                        let position = row["seq"].as_u64().ok_or_else(|| DriverError::QueryFailed("Foreign key has no column position".into()))? as usize;
                        keys.get(position).map(|(_, name)| (*name).to_owned())
                            .ok_or_else(|| DriverError::QueryFailed(format!("Cannot resolve the referenced primary key of {target_table}")))?
                    }
                };
                relationships.push(Relationship {
                    source_schema: None, target_schema: None,
                    source_table: table.name.clone(), source_column,
                    target_table: target_table.to_owned(), target_column,
                    relationship_type: Some("many-to-one".into()),
                });
            }
        }
        Ok(relationships)
    }

    async fn preview_table_data(&self, table: &str, _schema: Option<&str>, limit: i32) -> Result<Vec<Value>, DriverError> {
        self.query_rows(&format!("SELECT * FROM {} LIMIT {}", escape_sqlite_identifier(table), limit)).await
    }

    fn engine_name(&self) -> &str {
        "sqlite"
    }

    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::Sql
    }
}

fn escape_sqlite_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[cfg(test)]
mod read_only_tests {
    use super::*;
    #[tokio::test]
    async fn readonly_file_blocks_writes_and_preserves_data() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("readonly-{}.db", uuid::Uuid::new_v4()));
        let writable = SqliteDriver::new(path.to_str().unwrap()).unwrap();
        writable.execute_statement("CREATE TABLE fixture (value INTEGER)").await.unwrap();
        writable.execute_statement("INSERT INTO fixture VALUES (7)").await.unwrap();
        let readonly = SqliteDriver::new_with_policy(path.to_str().unwrap(), true).unwrap();
        assert_eq!(readonly.execute_query("SELECT value FROM fixture").await.unwrap()[0]["value"], 7);
        assert!(readonly.execute_statement("UPDATE fixture SET value = 9").await.is_err());
        assert_eq!(writable.execute_query("SELECT value FROM fixture").await.unwrap()[0]["value"], 7);
        drop(readonly); drop(writable); std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod relationship_tests {
    use super::*;

    #[tokio::test]
    async fn resolves_explicit_implicit_composite_and_self_references() {
        let driver = SqliteDriver::new(":memory:").unwrap();
        driver.conn.lock().await.execute_batch("
            CREATE TABLE parent (first TEXT, second TEXT, PRIMARY KEY(second, first));
            CREATE TABLE implicit_child (x TEXT, y TEXT, FOREIGN KEY(x,y) REFERENCES parent);
            CREATE TABLE named_parent (id INTEGER PRIMARY KEY);
            CREATE TABLE explicit_child (a INTEGER REFERENCES named_parent(id));
            CREATE TABLE tree (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES tree);
        ").unwrap();
        let edges = driver.get_relationships().await.unwrap();
        assert_eq!(edges.len(), 4);
        let target = |table: &str, column: &str| edges.iter().find(|e| e.source_table == table && e.source_column == column).unwrap().target_column.as_str();
        assert_eq!(target("implicit_child", "x"), "second");
        assert_eq!(target("implicit_child", "y"), "first");
        assert_eq!(target("explicit_child", "a"), "id");
        assert_eq!(target("tree", "parent_id"), "id");
        assert!(edges.iter().all(|e| e.source_schema.is_none() && e.target_schema.is_none()));
        let parent = driver.get_table_columns("parent", None).await.unwrap();
        assert!(parent.iter().all(|c| c.is_primary_key));
        let child = driver.get_table_columns("implicit_child", None).await.unwrap();
        assert!(child.iter().all(|c| c.is_foreign_key));
    }
    #[tokio::test]
    async fn canonicalizes_case_insensitive_foreign_key_identifiers() {
        let driver = SqliteDriver::new(":memory:").unwrap();
        driver.conn.lock().await.execute_batch("
            CREATE TABLE Parent (ID INTEGER PRIMARY KEY);
            CREATE TABLE Child (Owner_ID INTEGER, FOREIGN KEY(owner_id) REFERENCES parent(id));
        ").unwrap();
        let edges = driver.get_relationships().await.unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].source_table, "Child");
        assert_eq!(edges[0].source_column, "Owner_ID");
        assert_eq!(edges[0].target_table, "Parent");
        assert_eq!(edges[0].target_column, "ID");
    }

}
