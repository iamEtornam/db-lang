//! Script library helpers: param substitution, statement splitting, and
//! seeding the bundled built-in library from the app's resource dir.

use crate::app_db::Script;
use serde::Deserialize;
use std::collections::HashMap;

/// One bundled script as authored in `resources/scripts/<engine>.json`.
/// The `engine`/`id`/timestamps are assigned at seed time, not in the file.
#[derive(Debug, Deserialize)]
struct BuiltinScript {
    name: String,
    #[serde(default)]
    description: Option<String>,
    query_language: String,
    body: String,
    #[serde(default)]
    tags: String,
    #[serde(default)]
    params: serde_json::Value,
}

const BUILTIN_ENGINES: [&str; 7] = [
    "postgres",
    "mysql",
    "sqlite",
    "mongodb",
    "redis",
    "firestore",
    "firebase_rtdb",
];

/// Substitute `{{param}}` tokens in `body` with the supplied values.
/// Unknown tokens are left untouched (the run will surface any resulting
/// query error). Whitespace inside the braces is tolerated: `{{ name }}`.
pub fn substitute_params(body: &str, params: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(body.len());
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(end) = body[i + 2..].find("}}") {
                let raw = &body[i + 2..i + 2 + end];
                let key = raw.trim();
                if let Some(val) = params.get(key) {
                    out.push_str(val);
                    i = i + 2 + end + 2;
                    continue;
                }
                // Unknown placeholder: keep it verbatim.
                out.push_str(&body[i..i + 2 + end + 2]);
                i = i + 2 + end + 2;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Split a script body into individual statements for the given query
/// language. SQL splits on `;`, Redis on newlines; everything else (MQL,
/// Firestore, RTDB) is treated as a single statement so embedded JSON is
/// never broken apart.
pub fn split_statements(body: &str, query_language: &str) -> Vec<String> {
    match query_language.to_lowercase().as_str() {
        "sql" => body
            .split(';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        "redis" => body
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => {
            let trimmed = body.trim();
            if trimmed.is_empty() {
                vec![]
            } else {
                vec![trimmed.to_string()]
            }
        }
    }
}

/// Load every bundled `<engine>.json` from `resource_dir` and reseed the
/// built-in rows. Missing files are skipped (a build may not ship them all).
/// Returns the list of parsed scripts so callers can also use it in tests.
pub fn load_builtin_scripts(resource_dir: &std::path::Path) -> Vec<Script> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut scripts = Vec::new();
    for engine in BUILTIN_ENGINES {
        let path = resource_dir.join("scripts").join(format!("{}.json", engine));
        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue, // ponytail: tolerate a missing engine file
        };
        let parsed: Vec<BuiltinScript> = match serde_json::from_str(&contents) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Skipping builtin scripts for {}: {}", engine, e);
                continue;
            }
        };
        for (idx, b) in parsed.into_iter().enumerate() {
            let params_json = if b.params.is_null() {
                "[]".to_string()
            } else {
                b.params.to_string()
            };
            scripts.push(Script {
                // Deterministic id so reseeding is idempotent.
                id: format!("builtin:{}:{}", engine, idx),
                name: b.name,
                description: b.description,
                engine: engine.to_string(),
                query_language: b.query_language,
                body: b.body,
                params_json,
                tags: b.tags,
                is_builtin: true,
                created_at: now.clone(),
                updated_at: now.clone(),
            });
        }
    }
    scripts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_known_and_keeps_unknown() {
        let mut params = HashMap::new();
        params.insert("table".to_string(), "users".to_string());
        params.insert("limit".to_string(), "10".to_string());
        let body = "SELECT * FROM {{ table }} LIMIT {{limit}} -- {{missing}}";
        let out = substitute_params(body, &params);
        assert_eq!(out, "SELECT * FROM users LIMIT 10 -- {{missing}}");
    }

    #[test]
    fn splits_sql_on_semicolons() {
        let stmts = split_statements("TYPE k;\n TTL k ;", "sql");
        assert_eq!(stmts, vec!["TYPE k", "TTL k"]);
    }

    #[test]
    fn redis_splits_on_lines_mql_stays_whole() {
        assert_eq!(split_statements("TYPE k\nTTL k", "redis"), vec!["TYPE k", "TTL k"]);
        let mql = "coll.[{ \"$count\": \"n\" }]";
        assert_eq!(split_statements(mql, "mql"), vec![mql.to_string()]);
    }
}
