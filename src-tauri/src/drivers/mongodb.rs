use async_trait::async_trait;
use mongodb::{bson::doc, options::ClientOptions, Client};
use serde_json::Value;
use std::collections::HashMap;

use super::{ColumnInfo, DatabaseDriver, DriverError, PaginatedResult, QueryLanguage, Relationship, TableInfo};

pub struct MongoDriver {
    client: Client,
    db_name: String,
}

impl MongoDriver {
    pub async fn new(conn_str: &str) -> Result<Self, DriverError> {
        let mut opts = ClientOptions::parse(conn_str)
            .await
            .map_err(|e| DriverError::ConnectionFailed(e.to_string()))?;
        opts.server_selection_timeout = Some(std::time::Duration::from_secs(5));

        let client = Client::with_options(opts)
            .map_err(|e| DriverError::ConnectionFailed(e.to_string()))?;

        // Extract db name from connection string
        let db_name = extract_db_name(conn_str).unwrap_or_else(|| "test".to_string());

        Ok(Self { client, db_name })
    }

    fn db(&self) -> mongodb::Database {
        self.client.database(&self.db_name)
    }

    async fn bson_to_json(doc: &mongodb::bson::Document) -> Value {
        let mut map = serde_json::Map::new();
        for (key, val) in doc.iter() {
            map.insert(key.clone(), bson_value_to_json(val));
        }
        Value::Object(map)
    }
}

fn extract_db_name(conn_str: &str) -> Option<String> {
    // mongodb://user:pass@host:port/dbname
    let url = url::Url::parse(conn_str).ok()?;
    let path = url.path().trim_start_matches('/');
    if path.is_empty() { None } else { Some(path.to_string()) }
}

