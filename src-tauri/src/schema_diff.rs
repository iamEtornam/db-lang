use crate::drivers::{DatabaseDriver, ColumnInfo, TableInfo};
use serde::Serialize;

#[derive(Serialize)]
pub struct SnapshotTable { pub table: TableInfo, pub columns: Vec<ColumnInfo>, pub special_columns: Vec<String> }
#[derive(Serialize)]
pub struct SchemaComparisonInput { pub engine: String, pub tables: Vec<SnapshotTable> }

#[tauri::command]
pub async fn get_comparison_schema(connection_id: &str) -> Result<SchemaComparisonInput, String> {
    let (engine, uri, read_only, lease) = crate::resolve_connection(connection_id).await?;
    if !["postgres", "mysql", "mariadb", "sqlite"].contains(&engine.as_str()) {
        return Err("Schema comparison supports SQL connections only".into());
    }
    let driver = crate::drivers::create_driver_with_policy(&engine, &uri, read_only, lease.is_some()).await.map_err(|e| e.to_string())?;
    let tables = snapshot(driver.as_ref(), &engine).await?;
    Ok(SchemaComparisonInput { engine, tables })
}

fn unique_columns(columns: Vec<ColumnInfo>) -> Result<Vec<ColumnInfo>, String> {
    let mut unique = std::collections::BTreeMap::<String, ColumnInfo>::new();
    for column in columns {
        if let Some(existing) = unique.get_mut(&column.name) {
            if existing.data_type != column.data_type || existing.is_nullable != column.is_nullable || existing.column_default != column.column_default { return Err("Inconsistent column metadata; refresh comparison".into()); }
            existing.is_primary_key |= column.is_primary_key;
            existing.is_foreign_key |= column.is_foreign_key;
        } else { unique.insert(column.name.clone(), column); }
    }
    Ok(unique.into_values().collect())
}

