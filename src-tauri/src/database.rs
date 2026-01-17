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
                let pool = mysql_async::Pool::new(conn_str);
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
                    .map_err(|e| DbError::QueryFailed(e.to_string()))?;
                let mut results = Vec::new();
                for row in rows {
                    let mut map = HashMap::new();
                    for (i, column) in row.columns().iter().enumerate() {
                        let value: Value = match column.type_() {
                            &tokio_postgres::types::Type::VARCHAR | &tokio_postgres::types::Type::TEXT => {
                                let s: String = row.get(i);
                                serde_json::to_value(s).unwrap()
                            }
                            &tokio_postgres::types::Type::INT4 => {
                                let s: i32 = row.get(i);
                                serde_json::to_value(s).unwrap()
                            }
                            _ => continue,
                        };
                        map.insert(column.name().to_string(), value);
                    }
                    results.push(map);
                }
                serde_json::to_string(&results).map_err(|e| DbError::QueryFailed(e.to_string()))
            }
            DbConnection::Mysql(_conn) => {
                // Not yet implemented
                let mock_data = r#"
        [
            { "id": 1, "name": "mock data" },
            { "id": 2, "name": "mock data" }
        ]
        "#;
                Ok(mock_data.to_string())
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
                                rusqlite::types::ValueRef::Integer(i) => serde_json::to_value(i).unwrap(),
                                rusqlite::types::ValueRef::Real(f) => serde_json::to_value(f).unwrap(),
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
                let pool = mysql_async::Pool::new(conn_str);
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

#[command]
pub async fn query_db(engine: &str, conn_str: &str, query: &str) -> Result<String, String> {
    let mut conn = DbConnection::new(engine, conn_str)
        .await
        .map_err(|e| e.to_string())?;

    conn.query(query).await.map_err(|e| e.to_string())
}

#[command]
pub async fn test_connection(engine: &str, conn_str: &str) -> Result<bool, String> {
    DbConnection::test(engine, conn_str)
        .await
        .map_err(|e| e.to_string())
}
