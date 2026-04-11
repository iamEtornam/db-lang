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
                    model: "gemini-2.5-flash".to_string(),
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

/// Public wrapper so schema_kb.rs can call the LLM
pub async fn call_llm_api_pub(prompt: &str) -> Result<String, LlmError> {
    call_llm_api(prompt).await
}

/// Check if an LLM provider is configured with an API key
pub fn is_llm_configured() -> bool {
    get_llm_settings().is_ok()
}

// ============ Public API (Tauri Commands) ============

/// Translate with full schema context passed from the frontend
pub async fn translate_with_schema(
    query: &str,
    schema_context: &str,
    table_names: &[String],
    engine: &str,
) -> Result<String, LlmError> {
    let quote = |name: &str| match engine {
        "mysql" | "mariadb" => format!("`{}`", name),
        "mssql" => format!("[{}]", name),
        _ => format!("\"{}\"", name),
    };

    // Build the exact table name list — this is the most important part
    let exact_tables: Vec<String> = table_names
        .iter()
        .map(|t| format!("  - {} (use EXACTLY this spelling and casing)", quote(t)))
        .collect();
    let exact_tables_str = exact_tables.join("\n");

    let prompt = format!(
        r#"You are a {engine} database expert generating SQL queries.

## EXACT TABLE NAMES IN THIS DATABASE (copy character-for-character, including case):
{exact_tables_str}

STOP: Before writing any SQL, look at the table names above. Do not invent table names. Do not pluralize or lowercase them.

## Full Database Schema:
{schema_context}

## Question to answer:
"{query}"

Write a single SQL query that answers the question using ONLY the tables and columns listed above.
- Use the exact table name spelling (e.g. if the table is "User", write "User", NOT "users" or "Users")
- Quote all identifiers
- Return ONLY the raw SQL query with no explanation, no markdown, no code fences"#,
        engine = engine,
        exact_tables_str = exact_tables_str,
        schema_context = schema_context,
        query = query,
    );

    let sql_query = call_llm_api(&prompt).await?;

    let mut sql_query = sql_query
        .trim()
        .trim_start_matches("```sql")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    if contains_destructive_keywords(&sql_query) {
        return Err(LlmError::DestructiveQuery(sql_query));
    }

    // Post-process: fix any wrong-case table name references using known exact names
    sql_query = fix_table_name_casing(&sql_query, table_names, engine);

    Ok(sql_query)
}