async fn snapshot(driver: &dyn DatabaseDriver, engine: &str) -> Result<Vec<SnapshotTable>, String> {
    let tables: Vec<_> = driver.get_tables().await.map_err(|e| e.to_string())?.into_iter()
        .filter(|table| table.table_type.eq_ignore_ascii_case("table") || table.table_type.eq_ignore_ascii_case("base table")).collect();
    if tables.len() > 256 { return Err("Comparison supports at most 256 tables per connection".into()); }
    // Preserve native length/precision information absent from the general column metadata.
    let native_types = match engine {
        "postgres" => driver.execute_query("SELECT n.nspname AS schema, c.relname AS table_name, a.attname AS name, pg_catalog.format_type(a.atttypid, a.atttypmod) AS data_type, (a.attgenerated <> '' OR a.attidentity <> '') AS special FROM pg_catalog.pg_attribute a JOIN pg_catalog.pg_class c ON c.oid=a.attrelid JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE a.attnum>0 AND NOT a.attisdropped AND c.relkind IN ('r','p') AND n.nspname NOT IN ('pg_catalog','information_schema') LIMIT 32769").await,
        "mysql" | "mariadb" => driver.execute_query("SELECT TABLE_SCHEMA AS `schema`, TABLE_NAME AS table_name, COLUMN_NAME AS name, COLUMN_TYPE AS data_type, (EXTRA <> '') AS special FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() LIMIT 32769").await,
        _ => Ok(Vec::new()),
    }.map_err(|e| e.to_string())?;
    if native_types.len() > 32768 { return Err("Comparison supports at most 32,768 columns".into()); }
    let native_types: std::collections::HashMap<_, _> = native_types.into_iter().map(|row| {
        let key = (row["schema"].as_str().map(str::to_string), row["table_name"].as_str().unwrap_or("").to_string(), row["name"].as_str().unwrap_or("").to_string());
        (key, row)
    }).collect();
    let mut output = Vec::new();
    // ponytail: sequential metadata calls reuse driver APIs; batch snapshots if 256-table latency matters.
    for table in tables {
        let mut columns = unique_columns(driver.get_table_columns(&table.name, table.schema.as_deref()).await.map_err(|e| e.to_string())?)?;
        if columns.len() > 128 { return Err(format!("Table {} exceeds 128 columns", table.name)); }
        let mut special_columns = Vec::new();
        if engine != "sqlite" {
            for column in &mut columns {
                let row = native_types.get(&(table.schema.clone(), table.name.clone(), column.name.clone())).ok_or("Schema changed or native type metadata is unavailable; refresh comparison")?;
                let native = row["data_type"].as_str().ok_or("Native type metadata is unavailable")?;
                if row["special"].as_bool() == Some(true) || row["special"].as_i64().is_some_and(|v| v != 0) { special_columns.push(column.name.clone()); }
                column.data_type = native.to_string();
            }
        }
        else {
            let rows = driver.execute_query(&format!("SELECT name, hidden, pk FROM pragma_table_xinfo('{}')", table.name.replace('\'', "''"))).await.map_err(|e| e.to_string())?;
            for row in rows {
                let name = row["name"].as_str().ok_or("Missing SQLite column name")?;
                if row["hidden"].as_i64().is_some_and(|v| v != 0) { special_columns.push(name.to_string()); }
                if let Some(column) = columns.iter_mut().find(|column| column.name == name) { column.is_primary_key = row["pk"].as_i64().is_some_and(|v| v > 0); }
            }
            let foreign_keys = driver.execute_query(&format!("SELECT * FROM pragma_foreign_key_list('{}')", table.name.replace('\'', "''"))).await.map_err(|e| e.to_string())?;
            for row in foreign_keys {
                if let Some(column) = columns.iter_mut().find(|column| Some(column.name.as_str()) == row["from"].as_str()) {
                    column.is_foreign_key = true;
                    column.referenced_table = row["table"].as_str().map(str::to_string);
                    column.referenced_column = row["to"].as_str().map(str::to_string);
                }
            }
        }
        output.push(SnapshotTable { table, columns, special_columns });
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_key_metadata_collapses_to_one_conservative_column() {
        let column = ColumnInfo { name: "id".into(), data_type: "integer".into(), is_nullable: true, column_default: None, is_primary_key: false, is_foreign_key: false, referenced_table: None, referenced_column: None };
        let mut foreign = column.clone(); foreign.is_foreign_key = true;
        let result = unique_columns(vec![column.clone(), foreign]).unwrap();
        assert_eq!(result.len(), 1); assert!(result[0].is_foreign_key);
        let mut inconsistent = column.clone(); inconsistent.data_type = "text".into();
        assert!(unique_columns(vec![column, inconsistent]).is_err());
    }
    #[tokio::test]
    async fn sqlite_snapshot_preserves_declared_types_and_defaults_without_writes() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("target/schema-diff-{}.sqlite", uuid::Uuid::new_v4()));
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE people(id INTEGER, name VARCHAR(30) DEFAULT 'Ada', derived TEXT GENERATED ALWAYS AS (name) VIRTUAL, manager INTEGER REFERENCES people(id), PRIMARY KEY(id, name));").unwrap();
        drop(conn);
        let driver = crate::drivers::create_driver_with_policy("sqlite", path.to_str().unwrap(), true, false).await.unwrap();
        let result = snapshot(driver.as_ref(), "sqlite").await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].special_columns, ["derived"]);
        assert!(result[0].columns.iter().find(|column| column.name == "name").unwrap().is_primary_key);
        assert!(result[0].columns.iter().find(|column| column.name == "manager").unwrap().is_foreign_key);
        let name = result[0].columns.iter().find(|column| column.name == "name").unwrap();
        assert_eq!(name.data_type, "VARCHAR(30)");
        assert_eq!(name.column_default.as_deref(), Some("'Ada'"));
    }
}
