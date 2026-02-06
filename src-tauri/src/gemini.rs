use crate::app_db::{get_app_database, LlmConfig};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::command;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LlmError {
    #[error("Failed to translate to SQL: {0}")]
    TranslationFailed(String),
    #[error("Destructive query detected: {0}")]
    DestructiveQuery(String),
    #[error("API request failed: {0}")]
    ApiRequestFailed(String),
    #[error("API key not configured. Please set your API key in Settings > AI Model.")]
    ApiKeyNotFound,
    #[error("LLM provider not configured. Please configure your AI model in Settings.")]
    NotConfigured,
}

/// Query explanation response
#[derive(Serialize, Deserialize)]
pub struct QueryExplanation {
    pub summary: String,
    #[serde(default)]
    pub clauses: Vec<ClauseExplanation>,
    #[serde(default)]
    pub tables_involved: Vec<String>,
    #[serde(default)]
    pub potential_issues: Vec<String>,
    #[serde(default)]
    pub optimization_tips: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ClauseExplanation {
    pub clause_type: String,
    pub content: String,
    pub explanation: String,
}

// ============ LLM Provider Abstraction ============

/// Get the current LLM config from database, falling back to env var for API key
fn get_llm_settings() -> Result<LlmConfig, LlmError> {
    let db = get_app_database().map_err(|e| LlmError::ApiRequestFailed(e.to_string()))?;
    
    let config = db.get_llm_config()
        .map_err(|e| LlmError::ApiRequestFailed(e.to_string()))?;
    
    match config {
        Some(c) if !c.api_key.is_empty() => Ok(c),
        Some(mut c) => {
            // Fall back to env var if api_key is empty
            if let Ok(env_key) = std::env::var("GEMINI_API_KEY") {
                c.api_key = env_key;
                Ok(c)
            } else if let Ok(env_key) = std::env::var("OPENAI_API_KEY") {
                c.api_key = env_key;
                Ok(c)
            } else {
                Err(LlmError::ApiKeyNotFound)
            }
        }
        None => {
            // No config at all -- try env vars
            if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
                Ok(LlmConfig {
                    provider: "gemini".to_string(),
                    model: "gemini-pro".to_string(),
                    api_key,
                    api_url: None,
                    created_at: String::new(),
                    updated_at: String::new(),
                })
            } else if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
                Ok(LlmConfig {
                    provider: "openai".to_string(),
                    model: "gpt-4".to_string(),
                    api_key,
                    api_url: None,
                    created_at: String::new(),
                    updated_at: String::new(),
                })
            } else {
                Err(LlmError::ApiKeyNotFound)
            }
        }
    }
}

/// Build the API URL for a given provider and model
fn build_api_url(config: &LlmConfig) -> String {
    // If user provides a custom URL, use that directly
    if let Some(ref url) = config.api_url {
        if !url.is_empty() {
            return url.clone();
        }
    }

    match config.provider.as_str() {
        "gemini" => format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            config.model, config.api_key
        ),
        "openai" => "https://api.openai.com/v1/chat/completions".to_string(),
        "anthropic" => "https://api.anthropic.com/v1/messages".to_string(),
        "ollama" => {
            let base = config.api_url.as_deref().unwrap_or("http://localhost:11434");
            format!("{}/api/generate", base)
        }
        "deepseek" => "https://api.deepseek.com/v1/chat/completions".to_string(),
        "groq" => "https://api.groq.com/openai/v1/chat/completions".to_string(),
        // Custom provider -- user must provide api_url
        _ => config.api_url.clone().unwrap_or_default(),
    }
}

/// Build the request body for a given provider
fn build_request_body(config: &LlmConfig, prompt: &str) -> serde_json::Value {
    match config.provider.as_str() {
        "gemini" => json!({
            "contents": [{
                "parts": [{ "text": prompt }]
            }]
        }),
        "openai" | "deepseek" | "groq" => json!({
            "model": config.model,
            "messages": [
                { "role": "system", "content": "You are a helpful SQL assistant." },
                { "role": "user", "content": prompt }
            ],
            "temperature": 0.2
        }),
        "anthropic" => json!({
            "model": config.model,
            "max_tokens": 4096,
            "messages": [
                { "role": "user", "content": prompt }
            ]
        }),
        "ollama" => json!({
            "model": config.model,
            "prompt": prompt,
            "stream": false
        }),
        // For custom/unknown providers, try OpenAI-compatible format
        _ => json!({
            "model": config.model,
            "messages": [
                { "role": "system", "content": "You are a helpful SQL assistant." },
                { "role": "user", "content": prompt }
            ],
            "temperature": 0.2
        }),
    }
}

/// Parse the response text from different provider formats
fn parse_response_text(config: &LlmConfig, response_json: &serde_json::Value) -> Result<String, LlmError> {
    match config.provider.as_str() {
        "gemini" => {
            response_json
                .get("candidates")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("content"))
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.get(0))
                .and_then(|p| p.get("text"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| LlmError::TranslationFailed("No response from Gemini".to_string()))
        }
        "openai" | "deepseek" | "groq" => {
            response_json
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| LlmError::TranslationFailed("No response from OpenAI-compatible API".to_string()))
        }
        "anthropic" => {
            response_json
                .get("content")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("text"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| LlmError::TranslationFailed("No response from Anthropic".to_string()))
        }
        "ollama" => {
            response_json
                .get("response")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| LlmError::TranslationFailed("No response from Ollama".to_string()))
        }
        // Try OpenAI format, then Gemini format, then raw
        _ => {
            // Try OpenAI format
            if let Some(text) = response_json.get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|t| t.as_str()) {
                return Ok(text.to_string());
            }
            // Try Gemini format
            if let Some(text) = response_json.get("candidates")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("content"))
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.get(0))
                .and_then(|p| p.get("text"))
                .and_then(|t| t.as_str()) {
                return Ok(text.to_string());
            }
            Err(LlmError::TranslationFailed("Could not parse response from custom provider".to_string()))
        }
    }
}

