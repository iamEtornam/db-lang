use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::Emitter;
use uuid::Uuid;

use super::firebase_auth::{FirebaseAuth, FirebaseConnBlob, ServiceAccount};
use super::{ColumnInfo, DatabaseDriver, DriverError, PaginatedResult, QueryLanguage, Relationship, TableInfo};

const RTDB_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/firebase.database",
    "https://www.googleapis.com/auth/userinfo.email",
];

pub struct RtdbDriver {
    auth: Arc<FirebaseAuth>,
    database_url: String,
    http: reqwest::Client,
}

impl RtdbDriver {
    pub async fn new(conn_str: &str) -> Result<Self, DriverError> {
        let blob = FirebaseConnBlob::decode(conn_str)?;
        let sa = ServiceAccount::from_json(&blob.auth_json)?;
        let auth = Arc::new(FirebaseAuth::new(sa));

        let database_url = blob.database_url.trim_end_matches('/').to_string();
        if database_url.is_empty() {
            return Err(DriverError::ConnectionFailed(
                "Realtime Database URL is required".into(),
            ));
        }

        auth.access_token(RTDB_SCOPES).await?;

        Ok(Self {
            auth,
            database_url,
            http: reqwest::Client::new(),
        })
    }

    async fn authed_get(&self, path: &str, params: &str) -> Result<Value, DriverError> {
        let token = self.auth.access_token(RTDB_SCOPES).await?;
        let sep = if params.is_empty() { "?" } else { "&" };
        let url = if params.is_empty() {
            format!("{}/{}.json?access_token={}", self.database_url, path, token)
        } else {
            format!(
                "{}/{}.json?{}{}access_token={}",
                self.database_url, path, params, sep, token
            )
        };

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(DriverError::QueryFailed(format!(
                "RTDB GET failed ({}): {}",
                status, body
            )));
        }

        resp.json()
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))
    }

    fn flatten_object_to_rows(obj: &Value) -> Vec<Value> {
        match obj {
            Value::Object(map) => {
                map.iter()
                    .map(|(key, val)| {
                        let mut row = Map::new();
                        row.insert("_key".to_string(), json!(key));
                        match val {
                            Value::Object(child) => {
                                for (k, v) in child {
                                    row.insert(k.clone(), v.clone());
                                }
                            }
                            other => {
                                row.insert("_value".to_string(), other.clone());
                            }
                        }
                        Value::Object(row)
                    })
                    .collect()
            }
            Value::Array(arr) => {
                arr.iter()
                    .enumerate()
                    .map(|(i, val)| {
                        let mut row = Map::new();
                        row.insert("_index".to_string(), json!(i));
                        match val {
                            Value::Object(child) => {
                                for (k, v) in child {
                                    row.insert(k.clone(), v.clone());
                                }
                            }
                            other => {
                                row.insert("_value".to_string(), other.clone());
                            }
                        }
                        Value::Object(row)
                    })
                    .collect()
            }
            Value::Null => vec![],
            other => {
                vec![json!({ "_value": other })]
            }
        }
    }

    pub fn auth_ref(&self) -> &Arc<FirebaseAuth> {
        &self.auth
    }

    pub fn database_url(&self) -> &str {
        &self.database_url
    }
}

#[async_trait]
impl DatabaseDriver for RtdbDriver {
    async fn execute_query(&self, query: &str) -> Result<Vec<Value>, DriverError> {
        let trimmed = query.trim().trim_start_matches('/');

        let (path, params) = if let Some(idx) = trimmed.find('?') {
            (&trimmed[..idx], &trimmed[idx + 1..])
        } else {
            (trimmed, "")
        };

        let data = self.authed_get(path, params).await?;
        Ok(Self::flatten_object_to_rows(&data))
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
        self.authed_get("", "shallow=true").await?;
        Ok(true)
    }

    async fn get_tables(&self) -> Result<Vec<TableInfo>, DriverError> {
        let data = self.authed_get("", "shallow=true").await?;
        match data {
            Value::Object(map) => Ok(map
                .keys()
                .map(|name| TableInfo {
                    name: name.clone(),
                    schema: None,
                    table_type: "NODE".to_string(),
                })
                .collect()),
            _ => Ok(vec![]),
        }
    }

    async fn get_table_columns(
        &self,
        table: &str,
        _schema: Option<&str>,
    ) -> Result<Vec<ColumnInfo>, DriverError> {
        let data = self
            .authed_get(table, "limitToFirst=20")
            .await?;
        let rows = Self::flatten_object_to_rows(&data);

        let mut field_types: HashMap<String, String> = HashMap::new();
        for row in &rows {
            if let Some(obj) = row.as_object() {
                for (key, val) in obj {
                    let type_str = match val {
                        Value::Null => "null",
                        Value::Bool(_) => "boolean",
                        Value::Number(n) => {
                            if n.is_f64() { "double" } else { "integer" }
                        }
                        Value::String(_) => "string",
                        Value::Array(_) => "array",
                        Value::Object(_) => "object",
                    };
                    field_types
                        .entry(key.clone())
                        .or_insert_with(|| type_str.to_string());
                }
            }
        }

        Ok(field_types
            .into_iter()
            .map(|(name, data_type)| {
                let is_pk = name == "_key";
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
            })
            .collect())
    }

