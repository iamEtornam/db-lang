use async_trait::async_trait;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// Default page size when a Firestore query specifies a collection name with
/// no structured query. Users who need more rows can pass a `limit` in the
/// JSON tail (e.g. `Profiles.{"limit":500}`).
const DEFAULT_LIST_PAGE_SIZE: i32 = 100;

use super::firebase_auth::{FirebaseAuth, FirebaseConnBlob, ServiceAccount};
use super::{ColumnInfo, DatabaseDriver, DriverError, PaginatedResult, QueryLanguage, Relationship, TableInfo};

const FIRESTORE_SCOPES: &[&str] = &["https://www.googleapis.com/auth/datastore"];

pub struct FirestoreDriver {
    auth: FirebaseAuth,
    project_id: String,
    database_id: String,
    http: reqwest::Client,
}

impl FirestoreDriver {
    pub async fn new(conn_str: &str) -> Result<Self, DriverError> {
        let blob = FirebaseConnBlob::decode(conn_str)?;
        let sa = ServiceAccount::from_json(&blob.auth_json)?;
        let project_id = sa.project_id.clone();
        let auth = FirebaseAuth::new(sa)?;

        let db_id = if blob.firestore_db_id.is_empty() {
            "(default)".to_string()
        } else {
            blob.firestore_db_id
        };

        let driver = Self {
            auth,
            project_id,
            database_id: db_id,
            http: reqwest::Client::new(),
        };

        driver.auth.access_token(FIRESTORE_SCOPES).await?;

        Ok(driver)
    }

    fn base_url(&self) -> String {
        format!(
            "https://firestore.googleapis.com/v1/projects/{}/databases/{}/documents",
            self.project_id, self.database_id
        )
    }

    async fn authed_get(&self, url: &str) -> Result<Value, DriverError> {
        let token = self.auth.access_token(FIRESTORE_SCOPES).await?;
        let resp = self
            .http
            .get(url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(DriverError::QueryFailed(format!("Firestore GET {} failed ({}): {}", url, status, body)));
        }

        resp.json().await.map_err(|e| DriverError::QueryFailed(e.to_string()))
    }

    async fn authed_post(&self, url: &str, body: &Value) -> Result<Value, DriverError> {
        let token = self.auth.access_token(FIRESTORE_SCOPES).await?;
        let resp = self
            .http
            .post(url)
            .bearer_auth(&token)
            .json(body)
            .send()
            .await
            .map_err(|e| DriverError::QueryFailed(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(DriverError::QueryFailed(format!("Firestore POST {} failed ({}): {}", url, status, body_text)));
        }

        resp.json().await.map_err(|e| DriverError::QueryFailed(e.to_string()))
    }

    async fn list_collection_ids(&self) -> Result<Vec<String>, DriverError> {
        let url = format!("{}:listCollectionIds", self.base_url());
        let body = json!({});
        let resp = self.authed_post(&url, &body).await?;

        let ids = resp
            .get("collectionIds")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        Ok(ids)
    }

    async fn list_documents(
        &self,
        collection: &str,
        page_size: i32,
    ) -> Result<Vec<Value>, DriverError> {
        let url = format!("{}/{}?pageSize={}", self.base_url(), collection, page_size);
        let resp = self.authed_get(&url).await?;

        let docs = resp
            .get("documents")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        Ok(docs.into_iter().map(|d| firestore_doc_to_json(&d)).collect())
    }

    async fn run_query(
        &self,
        collection: &str,
        query_json: &Value,
    ) -> Result<Vec<Value>, DriverError> {
        let url = format!("{}:runQuery", self.base_url());

        let mut structured_query = Map::new();
        structured_query.insert(
            "from".to_string(),
            json!([{ "collectionId": collection }]),
        );

        if let Some(where_clause) = query_json.get("where") {
            structured_query.insert("where".to_string(), where_clause.clone());
        }
        if let Some(order_by) = query_json.get("orderBy") {
            structured_query.insert("orderBy".to_string(), order_by.clone());
        }
        if let Some(limit) = query_json.get("limit") {
            structured_query.insert("limit".to_string(), limit.clone());
        }
        if let Some(offset) = query_json.get("offset") {
            structured_query.insert("offset".to_string(), offset.clone());
        }
        if let Some(select) = query_json.get("select") {
            structured_query.insert("select".to_string(), select.clone());
        }

        let body = json!({ "structuredQuery": structured_query });
        let resp = self.authed_post(&url, &body).await?;

        let results = resp
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![resp.clone()]);

        let mut rows = Vec::new();
        for item in results {
            if let Some(doc) = item.get("document") {
                rows.push(firestore_doc_to_json(doc));
            }
        }

        Ok(rows)
    }
}

