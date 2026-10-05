use serde::Serialize;
use serde_json::Value;
use sqlparser::{
    ast::Statement,
    dialect::{Dialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect},
    parser::Parser,
    tokenizer::{Token, Tokenizer, Whitespace},
};

#[derive(Serialize)]
pub struct QueryPlan {
    pub engine: String,
    pub format: &'static str,
    pub rows: Vec<Value>,
}

/// Plan exactly one statement. Never accept caller-provided EXPLAIN options.
/// SQL parsing distinguishes statement separators from quoted semicolons.
pub fn explain_statement(engine: &str, query: &str) -> Result<(String, &'static str), String> {
    if query.len() > 1_048_576 {
        return Err("The query is too large to inspect (maximum 1 MiB)".into());
    }
    let (dialect, prefix, format): (Box<dyn Dialect>, &str, &str) = match engine {
        "postgres" => (Box::new(PostgreSqlDialect {}), "EXPLAIN (ANALYZE FALSE, FORMAT JSON)", "postgres-json"),
        "mysql" | "mariadb" => (Box::new(MySqlDialect {}), "EXPLAIN FORMAT=JSON", "mysql-json"),
        "sqlite" => (Box::new(SQLiteDialect {}), "EXPLAIN QUERY PLAN", "sqlite-tree"),
        _ => return Err("Query plans require PostgreSQL, MySQL, MariaDB, or SQLite".into()),
    };
    // MySQL/MariaDB execute versioned comments, whereas the parser treats them
    // as comments. Reject these tokens without rejecting string literals.
    let tokens = Tokenizer::new(dialect.as_ref(), query).tokenize().map_err(|e| format!("Could not parse query: {e}"))?;
    if ["mysql", "mariadb"].contains(&engine) && tokens.iter().any(|token| matches!(token,
        Token::Whitespace(Whitespace::MultiLineComment(comment)) if comment.starts_with('!') || comment.starts_with("M!")
    )) {
        return Err("Remove executable versioned comments before inspecting this query".into());
    }
    let statements = Parser::parse_sql(dialect.as_ref(), query).map_err(|e| format!("Could not parse query: {e}"))?;
    if statements.len() != 1 {
        return Err("Inspect one SQL statement at a time".into());
    }
    if !matches!(&statements[0], Statement::Query(_) | Statement::Insert(_) | Statement::Update(_) | Statement::Delete(_)) {
        return Err("Inspect a SELECT, WITH, INSERT, UPDATE, or DELETE statement. Do not include EXPLAIN or ANALYZE".into());
    }
    Ok((format!("{prefix} {}", query.trim()), format))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drivers::{sqlite::SqliteDriver, DatabaseDriver};

    #[test]
    fn separators_are_parsed_and_options_cannot_enable_execution() {
        for engine in ["postgres", "mysql", "mariadb", "sqlite"] {
            assert!(explain_statement(engine, "SELECT ';' AS separator;").is_ok());
            assert!(explain_statement(engine, "SELECT 1; DELETE FROM users").is_err());
            assert!(explain_statement(engine, "EXPLAIN ANALYZE SELECT 1").is_err());
            assert!(explain_statement(engine, "").is_err());
        }
        assert!(explain_statement("postgres", "SELECT $$a;b$$").is_ok());
        assert!(explain_statement("postgres", "/* comment */ WITH rows AS (SELECT 1) SELECT * FROM rows").is_ok());
        assert!(explain_statement("mysql", "SELECT 1 /*!; DELETE FROM users */").is_err());
        assert!(explain_statement("mariadb", "SELECT 1 /*M!; DELETE FROM users */").is_err());
        assert!(explain_statement("mysql", "SELECT '/*! just text */'").is_ok());
        assert!(explain_statement("redis", "GET key").is_err());
    }

    #[tokio::test]
    async fn sqlite_plans_scans_and_indexes_without_running_writes() {
        let driver = SqliteDriver::new(":memory:").unwrap();
        driver.execute_query("CREATE TABLE items(id INTEGER PRIMARY KEY, label TEXT)").await.unwrap();
        driver.execute_query("INSERT INTO items VALUES (1, 'original')").await.unwrap();
        let (scan, _) = explain_statement("sqlite", "SELECT * FROM items WHERE label = 'original'").unwrap();
        let scan = driver.inspect_plan(&scan).await.unwrap();
        assert!(scan.iter().any(|row| row["detail"].as_str().unwrap_or("").contains("SCAN items")));
        let (indexed, _) = explain_statement("sqlite", "SELECT * FROM items WHERE id = 1").unwrap();
        let indexed = driver.inspect_plan(&indexed).await.unwrap();
        assert!(indexed.iter().any(|row| row["detail"].as_str().unwrap_or("").contains("SEARCH items")));
        let (update, _) = explain_statement("sqlite", "UPDATE items SET label = 'changed'").unwrap();
        driver.inspect_plan(&update).await.unwrap();
        let rows = driver.execute_query("SELECT label FROM items").await.unwrap();
        assert_eq!(rows[0]["label"], "original");
    }
}