/// Correct any table names in the SQL that differ only in case from known table names
fn fix_table_name_casing(sql: &str, table_names: &[String], engine: &str) -> String {
    let mut result = sql.to_string();
    for name in table_names {
        // Match quoted variants that differ in case: "users", "Users", etc.
        let lower = name.to_lowercase();
        let upper = name.to_uppercase();

        let (open, close) = match engine {
            "mysql" | "mariadb" => ("`", "`"),
            "mssql" => ("[", "]"),
            _ => ("\"", "\""),
        };

        // Build regex-like replacement for case-insensitive quoted identifiers
        // Try common wrong-case variants
        let variants = vec![
            format!("{}{}{}", open, lower, close),
            format!("{}{}{}", open, upper, close),
            format!("{}{}{}", open, capitalize(&lower), close),
            format!("{}{}{}", open, name, close), // already correct
        ];

        let correct = format!("{}{}{}", open, name, close);

        for variant in &variants {
            if variant != &correct {
                // Case-insensitive string replace
                let variant_lower = variant.to_lowercase();
                let result_lower = result.to_lowercase();
                if let Some(pos) = result_lower.find(&variant_lower) {
                    result = format!("{}{}{}", &result[..pos], correct, &result[pos + variant.len()..]);
                }
            }
        }
    }
    result
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

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

/// Translation result with rich metadata
#[derive(Serialize, Deserialize)]
pub struct TranslationResult {
    pub query: String,
    pub query_language: String,
    pub tables_used: Vec<String>,
    pub confidence: f32,
    pub explanation: String,
}

/// Chart configuration returned by AI
#[derive(Serialize, Deserialize)]
pub struct ChartConfig {
    pub chart_type: String,
    pub title: String,
    pub x_axis: ChartAxis,
    pub y_axis: ChartAxis,
    pub series: Vec<ChartSeries>,
    pub explanation: String,
}

#[derive(Serialize, Deserialize)]
pub struct ChartAxis {
    pub field: String,
    pub label: String,
}

#[derive(Serialize, Deserialize)]
pub struct ChartSeries {
    pub field: String,
    pub label: String,
    pub color: Option<String>,
}

/// KB-aware natural language to query translation
pub async fn translate_to_query_with_kb(
    natural_language: &str,
    connection_id: &str,
    engine: &str,
) -> Result<TranslationResult, LlmError> {
    use crate::schema_kb;

    // Try to load KB context
    let schema_context = match schema_kb::get_schema_kb(connection_id) {
        Ok(Some(kb)) => schema_kb::build_schema_context(&kb),
        _ => String::new(),
    };

    let query_language = match engine {
        "mongodb" => "MongoDB Aggregation Pipeline (JSON array)",
        "redis" => "Redis commands",
        _ => "SQL",
    };

    let dialect_hint = match engine {
        "postgres" => "PostgreSQL dialect. Double-quote identifiers.",
        "mysql" | "mariadb" => "MySQL dialect. Backtick-quote identifiers.",
        "sqlite" => "SQLite dialect.",
        "mssql" => "T-SQL (SQL Server) dialect.",
        "mongodb" => "MongoDB aggregation pipeline as JSON array.",
        "redis" => "Redis commands, one per line.",
        _ => "Standard SQL.",
    };

    let has_context = !schema_context.is_empty();

    let prompt = if has_context {
        format!(
            r#"You are a database query expert. Convert the natural language question into a {} query.

{}

## User Question:
{}

## Instructions:
- Use {} 
- Only return the query itself, no markdown, no explanation
- Do not include destructive operations (DROP, DELETE, TRUNCATE, ALTER, UPDATE)
- For SQL: use appropriate quoting for identifiers
- For MongoDB: return a valid JSON aggregation pipeline array
- For Redis: return the commands, one per line
- After the query, on a new line starting with "TABLES:", list the table/collection names used separated by commas
- After that, on a new line starting with "EXPLANATION:", write one sentence explaining what the query does
- After that, on a new line starting with "CONFIDENCE:", write a number 0.0-1.0"#,
            query_language,
            schema_context,
            natural_language,
            dialect_hint,
        )
    } else {
        format!(
            r#"Convert this natural language question into a {} query: "{}"

Use {}
Only return the query, no markdown, no explanation."#,
            query_language, natural_language, dialect_hint,
        )
    };

    let response = call_llm_api(&prompt).await?;

    // Parse the response
    let lines: Vec<&str> = response.lines().collect();
    let mut query_lines: Vec<&str> = Vec::new();
    let mut tables_used: Vec<String> = Vec::new();
    let mut explanation = String::new();
    let mut confidence: f32 = 0.5;
    let mut in_query = true;

    for line in &lines {
        if line.starts_with("TABLES:") {
            in_query = false;
            let tables_str = line.trim_start_matches("TABLES:").trim();
            tables_used = tables_str.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
        } else if line.starts_with("EXPLANATION:") {
            in_query = false;
            explanation = line.trim_start_matches("EXPLANATION:").trim().to_string();
        } else if line.starts_with("CONFIDENCE:") {
            in_query = false;
            confidence = line.trim_start_matches("CONFIDENCE:").trim().parse().unwrap_or(0.5);
        } else if in_query {
            query_lines.push(line);
        }
    }

    let query = query_lines.join("\n")
        .trim()
        .trim_start_matches("```sql")
        .trim_start_matches("```javascript")
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    // If no structured parsing worked, use the whole response as the query
    let query = if query.is_empty() {
        response.trim()
            .trim_start_matches("```sql")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string()
    } else {
        query
    };

    if contains_destructive_keywords(&query) {
        return Err(LlmError::DestructiveQuery(query));
    }

    let ql = match engine {
        "mongodb" => "mql",
        "redis" => "redis",
        _ => "sql",
    };

    Ok(TranslationResult {
        query,
        query_language: ql.to_string(),
        tables_used,
        confidence,
        explanation,
    })
}

/// Generate chart configuration from query result data
pub async fn generate_chart_config(
    data: &str,
    columns: &[String],
    query: &str,
    engine: &str,
) -> Result<ChartConfig, LlmError> {
    let prompt = format!(
        r#"Analyze this query result data and return the optimal chart configuration as JSON.

Original question: "{}"
Database: {}
Columns: {}
Sample data (up to 50 rows): {}

Return a JSON object with this exact structure (no markdown):
{{
  "chart_type": "bar|line|area|pie|scatter|table",
  "title": "descriptive chart title",
  "x_axis": {{"field": "column_name", "label": "X axis label"}},
  "y_axis": {{"field": "column_name", "label": "Y axis label"}},
  "series": [{{"field": "column_name", "label": "series label"}}],
  "explanation": "one sentence explaining why this chart type was chosen"
}}

Rules:
- Choose bar chart for comparisons across categories
- Choose line/area for time series or trends  
- Choose pie for proportions/percentages (max 8 slices)
- Choose scatter for correlations between two numeric values
- Default to table if data doesn't suit a chart
- x_axis.field must be a column that exists in the data
- y_axis.field must be a numeric column"#,
        query,
        engine,
        columns.join(", "),
        data,
    );

    let response = call_llm_api(&prompt).await?;

    let cleaned = response.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str::<ChartConfig>(cleaned).map_err(|_| {
        // Fallback: return a table config
        LlmError::TranslationFailed("Could not parse chart config".to_string())
    }).or_else(|_| {
        let first_col = columns.first().cloned().unwrap_or_default();
        let second_col = columns.get(1).cloned().unwrap_or_else(|| first_col.clone());
        Ok(ChartConfig {
            chart_type: "table".to_string(),
            title: "Query Results".to_string(),
            x_axis: ChartAxis { field: first_col.clone(), label: first_col.clone() },
            y_axis: ChartAxis { field: second_col.clone(), label: second_col.clone() },
            series: vec![],
            explanation: "Showing data as table".to_string(),
        })
    })
}

/// Explain data in plain English
pub async fn explain_data(
    data: &str,
    columns: &[String],
    query: &str,
) -> Result<String, LlmError> {
    let prompt = format!(
        r#"You are a data analyst. The user asked: "{}"

Here is the query result:
Columns: {}
Data (up to 100 rows):
{}

Write a concise plain-English explanation of what this data shows. Include:
- The main findings and patterns
- Key numbers or statistics
- Any notable outliers or trends
- What this means in business terms

Keep it to 3-5 sentences. Do not include technical jargon or SQL. Write for a non-technical user."#,
        query,
        columns.join(", "),
        data,
    );

    call_llm_api(&prompt).await
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