fn firestore_value_to_json(val: &Value) -> Value {
    if let Some(s) = val.get("stringValue") {
        return s.clone();
    }
    if let Some(i) = val.get("integerValue") {
        if let Some(s) = i.as_str() {
            if let Ok(n) = s.parse::<i64>() {
                return json!(n);
            }
        }
        return i.clone();
    }
    if let Some(d) = val.get("doubleValue") {
        return d.clone();
    }
    if let Some(b) = val.get("booleanValue") {
        return b.clone();
    }
    if val.get("nullValue").is_some() {
        return Value::Null;
    }
    if let Some(ts) = val.get("timestampValue") {
        return ts.clone();
    }
    if let Some(arr) = val.get("arrayValue").and_then(|a| a.get("values")).and_then(|v| v.as_array()) {
        return Value::Array(arr.iter().map(firestore_value_to_json).collect());
    }
    if let Some(map) = val.get("mapValue").and_then(|m| m.get("fields")).and_then(|f| f.as_object()) {
        let mut out = Map::new();
        for (k, v) in map {
            out.insert(k.clone(), firestore_value_to_json(v));
        }
        return Value::Object(out);
    }
    if let Some(ref_val) = val.get("referenceValue") {
        return ref_val.clone();
    }
    if let Some(geo) = val.get("geoPointValue") {
        return geo.clone();
    }
    if let Some(bytes) = val.get("bytesValue") {
        return bytes.clone();
    }
    val.clone()
}

fn firestore_doc_to_json(doc: &Value) -> Value {
    let mut row = Map::new();

    if let Some(name) = doc.get("name").and_then(|n| n.as_str()) {
        let doc_id = name.rsplit('/').next().unwrap_or(name);
        row.insert("_id".to_string(), json!(doc_id));
    }

    if let Some(fields) = doc.get("fields").and_then(|f| f.as_object()) {
        for (k, v) in fields {
            row.insert(k.clone(), firestore_value_to_json(v));
        }
    }

    if let Some(ct) = doc.get("createTime") {
        row.insert("_createTime".to_string(), ct.clone());
    }
    if let Some(ut) = doc.get("updateTime") {
        row.insert("_updateTime".to_string(), ut.clone());
    }

    Value::Object(row)
}

fn infer_type(val: &Value) -> &'static str {
    match val {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) => {
            if n.is_f64() { "double" } else { "integer" }
        }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "map",
    }
}

fn infer_columns_from_docs(docs: &[Value]) -> Vec<ColumnInfo> {
    let mut field_types: HashMap<String, String> = HashMap::new();

    for doc in docs {
        if let Some(obj) = doc.as_object() {
            for (key, val) in obj {
                field_types
                    .entry(key.clone())
                    .or_insert_with(|| infer_type(val).to_string());
            }
        }
    }

    field_types
        .into_iter()
        .map(|(name, data_type)| {
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
        })
        .collect()
}

/// Validate that a query string looks like a Firestore query
/// (`collectionName` or `collectionName.{...json...}`) rather than something
/// else (most commonly a stray SQL statement). Returns the collection name
/// and optional structured-query JSON tail.
pub(crate) fn parse_firestore_query(query: &str) -> Result<(String, Option<String>), DriverError> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err(DriverError::QueryFailed("Empty query".to_string()));
    }

    let upper = trimmed.to_uppercase();
    const SQL_KEYWORDS: &[&str] = &[
        "SELECT ", "INSERT ", "UPDATE ", "DELETE ", "DROP ", "ALTER ", "CREATE ", "TRUNCATE ",
        "WITH ",
    ];
    if SQL_KEYWORDS.iter().any(|kw| upper.starts_with(kw)) {
        return Err(DriverError::QueryFailed(format!(
            "Firestore does not support SQL. Got: `{}`. Expected `collectionName` or `collectionName.{{...structuredQuery JSON...}}`.",
            trimmed
        )));
    }

    let (head, tail) = match trimmed.find('.') {
        Some(idx) => (&trimmed[..idx], Some(trimmed[idx + 1..].trim().to_string())),
        None => (trimmed, None),
    };

    let collection_name = head.trim().trim_matches('`').trim_matches('"').to_string();
    if collection_name.is_empty() {
        return Err(DriverError::QueryFailed("Missing collection name".into()));
    }
    let valid = collection_name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-');
    if !valid {
        return Err(DriverError::QueryFailed(format!(
            "Invalid Firestore collection name: `{}`. Expected `collectionName` or `collectionName.{{...structuredQuery JSON...}}`.",
            collection_name
        )));
    }

    Ok((collection_name, tail))
}

