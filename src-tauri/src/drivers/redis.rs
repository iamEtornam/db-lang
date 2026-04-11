use async_trait::async_trait;
use redis::AsyncCommands;
use serde_json::{json, Value};
use std::collections::HashMap;

use super::{ColumnInfo, DatabaseDriver, DriverError, PaginatedResult, QueryLanguage, Relationship, TableInfo};

pub struct RedisDriver {
    client: redis::Client,
    conn: tokio::sync::Mutex<redis::aio::MultiplexedConnection>,
}

impl RedisDriver {
    pub async fn new(conn_str: &str) -> Result<Self, DriverError> {
        let client = redis::Client::open(conn_str)
            .map_err(|e| DriverError::ConnectionFailed(e.to_string()))?;
        let conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| DriverError::ConnectionFailed(e.to_string()))?;

        Ok(Self { client, conn: tokio::sync::Mutex::new(conn) })
    }

    async fn execute_redis_command(&self, command: &str) -> Result<Value, DriverError> {
        let mut conn = self.conn.lock().await;
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(DriverError::QueryFailed("Empty command".to_string()));
        }

        let cmd_name = parts[0].to_uppercase();
        let args = &parts[1..];

        let result: redis::RedisResult<redis::Value> = match cmd_name.as_str() {
            "GET" => {
                let val: redis::RedisResult<Option<String>> = conn.get(args[0]).await;
                val.map(|v| redis::Value::BulkString(v.unwrap_or_default().into_bytes()))
            }
            "SET" => {
                let val: redis::RedisResult<()> = conn.set(args[0], args.get(1).copied().unwrap_or("")).await;
                val.map(|_| redis::Value::SimpleString("OK".to_string()))
            }
            "KEYS" => {
                let pattern = args.get(0).copied().unwrap_or("*");
                let val: redis::RedisResult<Vec<String>> = conn.keys(pattern).await;
                val.map(|v| redis::Value::Array(v.into_iter().map(|s| redis::Value::BulkString(s.into_bytes())).collect()))
            }
            "HGETALL" => {
                let val: redis::RedisResult<Vec<(String, String)>> = conn.hgetall(args[0]).await;
                val.map(|v| {
                    let mut pairs = Vec::new();
                    for (k, vv) in v {
                        pairs.push(redis::Value::BulkString(k.into_bytes()));
                        pairs.push(redis::Value::BulkString(vv.into_bytes()));
                    }
                    redis::Value::Array(pairs)
                })
            }
            "LRANGE" => {
                let start = args.get(1).and_then(|s| s.parse::<isize>().ok()).unwrap_or(0);
                let stop = args.get(2).and_then(|s| s.parse::<isize>().ok()).unwrap_or(-1);
                let val: redis::RedisResult<Vec<String>> = conn.lrange(args[0], start, stop).await;
                val.map(|v| redis::Value::Array(v.into_iter().map(|s| redis::Value::BulkString(s.into_bytes())).collect()))
            }
            "SMEMBERS" => {
                let val: redis::RedisResult<Vec<String>> = conn.smembers(args[0]).await;
                val.map(|v| redis::Value::Array(v.into_iter().map(|s| redis::Value::BulkString(s.into_bytes())).collect()))
            }
            "TTL" => {
                let val: redis::RedisResult<i64> = conn.ttl(args[0]).await;
                val.map(|v| redis::Value::Int(v))
            }
            "TYPE" => {
                let val: redis::RedisResult<String> = conn.key_type(args[0]).await;
                val.map(|v| redis::Value::SimpleString(v))
            }
            "DBSIZE" => {
                let val: redis::RedisResult<redis::Value> = redis::cmd("DBSIZE").query_async(&mut *conn).await;
                val
            }
            "INFO" => {
                let val: redis::RedisResult<redis::Value> = redis::cmd("INFO").query_async(&mut *conn).await;
                val
            }
            _ => {
                return Err(DriverError::QueryFailed(format!("Unsupported Redis command: {}", cmd_name)))
            }
        };

        redis_value_to_json(result.map_err(|e| DriverError::QueryFailed(e.to_string()))?)
    }
}

