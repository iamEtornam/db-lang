use serde::Deserialize;
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GeminiError {
    #[error("Failed to translate to SQL: {0}")]
    TranslationFailed(String),
    #[error("Destructive query detected: {0}")]
    DestructiveQuery(String),
    #[error("API request failed: {0}")]
    ApiRequestFailed(String),
    #[error("API key not found. Please set the GEMINI_API_KEY environment variable.")]
    ApiKeyNotFound,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: Content,
}

#[derive(Deserialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Deserialize)]
struct Part {
    text: String,
}

fn contains_destructive_keywords(query: &str) -> bool {
    let destructive_keywords = ["DROP", "DELETE", "UPDATE", "TRUNCATE", "ALTER"];
    let upper_query = query.to_uppercase();
    destructive_keywords
        .iter()
        .any(|&keyword| upper_query.contains(keyword))
}

pub async fn translate_to_sql(query: &str) -> Result<String, GeminiError> {
    let api_key = std::env::var("GEMINI_API_KEY").map_err(|_| GeminiError::ApiKeyNotFound)?;

    let client = reqwest::Client::new();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent?key={}",
        api_key
    );

    let prompt = format!(
        "Translate the following natural language query into a SQL query: \"{}\". Only return the SQL query.",
        query
    );

    let response = client
        .post(&url)
        .json(&json!({
            "contents": [{
                "parts": [{
                    "text": prompt
                }]
            }]
        }))
        .send()
        .await
        .map_err(|e| GeminiError::ApiRequestFailed(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(GeminiError::ApiRequestFailed(format!(
            "API request failed with status code {}: {}",
            status,
            error_text
        )));
    }

    let gemini_response = response
        .json::<GeminiResponse>()
        .await
        .map_err(|e| GeminiError::TranslationFailed(e.to_string()))?;

    let sql_query = gemini_response
        .candidates
        .get(0)
        .and_then(|c| c.content.parts.get(0))
        .map(|p| p.text.clone())
        .ok_or_else(|| GeminiError::TranslationFailed("No SQL query found in response".to_string()))?;

    if contains_destructive_keywords(&sql_query) {
        return Err(GeminiError::DestructiveQuery(sql_query));
    }

    Ok(sql_query)
}