    async fn get_relationships(&self) -> Result<Vec<Relationship>, DriverError> {
        Ok(vec![])
    }

    async fn preview_table_data(
        &self,
        table: &str,
        _schema: Option<&str>,
        limit: i32,
    ) -> Result<Vec<Value>, DriverError> {
        let data = self
            .authed_get(table, &format!("limitToFirst={}", limit))
            .await?;
        Ok(Self::flatten_object_to_rows(&data))
    }

    fn engine_name(&self) -> &str {
        "firebase_rtdb"
    }

    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::FirebaseRtdb
    }
}

// ============ SSE Streaming for Realtime Database ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtdbEvent {
    pub kind: String,
    pub path: String,
    pub data: Value,
}

struct SubscriptionEntry {
    abort: tokio::sync::oneshot::Sender<()>,
}

static SUBSCRIPTIONS: OnceLock<Mutex<HashMap<String, SubscriptionEntry>>> = OnceLock::new();

fn subscriptions() -> &'static Mutex<HashMap<String, SubscriptionEntry>> {
    SUBSCRIPTIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn subscribe_to_path(
    database_url: &str,
    auth: &Arc<FirebaseAuth>,
    path: &str,
    app: tauri::AppHandle,
) -> Result<String, DriverError> {
    let token = auth.access_token(RTDB_SCOPES).await?;
    let clean_path = path.trim().trim_start_matches('/');
    let url = format!(
        "{}/{}.json?access_token={}",
        database_url.trim_end_matches('/'),
        clean_path,
        token
    );

    let sub_id = Uuid::new_v4().to_string();
    let event_name = format!("rtdb:event:{}", sub_id);

    let (abort_tx, mut abort_rx) = tokio::sync::oneshot::channel::<()>();

    {
        let mut subs = subscriptions().lock().unwrap();
        subs.insert(sub_id.clone(), SubscriptionEntry { abort: abort_tx });
    }

    let sub_id_clone = sub_id.clone();
    let owned_path = clean_path.to_string();

    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let resp = match client
            .get(&url)
            .header("Accept", "text/event-stream")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let evt = RtdbEvent {
                    kind: "error".into(),
                    path: owned_path.clone(),
                    data: json!(e.to_string()),
                };
                let _ = app.emit(&event_name, evt);
                let mut subs = subscriptions().lock().unwrap();
                subs.remove(&sub_id_clone);
                return;
            }
        };

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();

        loop {
            tokio::select! {
                _ = &mut abort_rx => {
                    break;
                }
                chunk = stream.next() => {
                    match chunk {
                        Some(Ok(ref bytes)) => {
                            buffer.push_str(&String::from_utf8_lossy(bytes));

                            while let Some(pos) = buffer.find("\n\n") {
                                let block = buffer[..pos].to_string();
                                buffer = buffer[pos + 2..].to_string();

                                let mut event_type = String::new();
                                let mut event_data = String::new();

                                for line in block.lines() {
                                    if let Some(val) = line.strip_prefix("event: ") {
                                        event_type = val.trim().to_string();
                                    } else if let Some(val) = line.strip_prefix("data: ") {
                                        event_data = val.trim().to_string();
                                    }
                                }

                                if event_type == "keep-alive" {
                                    continue;
                                }

                                if !event_type.is_empty() && !event_data.is_empty() {
                                    if let Ok(parsed) = serde_json::from_str::<Value>(&event_data) {
                                        let evt_path = parsed
                                            .get("path")
                                            .and_then(|p| p.as_str())
                                            .unwrap_or("/")
                                            .to_string();
                                        let evt_data = parsed
                                            .get("data")
                                            .cloned()
                                            .unwrap_or(Value::Null);

                                        let _ = app.emit(
                                            &event_name,
                                            RtdbEvent {
                                                kind: event_type.clone(),
                                                path: evt_path,
                                                data: evt_data,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                        Some(Err(_)) | None => {
                            break;
                        }
                    }
                }
            }
        }

        let mut subs = subscriptions().lock().unwrap();
        subs.remove(&sub_id_clone);
    });

    Ok(sub_id)
}

pub fn unsubscribe(sub_id: &str) -> Result<(), DriverError> {
    let mut subs = subscriptions().lock().unwrap();
    if let Some(entry) = subs.remove(sub_id) {
        let _ = entry.abort.send(());
        Ok(())
    } else {
        Err(DriverError::QueryFailed(format!(
            "Subscription '{}' not found",
            sub_id
        )))
    }
}