#[async_trait]
impl DatabaseDriver for FirestoreDriver {
    async fn execute_query(&self, query: &str) -> Result<Vec<Value>, DriverError> {
        let (collection_name, tail) = parse_firestore_query(query)?;

        if let Some(query_str) = tail {
            let query_json: Value = serde_json::from_str(&query_str)
                .map_err(|e| DriverError::QueryFailed(format!("Invalid query JSON: {}", e)))?;
            self.run_query(&collection_name, &query_json).await
        } else {
            self.list_documents(&collection_name, DEFAULT_LIST_PAGE_SIZE)
                .await
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
        self.list_collection_ids().await?;
        Ok(true)
    }

    async fn get_tables(&self) -> Result<Vec<TableInfo>, DriverError> {
        let ids = self.list_collection_ids().await?;
        Ok(ids
            .into_iter()
            .map(|name| TableInfo {
                name,
                schema: None,
                table_type: "COLLECTION".to_string(),
            })
            .collect())
    }

    async fn get_table_columns(
        &self,
        table: &str,
        _schema: Option<&str>,
    ) -> Result<Vec<ColumnInfo>, DriverError> {
        let docs = self.list_documents(table, 20).await?;
        Ok(infer_columns_from_docs(&docs))
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
        self.list_documents(table, limit).await
    }

    fn engine_name(&self) -> &str {
        "firestore"
    }

    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::Firestore
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reproduces the user's bug: a SQL statement reached the Firestore driver
    /// and was silently treated as a (nonsensical) collection name, returning 0
    /// rows. After the fix, the driver should reject SQL with a clear error.
    #[test]
    fn rejects_sql_select_statement() {
        let result = parse_firestore_query("SELECT * FROM \"Profiles\"");
        assert!(result.is_err(), "SQL should be rejected, got: {:?}", result);
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("does not support SQL"),
            "Error should mention SQL not supported, got: {}",
            err
        );
    }

    #[test]
    fn rejects_other_sql_keywords() {
        for sql in [
            "select * from foo",
            "DELETE FROM x",
            "update y set a=1",
            "WITH cte AS (SELECT 1)",
        ] {
            assert!(
                parse_firestore_query(sql).is_err(),
                "should reject SQL: {}",
                sql
            );
        }
    }

    #[test]
    fn accepts_plain_collection_name() {
        let (coll, tail) = parse_firestore_query("Profiles").expect("should parse");
        assert_eq!(coll, "Profiles");
        assert!(tail.is_none());
    }

    #[test]
    fn accepts_quoted_collection_name() {
        let (coll, _) = parse_firestore_query("\"Profiles\"").expect("should parse");
        assert_eq!(coll, "Profiles");
    }

    #[test]
    fn accepts_collection_with_structured_query() {
        let (coll, tail) = parse_firestore_query("Profiles.{\"limit\":10}").expect("should parse");
        assert_eq!(coll, "Profiles");
        assert_eq!(tail.as_deref(), Some("{\"limit\":10}"));
    }

    #[test]
    fn rejects_empty_query() {
        assert!(parse_firestore_query("   ").is_err());
    }

    #[test]
    fn rejects_collection_with_spaces() {
        // The buggy split-on-`.` approach would have happily passed this through;
        // now we should reject it.
        let result = parse_firestore_query("Some Collection Name");
        assert!(result.is_err(), "spaces in collection name should be rejected");
    }
}