fn bson_value_to_json(val: &mongodb::bson::Bson) -> Value {
    match val {
        mongodb::bson::Bson::Double(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        mongodb::bson::Bson::String(s) => Value::String(s.clone()),
        mongodb::bson::Bson::Array(arr) => Value::Array(arr.iter().map(bson_value_to_json).collect()),
        mongodb::bson::Bson::Document(doc) => {
            let mut map = serde_json::Map::new();
            for (k, v) in doc.iter() {
                map.insert(k.clone(), bson_value_to_json(v));
            }
            Value::Object(map)
        }
        mongodb::bson::Bson::Boolean(b) => Value::Bool(*b),
        mongodb::bson::Bson::Null => Value::Null,
        mongodb::bson::Bson::Int32(i) => Value::Number((*i).into()),
        mongodb::bson::Bson::Int64(i) => Value::Number((*i).into()),
        mongodb::bson::Bson::ObjectId(oid) => Value::String(oid.to_hex()),
        mongodb::bson::Bson::DateTime(dt) => Value::String(dt.to_string()),
        _ => Value::String(val.to_string()),
    }
}

async fn infer_schema_from_docs(docs: &[mongodb::bson::Document]) -> Vec<ColumnInfo> {
    let mut field_types: HashMap<String, String> = HashMap::new();

    for doc in docs {
        for (key, val) in doc.iter() {
            let type_str = match val {
                mongodb::bson::Bson::String(_) => "string",
                mongodb::bson::Bson::Int32(_) | mongodb::bson::Bson::Int64(_) => "integer",
                mongodb::bson::Bson::Double(_) => "double",
                mongodb::bson::Bson::Boolean(_) => "boolean",
                mongodb::bson::Bson::Array(_) => "array",
                mongodb::bson::Bson::Document(_) => "object",
                mongodb::bson::Bson::ObjectId(_) => "objectId",
                mongodb::bson::Bson::DateTime(_) => "date",
                mongodb::bson::Bson::Null => "null",
                _ => "mixed",
            };
            field_types.entry(key.clone()).or_insert_with(|| type_str.to_string());
        }
    }

    field_types.into_iter().map(|(name, data_type)| {
        let is_pk = name == "_id";
        ColumnInfo {
            name,
            data_type,
            is_nullable: true,
            column_default: None,
            is_primary_key: is_pk,
            is_foreign_key: false,
            referenced_table: None,
            referenced_column: None,
        }
    }).collect()
}

#[async_trait]
impl DatabaseDriver for MongoDriver {
    async fn execute_query(&self, query: &str) -> Result<Vec<Value>, DriverError> {
        // Parse query: expected format is `collection_name` or JSON pipeline
        let trimmed = query.trim();

        // If it starts with JSON array, treat as aggregation pipeline on last selected collection
        if trimmed.starts_with('[') || trimmed.starts_with('{') {
            return Err(DriverError::QueryFailed(
                "MongoDB queries require a collection context. Use format: <collection>.<json_pipeline>".to_string()
            ));
        }

        // Format: collectionName or collectionName.[{pipeline}]
        let parts: Vec<&str> = trimmed.splitn(2, '.').collect();
        let collection_name = parts[0].trim().trim_matches('`').trim_matches('"');
        let db = self.db();
        let collection = db.collection::<mongodb::bson::Document>(collection_name);

        if parts.len() > 1 {
            // Aggregation pipeline
            let pipeline_str = parts[1].trim();
            let pipeline_json: Value = serde_json::from_str(pipeline_str)
                .map_err(|e| DriverError::QueryFailed(format!("Invalid pipeline JSON: {}", e)))?;
            let arr = pipeline_json.as_array()
                .ok_or_else(|| DriverError::QueryFailed("Pipeline must be a JSON array".to_string()))?;
            let pipeline: Vec<mongodb::bson::Document> = arr.iter()
                .filter_map(|v| {
                    serde_json::to_vec(v).ok()
                        .and_then(|bytes| mongodb::bson::from_slice(&bytes).ok())
                })
                .collect();

            let mut cursor = collection.aggregate(pipeline)
                .await
                .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

            let mut results = Vec::new();
            while cursor.advance().await.map_err(|e| DriverError::QueryFailed(e.to_string()))? {
                let doc = cursor.deserialize_current()
                    .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
                results.push(MongoDriver::bson_to_json(&doc).await);
            }
            Ok(results)
        } else {
            // Simple find all
            let mut cursor = collection.find(doc! {})
                .limit(1000)
                .await
                .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

            let mut results = Vec::new();
            while cursor.advance().await.map_err(|e| DriverError::QueryFailed(e.to_string()))? {
                let doc = cursor.deserialize_current()
                    .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
                results.push(MongoDriver::bson_to_json(&doc).await);
            }
            Ok(results)
        }
    }

    async fn execute_query_paginated(
        &self,
        query: &str,
        page: i32,
        page_size: i32,
    ) -> Result<PaginatedResult, DriverError> {
        let rows = self.execute_query(query).await?;
        let total = rows.len() as i64;
        let start = ((page - 1) * page_size) as usize;
        let end = (start + page_size as usize).min(rows.len());
        let page_rows = rows[start..end].to_vec();
        let has_more = end < rows.len();
        let data = serde_json::to_string(&page_rows)
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

        Ok(PaginatedResult {
            data,
            total_count: Some(total),
            page,
            page_size,
            has_more,
        })
    }

    async fn test_connection(&self) -> Result<bool, DriverError> {
        self.client
            .database("admin")
            .run_command(doc! { "ping": 1 })
            .await
            .map(|_| true)
            .map_err(|e| DriverError::ConnectionFailed(e.to_string()))
    }

    async fn get_tables(&self) -> Result<Vec<TableInfo>, DriverError> {
        let db = self.db();
        let collections = db
            .list_collection_names()
            .await
            .map_err(|e| DriverError::SchemaError(e.to_string()))?;

        Ok(collections.into_iter().map(|name| TableInfo {
            name,
            schema: None,
            table_type: "COLLECTION".to_string(),
        }).collect())
    }

    async fn get_table_columns(&self, table: &str, _schema: Option<&str>) -> Result<Vec<ColumnInfo>, DriverError> {
        let db = self.db();
        let collection = db.collection::<mongodb::bson::Document>(table);

        // Sample up to 20 documents to infer schema
        let mut cursor = collection.find(doc! {})
            .limit(20)
            .await
            .map_err(|e| DriverError::SchemaError(e.to_string()))?;

        let mut docs = Vec::new();
        while cursor.advance().await.map_err(|e| DriverError::SchemaError(e.to_string()))? {
            if let Ok(doc) = cursor.deserialize_current() {
                docs.push(doc);
            }
        }

        Ok(infer_schema_from_docs(&docs).await)
    }

    async fn get_relationships(&self) -> Result<Vec<Relationship>, DriverError> {
        // MongoDB doesn't have formal FK relationships, return empty
        Ok(vec![])
    }

    async fn preview_table_data(&self, table: &str, _schema: Option<&str>, limit: i32) -> Result<Vec<Value>, DriverError> {
        let db = self.db();
        let collection = db.collection::<mongodb::bson::Document>(table);

        let mut cursor = collection.find(doc! {})
            .limit(limit as i64)
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

        let mut results = Vec::new();
        while cursor.advance().await.map_err(|e| DriverError::QueryFailed(e.to_string()))? {
            let doc = cursor.deserialize_current()
                .map_err(|e| DriverError::QueryFailed(e.to_string()))?;
            results.push(MongoDriver::bson_to_json(&doc).await);
        }
        Ok(results)
    }

    fn engine_name(&self) -> &str {
        "mongodb"
    }

    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::Mql
    }
}
