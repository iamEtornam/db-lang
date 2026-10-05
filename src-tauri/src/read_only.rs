use std::ops::ControlFlow;
use sqlparser::{ast::{Query, SetExpr, Statement, Visit, Visitor}, dialect::{Dialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect}, parser::Parser, tokenizer::{Token, Tokenizer}};

pub const POLICY_ERROR: &str = "This connection is read-only. Disable read-only in its settings to permit writes.";

pub fn check_query(engine: &str, query: &str) -> Result<(), String> {
    if query.len() > 1024 * 1024 { return Err("Query exceeds the 1 MiB limit".into()); }
    match engine {
        "postgres" | "mysql" | "mariadb" | "sqlite" => {
            let dialect: Box<dyn Dialect> = match engine { "postgres" => Box::new(PostgreSqlDialect {}), "sqlite" => Box::new(SQLiteDialect {}), _ => Box::new(MySqlDialect {}) };
            let tokens = Tokenizer::new(dialect.as_ref(), query).tokenize().map_err(|e| e.to_string())?;
            if tokens.iter().any(|t| matches!(t, Token::Whitespace(sqlparser::tokenizer::Whitespace::MultiLineComment(comment)) if comment.starts_with('!') || comment.starts_with("M!"))) { return Err(POLICY_ERROR.into()); }
            let statements = Parser::parse_sql(dialect.as_ref(), query).map_err(|e| format!("Read-only query validation: {e}"))?;
            if statements.len() != 1 { return Err(POLICY_ERROR.into()); }
            if let ControlFlow::Break(()) = statements.visit(&mut ReadOnlyVisitor) { return Err(POLICY_ERROR.into()); }
        }
        "redis" => {
            for line in query.lines().filter(|line| !line.trim().is_empty()) {
                let parts: Vec<_> = line.split_whitespace().collect();
                let command = parts[0].to_ascii_uppercase();
                if !["GET", "KEYS", "HGETALL", "LRANGE", "SMEMBERS", "TTL", "TYPE", "DBSIZE", "INFO"].contains(&command.as_str()) { return Err(POLICY_ERROR.into()); }
                if ["GET", "HGETALL", "LRANGE", "SMEMBERS", "TTL", "TYPE"].contains(&command.as_str()) && parts.len() < 2 { return Err("Redis command requires a key".into()); }
            }
        }
        "mongodb" => {
            if let Some((_, pipeline)) = query.trim().split_once('.') {
                let value: serde_json::Value = serde_json::from_str(pipeline).map_err(|e| e.to_string())?;
                if contains_write_stage(&value) { return Err(POLICY_ERROR.into()); }
            }
        }
        "firestore" | "firebase_rtdb" => {} // Their query APIs perform only reads; write commands are guarded separately.
        _ => return Err("Read-only policy is unavailable for this engine".into()),
    }
    Ok(())
}
fn contains_write_stage(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => map.iter().any(|(key, value)| key == "$out" || key == "$merge" || contains_write_stage(value)),
        serde_json::Value::Array(values) => values.iter().any(contains_write_stage),
        _ => false,
    }
}
struct ReadOnlyVisitor;
impl Visitor for ReadOnlyVisitor {
    type Break = ();
    fn pre_visit_statement(&mut self, statement: &Statement) -> ControlFlow<()> {
        if matches!(statement, Statement::Query(_)) { ControlFlow::Continue(()) } else { ControlFlow::Break(()) }
    }
    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<()> {
        if !query.locks.is_empty() || !safe_body(&query.body, 0) { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
    }
}
fn safe_body(body: &SetExpr, depth: usize) -> bool {
    if depth > 80 { return false; }
    match body {
        SetExpr::Select(select) => select.into.is_none(),
        SetExpr::Query(query) => safe_body(&query.body, depth + 1),
        SetExpr::SetOperation { left, right, .. } => safe_body(left, depth + 1) && safe_body(right, depth + 1),
        SetExpr::Values(_) | SetExpr::Table(_) => true,
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_and_write_bypasses() {
        for engine in ["postgres", "sqlite", "mysql", "mariadb"] {
            assert!(check_query(engine, "SELECT 'delete; update' AS literal").is_ok());
            for sql in ["DELETE FROM x", "SELECT 1; DELETE FROM x", "SELECT 1 INTO y", "WITH x AS (DELETE FROM y RETURNING *) SELECT * FROM x", "SELECT * FROM x FOR UPDATE", "/*!50000 DELETE FROM x */ SELECT 1"] { assert!(check_query(engine, sql).is_err(), "{engine}: {sql}"); }
        }
        assert!(check_query("redis", "GET k\nTTL k").is_ok());
        assert!(check_query("redis", "GET").is_err());
        assert!(check_query("redis", "GET k\nSET k v").is_err());
        assert!(check_query("mongodb", "items.[{\"$match\":{\"status\":\"$out\"}}]").is_ok());
        assert!(check_query("mongodb", "items.[{\"$out\":\"other\"}]").is_err());
    }
}
