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
    let mut rest = body;
    // `find` only returns offsets on char boundaries, so every slice below is
    // UTF-8 safe — non-ASCII bodies pass through unchanged.
    while let Some(start) = rest.find("{{") {
        match rest[start + 2..].find("}}") {
            Some(end) => {
                out.push_str(&rest[..start]);
                let key = rest[start + 2..start + 2 + end].trim();
                match params.get(key) {
                    Some(val) => out.push_str(val),
                    // Unknown placeholder: keep it verbatim.
                    None => out.push_str(&rest[start..start + 2 + end + 2]),
                }
                rest = &rest[start + 2 + end + 2..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

/// Split a script body into individual statements for the given query
/// language. SQL splits on `;`, Redis on newlines; everything else (MQL,
/// Firestore, RTDB) is treated as a single statement so embedded JSON is
/// never broken apart.
pub fn split_statements(body: &str, query_language: &str) -> Vec<String> {
    match query_language.to_lowercase().as_str() {
        // ponytail: state machine handles ';' inside single/double quotes and
        // `--` line comments. Does NOT handle Postgres $$ dollar-quoting or
        // /* */ block comments — add those arms if a builtin/user script needs them.
        "sql" => {
            let mut statements = Vec::new();
            let mut current = String::new();
            let mut in_single = false;
            let mut in_double = false;
            let mut in_comment = false;
            let mut chars = body.chars().peekable();
            while let Some(c) = chars.next() {
                if in_comment {
                    current.push(c);
                    if c == '\n' {
                        in_comment = false;
                    }
                } else if in_single {
                    current.push(c);
                    if c == '\'' {
                        // '' is an escaped quote, stay inside the literal.
                        if chars.peek() == Some(&'\'') {
                            current.push(chars.next().unwrap());
                        } else {
                            in_single = false;
                        }
                    }
                } else if in_double {
                    current.push(c);
                    if c == '"' {
                        in_double = false;
                    }
                } else if c == '-' && chars.peek() == Some(&'-') {
                    in_comment = true;
                    current.push(c);
                    current.push(chars.next().unwrap());
                } else if c == '\'' {
                    in_single = true;
                    current.push(c);
                } else if c == '"' {
                    in_double = true;
                    current.push(c);
                } else if c == ';' {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        statements.push(trimmed.to_string());
                    }
                    current.clear();
                } else {
                    current.push(c);
                }
            }
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                statements.push(trimmed.to_string());
            }
            statements
        }
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
    fn split_ignores_semicolons_in_quotes_and_comments() {
        // The `;` inside the string literal and inside the `--` comment must not
        // split; only the two real statement terminators do.
        let body = "INSERT INTO logs VALUES ('a; b'); -- c ; c\nSELECT 1;";
        let stmts = split_statements(body, "sql");
        assert_eq!(
            stmts,
            vec!["INSERT INTO logs VALUES ('a; b')", "-- c ; c\nSELECT 1"]
        );
    }

    #[test]
    fn substitute_is_utf8_safe() {
        let mut params = HashMap::new();
        params.insert("name".to_string(), "Zoë".to_string());
        // Non-ASCII before, inside the value, and after the placeholder.
        let out = substitute_params("café {{ name }} 日本", &params);
        assert_eq!(out, "café Zoë 日本");
    }

    #[test]
    fn redis_splits_on_lines_mql_stays_whole() {
        assert_eq!(split_statements("TYPE k\nTTL k", "redis"), vec!["TYPE k", "TTL k"]);
        let mql = "coll.[{ \"$count\": \"n\" }]";
        assert_eq!(split_statements(mql, "mql"), vec![mql.to_string()]);
    }
}