fn redis_value_to_json(val: redis::Value) -> Result<Value, DriverError> {
    match val {
        redis::Value::Nil => Ok(Value::Null),
        redis::Value::Int(i) => Ok(json!(i)),
        redis::Value::BulkString(b) => Ok(Value::String(String::from_utf8_lossy(&b).to_string())),
        redis::Value::Array(arr) => {
            let values: Result<Vec<Value>, _> = arr.into_iter().map(redis_value_to_json).collect();
            Ok(Value::Array(values?))
        }
        redis::Value::SimpleString(s) => Ok(Value::String(s)),
        redis::Value::Boolean(b) => Ok(Value::Bool(b)),
        redis::Value::Double(f) => Ok(json!(f)),
        redis::Value::BigNumber(n) => Ok(Value::String(n.to_string())),
        redis::Value::Map(pairs) => {
            let mut map = serde_json::Map::new();
            for (k, v) in pairs {
                let key = match redis_value_to_json(k)? {
                    Value::String(s) => s,
                    v => v.to_string(),
                };
                map.insert(key, redis_value_to_json(v)?);
            }
            Ok(Value::Object(map))
        }
        redis::Value::Set(members) => {
            let values: Result<Vec<Value>, _> = members.into_iter().map(redis_value_to_json).collect();
            Ok(Value::Array(values?))
        }
        redis::Value::Okay => Ok(Value::String("OK".to_string())),
        _ => Ok(Value::Null),
    }
}

#[async_trait]
impl DatabaseDriver for RedisDriver {
    async fn execute_query(&self, query: &str) -> Result<Vec<Value>, DriverError> {
        let commands: Vec<&str> = query.lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();

        let mut results = Vec::new();
        for cmd in commands {
            let result = self.execute_redis_command(cmd).await?;
            results.push(json!({
                "command": cmd,
                "result": result
            }));
        }
        Ok(results)
    }

    async fn execute_query_paginated(
        &self,
        query: &str,
        page: i32,
        page_size: i32,
    ) -> Result<PaginatedResult, DriverError> {
        let rows = self.execute_query(query).await?;
        let total = rows.len() as i64;
        let data = serde_json::to_string(&rows)
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

        Ok(PaginatedResult {
            data,
            total_count: Some(total),
            page,
            page_size,
            has_more: false,
        })
    }

    async fn test_connection(&self) -> Result<bool, DriverError> {
        let mut conn = self.conn.lock().await;
        let _: () = redis::cmd("PING")
            .query_async(&mut *conn)
            .await
            .map_err(|e| DriverError::ConnectionFailed(e.to_string()))?;
        Ok(true)
    }

    async fn get_tables(&self) -> Result<Vec<TableInfo>, DriverError> {
        let result = self.execute_redis_command("KEYS *").await?;
        let keys: Vec<String> = match result {
            Value::Array(arr) => arr.into_iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect(),
            _ => vec![],
        };

        // Group by prefix (first segment before :)
        let mut prefixes: HashMap<String, usize> = HashMap::new();
        for key in &keys {
            let prefix = key.split(':').next().unwrap_or(key).to_string();
            *prefixes.entry(prefix).or_insert(0) += 1;
        }

        let mut tables: Vec<TableInfo> = prefixes.into_iter().map(|(prefix, count)| TableInfo {
            name: prefix,
            schema: None,
            table_type: format!("KEY_GROUP ({} keys)", count),
        }).collect();
        tables.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tables)
    }

    async fn get_table_columns(&self, table: &str, _schema: Option<&str>) -> Result<Vec<ColumnInfo>, DriverError> {
        // For a key prefix group, return generic columns
        Ok(vec![
            ColumnInfo {
                name: "key".to_string(),
                data_type: "string".to_string(),
                is_nullable: false,
                column_default: None,
                is_primary_key: true,
                is_foreign_key: false,
                referenced_table: None,
                referenced_column: None,
            },
            ColumnInfo {
                name: "type".to_string(),
                data_type: "string".to_string(),
                is_nullable: false,
                column_default: None,
                is_primary_key: false,
                is_foreign_key: false,
                referenced_table: None,
                referenced_column: None,
            },
            ColumnInfo {
                name: "value".to_string(),
                data_type: "mixed".to_string(),
                is_nullable: true,
                column_default: None,
                is_primary_key: false,
                is_foreign_key: false,
                referenced_table: None,
                referenced_column: None,
            },
        ])
    }

    async fn get_relationships(&self) -> Result<Vec<Relationship>, DriverError> {
        Ok(vec![])
    }

    async fn preview_table_data(&self, table: &str, _schema: Option<&str>, limit: i32) -> Result<Vec<Value>, DriverError> {
        let pattern = format!("{}:*", table);
        let result = self.execute_redis_command(&format!("KEYS {}", pattern)).await?;

        let keys = match result {
            Value::Array(arr) => arr.into_iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>(),
            _ => vec![],
        };

        let mut rows = Vec::new();
        for key in keys.iter().take(limit as usize) {
            let type_result = self.execute_redis_command(&format!("TYPE {}", key)).await?;
            let type_str = type_result.as_str().unwrap_or("unknown").to_string();
            let value = self.execute_redis_command(&format!("GET {}", key)).await.ok();

            rows.push(json!({
                "key": key,
                "type": type_str,
                "value": value
            }));
        }
        Ok(rows)
    }

    fn engine_name(&self) -> &str {
        "redis"
    }

    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::Redis
    }
}