/// Main function to call any configured LLM provider
async fn call_llm_api(prompt: &str) -> Result<String, LlmError> {
    let config = get_llm_settings()?;
    let url = build_api_url(&config);
    let body = build_request_body(&config, prompt);

    let client = reqwest::Client::new();
    let mut request = client.post(&url).json(&body);

    // Add auth headers based on provider
    match config.provider.as_str() {
        "gemini" => {
            // Gemini uses API key in URL, no extra header needed
        }
        "openai" | "deepseek" | "groq" => {
            request = request.header("Authorization", format!("Bearer {}", config.api_key));
        }
        "anthropic" => {
            request = request
                .header("x-api-key", &config.api_key)
                .header("anthropic-version", "2023-06-01");
        }
        "ollama" => {
            // Ollama typically doesn't need auth
        }
        _ => {
            // For custom providers, add Bearer token by default
            if !config.api_key.is_empty() {
                request = request.header("Authorization", format!("Bearer {}", config.api_key));
            }
        }
    }

    let response = request
        .send()
        .await
        .map_err(|e| LlmError::ApiRequestFailed(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(LlmError::ApiRequestFailed(format!(
            "API returned status {}: {}",
            status, error_text
        )));
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| LlmError::TranslationFailed(e.to_string()))?;

    parse_response_text(&config, &response_json)
}

// ============ Utility ============

fn contains_destructive_keywords(query: &str) -> bool {
    let destructive_keywords = ["DROP", "DELETE", "UPDATE", "TRUNCATE", "ALTER"];
    let upper_query = query.to_uppercase();
    destructive_keywords
        .iter()
        .any(|&keyword| upper_query.contains(keyword))
}

// ============ Public API (Tauri Commands) ============

pub async fn translate_to_sql(query: &str) -> Result<String, LlmError> {
    let prompt = format!(
        r#"Translate the following natural language query into a PostgreSQL-compatible SQL query: "{}"

Rules:
- Always double-quote table and column names to handle reserved words and case sensitivity (e.g. "User", "Order", "public"."User")
- Only return the SQL query, nothing else
- Do not include markdown formatting"#,
        query
    );

    let sql_query = call_llm_api(&prompt).await?;

    let sql_query = sql_query
        .trim()
        .trim_start_matches("```sql")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    if contains_destructive_keywords(&sql_query) {
        return Err(LlmError::DestructiveQuery(sql_query));
    }

    Ok(sql_query)
}

#[command]
pub async fn explain_query(sql_query: &str) -> Result<QueryExplanation, String> {
    let prompt = format!(
        r#"Analyze this SQL query and provide a detailed explanation in JSON format:

SQL Query: {}

Return a JSON object with this exact structure (no markdown, just pure JSON):
{{
    "summary": "A brief one-sentence description of what this query does",
    "clauses": [
        {{
            "clause_type": "SELECT|FROM|WHERE|JOIN|GROUP BY|ORDER BY|etc",
            "content": "The actual SQL content of this clause",
            "explanation": "What this clause does in plain English"
        }}
    ],
    "tables_involved": ["list", "of", "table", "names"],
    "potential_issues": ["Any potential performance or logic issues"],
    "optimization_tips": ["Suggestions to improve the query"]
}}

Only return valid JSON, no additional text or markdown."#,
        sql_query
    );

    let response = call_llm_api(&prompt).await.map_err(|e| e.to_string())?;

    let cleaned_response = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    match serde_json::from_str::<QueryExplanation>(cleaned_response) {
        Ok(explanation) => Ok(explanation),
        Err(_) => {
            Ok(QueryExplanation {
                summary: response.lines().next().unwrap_or("Query analysis").to_string(),
                clauses: vec![],
                tables_involved: vec![],
                potential_issues: vec![],
                optimization_tips: vec![response],
            })
        }
    }
}

#[command]
pub async fn suggest_query_improvements(sql_query: &str, db_type: &str) -> Result<String, String> {
    let prompt = format!(
        r#"You are a {} database expert. Analyze this SQL query and suggest improvements:

SQL Query: {}

Provide:
1. Performance optimizations (indexing suggestions, query restructuring)
2. Best practices violations
3. Rewritten query if applicable
4. Explanation of each suggestion

Keep the response concise but informative."#,
        db_type, sql_query
    );

    call_llm_api(&prompt).await.map_err(|e| e.to_string())
}

#[command]
pub async fn generate_sample_queries(table_name: &str, columns: Vec<String>, db_type: &str) -> Result<Vec<String>, String> {
    let columns_str = columns.join(", ");
    let prompt = format!(
        r#"Generate 5 useful sample SQL queries for a {} database table named "{}" with columns: {}

Return only the SQL queries, one per line, no explanations. Include:
1. A basic SELECT query
2. A filtered query with WHERE
3. An aggregation query with GROUP BY
4. A sorted query with ORDER BY
5. A query with LIMIT/pagination"#,
        db_type, table_name, columns_str
    );

    let response = call_llm_api(&prompt).await.map_err(|e| e.to_string())?;

    let queries: Vec<String> = response
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter(|line| line.trim().to_uppercase().starts_with("SELECT") ||
                       line.trim().starts_with("1.") ||
                       line.trim().starts_with("2.") ||
                       line.trim().starts_with("3.") ||
                       line.trim().starts_with("4.") ||
                       line.trim().starts_with("5."))
        .map(|line| {
            line.trim()
                .trim_start_matches(|c: char| c.is_numeric() || c == '.' || c == ' ')
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect();

    Ok(queries)
}
