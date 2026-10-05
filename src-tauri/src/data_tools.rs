//! File previews, mapped transactional import, and isolated local DuckDB queries.
use std::{collections::HashSet, io::{Cursor, Read}, path::Path, sync::{Arc, OnceLock}, time::Instant};
use base64::{engine::general_purpose::STANDARD, Engine};
use calamine::{Data, Reader, Xlsx};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::drivers::{ColumnInfo, quote_identifier};

const MAX_FILE: usize = 8 * 1024 * 1024;
const MAX_ROWS: usize = 10_000;
const MAX_COLUMNS: usize = 128;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRequest { pub filename: String, pub content_base64: String, pub sheet: Option<String> }
#[derive(Serialize)]
pub struct FilePreview { pub columns: Vec<String>, pub rows: Vec<Vec<Option<String>>>, pub row_count: usize, pub sheets: Vec<String>, pub sheet: Option<String>, pub clipped_preview: bool }
struct ParsedFile { columns: Vec<String>, rows: Vec<Vec<Option<String>>>, sheets: Vec<String>, sheet: Option<String> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnMapping { pub source: usize, pub target: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportRequest { pub file: FileRequest, pub connection_id: String, pub table: String, pub schema: Option<String>, pub mapping: Vec<ColumnMapping>, pub empty_as_null: bool }
#[derive(Serialize)]
pub struct LocalResult { pub columns: Vec<String>, pub rows: Vec<Value>, pub truncated: bool, pub execution_time_ms: u128 }

fn decode_file(file: &FileRequest) -> Result<(String, Vec<u8>), String> {
    if file.filename.len() > 512 || file.content_base64.len() > (MAX_FILE + 2) / 3 * 4 { return Err("Files must be at most 8 MiB".into()); }
    let extension = Path::new(&file.filename).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let bytes = STANDARD.decode(&file.content_base64).map_err(|_| "Invalid file encoding")?;
    if bytes.is_empty() || bytes.len() > MAX_FILE { return Err("Choose a nonempty file of at most 8 MiB".into()); }
    Ok((extension, bytes))
}
fn validate_headers(columns: &[String]) -> Result<(), String> {
    let mut seen = HashSet::new();
    if columns.is_empty() || columns.len() > MAX_COLUMNS { return Err("Files need between 1 and 128 columns".into()); }
    for column in columns {
        if column.trim().is_empty() || column.len() > 256 || !seen.insert(column) { return Err("Headers must be nonempty, unique, and at most 256 bytes".into()); }
    }
    Ok(())
}
fn check_xlsx_archive(bytes: &[u8]) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "Invalid XLSX archive")?;
    if archive.len() > 512 { return Err("Workbook contains too many archive entries".into()); }
    let mut total = 0u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|e| e.to_string())?;
        if entry.size() > 32 * 1024 * 1024 { return Err("Workbook exceeds the 32 MiB expanded limit".into()); }
        let copied = std::io::copy(&mut entry.take(32 * 1024 * 1024 + 1), &mut std::io::sink()).map_err(|e| e.to_string())?;
        total += copied;
        if total > 32 * 1024 * 1024 { return Err("Workbook exceeds the 32 MiB expanded limit".into()); }
    }
    Ok(())
}
fn excel_value(value: Data) -> Result<Option<String>, String> {
    Ok(match value {
        Data::Empty => None,
        Data::Error(error) => return Err(format!("Workbook contains a cell error: {error}")),
        Data::DateTime(value) if !value.is_duration() => Some(value.as_datetime().ok_or("Invalid Excel date")?.format("%Y-%m-%d %H:%M:%S%.f").to_string()),
        other => Some(other.to_string()),
    })
}
fn parse_file(file: &FileRequest) -> Result<ParsedFile, String> {
    let (extension, bytes) = decode_file(file)?;
    match extension.as_str() {
        "csv" => {
            if file.sheet.is_some() { return Err("CSV files have no worksheets".into()); }
            let mut reader = csv::ReaderBuilder::new().from_reader(bytes.as_slice());
            let columns: Vec<String> = reader.headers().map_err(|e| e.to_string())?.iter().map(|s| s.trim_start_matches('\u{feff}').to_string()).collect();
            validate_headers(&columns)?;
            let mut rows = Vec::new();
            for row in reader.records() {
                if rows.len() == MAX_ROWS { return Err("Import is limited to 10,000 rows".into()); }
                rows.push(row.map_err(|e| e.to_string())?.iter().map(|s| Some(s.to_string())).collect());
            }
            Ok(ParsedFile { columns, rows, sheets: vec![], sheet: None })
        }
        "xlsx" => {
            check_xlsx_archive(&bytes)?;
            let mut workbook = Xlsx::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
            let sheets = workbook.sheet_names().to_vec();
            let sheet = file.sheet.clone().or_else(|| sheets.first().cloned()).ok_or("Workbook has no worksheets")?;
            let mut reader = workbook.worksheet_cells_reader(&sheet).map_err(|e| e.to_string())?;
            let mut cells: Vec<Vec<Option<String>>> = Vec::new();
            let mut first_row = None;
            let mut previous_position = None;
            while let Some(cell) = reader.next_cell().map_err(|e| e.to_string())? {
                let (row, column) = cell.get_position();
                if previous_position.is_some_and(|position| position >= (row, column)) { return Err("Workbook contains repeated or out-of-order cells".into()); }
                previous_position = Some((row, column));
                let start = *first_row.get_or_insert(row);
                let row = row.checked_sub(start).ok_or("Workbook cells are out of order")? as usize;
                let column = column as usize;
                if row > MAX_ROWS || column >= MAX_COLUMNS { return Err("Workbook exceeds 10,000 rows or 128 columns".into()); }
                cells.resize_with(row + 1, Vec::new);
                cells[row].resize(column + 1, None);
                cells[row][column] = excel_value(Data::from(cell.get_value().clone()))?;
            }
            if cells.is_empty() { return Err("Worksheet is empty".into()); }
            let columns: Vec<String> = cells.remove(0).into_iter().map(|cell| cell.unwrap_or_default()).collect();
            validate_headers(&columns)?;
            for row in &mut cells {
                if row.len() > columns.len() { return Err("Worksheet has data beyond its header columns".into()); }
                row.resize(columns.len(), None);
            }
            Ok(ParsedFile { columns, rows: cells, sheets, sheet: Some(sheet) })
        }
        _ => Err("Import supports CSV and XLSX files".into()),
    }
}
#[tauri::command]
pub async fn preview_import_file(file: FileRequest) -> Result<FilePreview, String> {
    tokio::task::spawn_blocking(move || {
        let parsed = parse_file(&file)?;
        let row_count = parsed.rows.len();
        let mut clipped_preview = false;
        let rows = parsed.rows.into_iter().take(20).map(|row| row.into_iter().map(|cell| cell.map(|text| {
            if text.chars().count() > 2000 { clipped_preview = true; text.chars().take(2000).collect() } else { text }
        })).collect()).collect();
        Ok(FilePreview { columns: parsed.columns, rows, row_count, sheets: parsed.sheets, sheet: parsed.sheet, clipped_preview })
    }).await.map_err(|e| e.to_string())?
}
fn map_rows(parsed: ParsedFile, mapping: &[ColumnMapping], targets: &[ColumnInfo], empty_as_null: bool) -> Result<(Vec<String>, Vec<Vec<Option<String>>>), String> {
    if mapping.is_empty() || mapping.len() > MAX_COLUMNS { return Err("Map at least one source column".into()); }
    let mut seen = HashSet::new();
    let mut columns = Vec::new();
    for entry in mapping {
        if entry.source >= parsed.columns.len() || !targets.iter().any(|column| column.name == entry.target) || !seen.insert(&entry.target) { return Err("Mapping contains an unknown source, unknown target, or repeated target column".into()); }
        columns.push(entry.target.clone());
    }
    if parsed.rows.is_empty() { return Err("File contains no data rows".into()); }
    let rows = parsed.rows.into_iter().map(|row| mapping.iter().map(|entry| {
        let value = row[entry.source].clone();
        if empty_as_null && value.as_ref().is_some_and(String::is_empty) { None } else { value }
    }).collect()).collect();
    Ok((columns, rows))
}
fn postgres_import_type(data_type: &str) -> Result<&str, String> {
    // Keep text unbounded: explicit character casts silently truncate before assignment.
    match data_type {
        "text" | "character varying" | "character" => Ok("text"),
        "smallint" | "integer" | "bigint" | "real" | "double precision" | "numeric" | "decimal" | "boolean" | "date" | "time without time zone" | "time with time zone" | "timestamp without time zone" | "timestamp with time zone" | "json" | "jsonb" | "uuid" | "bytea" => Ok(data_type),
        _ => Err(format!("PostgreSQL import does not support {data_type}; omit that column")),
    }
}
#[tauri::command]
pub async fn import_table_file(request: ImportRequest) -> Result<usize, String> {
    if request.connection_id.is_empty() || request.table.is_empty() || request.table.len() > 512 || request.schema.as_ref().is_some_and(|s| s.is_empty() || s.len() > 512) { return Err("Choose a valid saved connection and table".into()); }
    let (engine, uri, read_only, lease) = crate::resolve_connection(&request.connection_id).await?;
    if read_only { return Err(crate::read_only::POLICY_ERROR.into()); }
    if !["postgres", "mysql", "mariadb", "sqlite"].contains(&engine.as_str()) { return Err("Import supports SQL tables only".into()); }
    let parsed = tokio::task::spawn_blocking(move || parse_file(&request.file)).await.map_err(|e| e.to_string())??;
    let driver = crate::drivers::create_driver_with_policy(&engine, &uri, false, lease.is_some()).await.map_err(|e| e.to_string())?;
    let targets = driver.get_table_columns(&request.table, request.schema.as_deref()).await.map_err(|e| e.to_string())?;
    drop(driver);
    let (columns, rows) = map_rows(parsed, &request.mapping, &targets, request.empty_as_null)?;
    let table = quote_identifier(&engine, &request.table, request.schema.as_deref());
    let count = rows.len();
    match engine.as_str() {
        "sqlite" => crate::drivers::sqlite::SqliteDriver::new(&uri).map_err(|e| e.to_string())?.import_rows(&table, &columns, &rows).await.map_err(|e| e.to_string())?,
        "postgres" => {
            let types: Result<Vec<_>, _> = columns.iter().map(|name| {
                let column = targets.iter().find(|column| &column.name == name).unwrap();
                let data_type = column.data_type.as_str();
                postgres_import_type(data_type).map(str::to_string)
            }).collect();
            crate::drivers::postgres::PostgresDriver::new(&uri).await.map_err(|e| e.to_string())?.import_rows(&table, &columns, &types?, &rows).await.map_err(|e| e.to_string())?;
        }
        _ => crate::drivers::mysql::MysqlDriver::new_with_policy(&uri, false, true).await.map_err(|e| e.to_string())?.import_rows(&request.table, request.schema.as_deref(), &table, &columns, &rows).await.map_err(|e| e.to_string())?,
    }
    Ok(count)
}

struct InterruptOnDrop(Arc<duckdb::InterruptHandle>);
impl Drop for InterruptOnDrop { fn drop(&mut self) { self.0.interrupt(); } }
static LOCAL_QUERY_LOCK: OnceLock<Arc<tokio::sync::Mutex<()>>> = OnceLock::new();
#[tauri::command]
pub async fn query_local_file(file: FileRequest, query: String) -> Result<LocalResult, String> {
    // ponytail: one native file query at a time; per-session databases if parallel work is needed.
    let guard = LOCAL_QUERY_LOCK.get_or_init(|| Arc::new(tokio::sync::Mutex::new(()))).clone().lock_owned().await;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let parent = crate::get_app_data_dir();
    let mut task = tokio::task::spawn_blocking(move || {
        let _guard = guard;
        let conn = duckdb::Connection::open_in_memory().map_err(|e| e.to_string())?;
        sender.send(conn.interrupt_handle()).map_err(|_| "File query was cancelled")?;
        run_local_file(&conn, &file, &query, &parent)
    });
    let interrupt = InterruptOnDrop(receiver.await.map_err(|_| "Could not start DuckDB")?);
    let result = match tokio::time::timeout(std::time::Duration::from_secs(30), &mut task).await {
        Ok(result) => result.map_err(|e| e.to_string())?,
        Err(_) => { interrupt.0.interrupt(); let _ = task.await; Err("Local query exceeded 30 seconds".into()) }
    };
    result
}
fn run_local_file(conn: &duckdb::Connection, file: &FileRequest, query: &str, parent: &Path) -> Result<LocalResult, String> {
    let start = Instant::now();
    let (extension, bytes) = decode_file(file)?;
    if !["csv", "json", "parquet"].contains(&extension.as_str()) || file.sheet.is_some() { return Err("Local queries support CSV, JSON, and Parquet".into()); }
    crate::read_only::check_query("postgres", query).map_err(|_| "Enter one read-only SQL query")?;
    let query = crate::read_only::without_terminators("postgres", query)?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = tempfile::Builder::new().prefix("qs-local-").tempdir_in(parent).map_err(|e| e.to_string())?;
    let path = temporary.path().join(format!("data.{extension}"));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    conn.execute_batch("SET memory_limit='256MB'; SET threads=2; SET max_temp_directory_size='0B'; SET autoinstall_known_extensions=false;").map_err(|e| e.to_string())?;
    let reader = match extension.as_str() { "csv" => "read_csv_auto", "json" => "read_json_auto", _ => "read_parquet" };
    conn.execute(&format!("CREATE TABLE data AS SELECT * FROM {reader}(?)"), [path.to_str().ok_or("Invalid temporary path")?]).map_err(|e| e.to_string())?;
    conn.execute_batch("SET enable_external_access=false; SET autoload_known_extensions=false; SET lock_configuration=true;").map_err(|e| e.to_string())?;
    let mut description = conn.prepare(&format!("DESCRIBE SELECT * FROM (\n{query}\n) AS result")).map_err(|e| e.to_string())?;
    let columns: Vec<String> = description.query_map([], |row| row.get(0)).map_err(|e| e.to_string())?.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
    if columns.len() > MAX_COLUMNS { return Err("Query results are limited to 128 columns".into()); }
    let mut statement = conn.prepare(&format!("SELECT to_json(result)::VARCHAR FROM (\n{query}\n) AS result LIMIT 501")).map_err(|e| e.to_string())?;
    let mut cursor = statement.query([]).map_err(|e| e.to_string())?;
    let mut rows = Vec::new(); let mut size = 0;
    while let Some(row) = cursor.next().map_err(|e| e.to_string())? {
        let text: String = row.get(0).map_err(|e| e.to_string())?;
        size += text.len(); if size > 2 * 1024 * 1024 { return Err("Result exceeds 2 MiB; select fewer columns or rows".into()); }
        rows.push(serde_json::from_str(&text).map_err(|e| e.to_string())?);
    }
    let truncated = rows.len() > 500; rows.truncate(500);
    Ok(LocalResult { columns, rows, truncated, execution_time_ms: start.elapsed().as_millis() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drivers::DatabaseDriver;
    fn file(name: &str, bytes: &[u8]) -> FileRequest { FileRequest { filename: name.into(), content_base64: STANDARD.encode(bytes), sheet: None } }
    fn column(name: &str) -> ColumnInfo { ColumnInfo { name:name.into(), data_type:"TEXT".into(), is_nullable:true, column_default:None, is_primary_key:false, is_foreign_key:false, referenced_table:None, referenced_column:None } }
    #[test]
    fn csv_quotes_headers_lengths_and_mapping_nulls() {
        let request = file("people.csv", b"id,name,note\r\n1,\"Ada, A\",\"line one\nline two\"\r\n2,Ben,\r\n");
        let parsed = parse_file(&request).unwrap();
        assert_eq!(parsed.rows[0][1].as_deref(), Some("Ada, A"));
        assert_eq!(parsed.rows[0][2].as_deref(), Some("line one\nline two"));
        let mapping = [ColumnMapping { source:2,target:"note".into() }];
        let (_, rows) = map_rows(parsed, &mapping, &[column("note")], true).unwrap();
        assert_eq!(rows[1][0], None);
        let (_, rows) = map_rows(parse_file(&request).unwrap(), &mapping, &[column("note")], false).unwrap();
        assert_eq!(rows[1][0].as_deref(), Some(""));
        for text in ["a,a\n1,2", ",b\n1,2", "a,b\n1", "a\n1,2"] { assert!(parse_file(&file("bad.csv", text.as_bytes())).is_err()); }
        assert!(map_rows(parse_file(&request).unwrap(), &[ColumnMapping { source:4,target:"note".into() }], &[column("note")], false).is_err());
        assert!(map_rows(parse_file(&request).unwrap(), &[ColumnMapping { source:0,target:"note".into() },ColumnMapping { source:1,target:"note".into() }], &[column("note")], false).is_err());
        assert!(decode_file(&file("empty.csv", b"")).is_err());
        assert!(parse_file(&file("too-many.csv", format!("id\n{}", "1\n".repeat(MAX_ROWS + 1)).as_bytes())).is_err());
    }
    #[test]
    fn postgres_character_import_keeps_text_unbounded() {
        for kind in ["text", "character varying", "character"] {
            assert_eq!(postgres_import_type(kind).unwrap(), "text");
        }
        assert_eq!(postgres_import_type("integer").unwrap(), "integer");
        assert!(postgres_import_type("USER-DEFINED").is_err());
        let parsed = parse_file(&file("names.csv", b"name\nAlice\n")).unwrap();
        let (_, rows) = map_rows(parsed, &[ColumnMapping { source: 0, target: "name".into() }], &[column("name")], false).unwrap();
        assert_eq!(rows[0][0].as_deref(), Some("Alice"));
    }
    #[test]
    fn xlsx_sheets_dates_sparse_cells_and_errors() {
        let mut workbook = rust_xlsxwriter::Workbook::new();
        let sheet = workbook.add_worksheet(); sheet.set_name("People").unwrap();
        sheet.write_string(0,0,"id").unwrap(); sheet.write_string(0,1,"name").unwrap();
        sheet.write_number(1,0,1.0).unwrap(); sheet.write_string(1,1,"Ada").unwrap();
        sheet.write_number(2,0,2.0).unwrap();
        workbook.add_worksheet().set_name("Other").unwrap().write_string(0,0,"empty_header_only").unwrap();
        let bytes = workbook.save_to_buffer().unwrap();
        let parsed = parse_file(&file("people.xlsx", &bytes)).unwrap();
        assert_eq!(parsed.sheets, ["People", "Other"]);
        assert_eq!(parsed.rows.len(), 2); assert_eq!(parsed.rows[1][1], None);
        let mut request = file("people.xlsx", &bytes); request.sheet = Some("Other".into());
        assert!(parse_file(&request).unwrap().rows.is_empty());
        request.sheet = Some("Missing".into()); assert!(parse_file(&request).is_err());
        assert!(parse_file(&file("broken.xlsx", b"not zip")).is_err());
        let mut bomb = Cursor::new(Vec::new());
        { let mut zip = zip::ZipWriter::new(&mut bomb); zip.start_file("oversized.xml", zip::write::SimpleFileOptions::default()).unwrap(); std::io::Write::write_all(&mut zip, &vec![b'a'; 32 * 1024 * 1024 + 1]).unwrap(); zip.finish().unwrap(); }
        assert!(check_xlsx_archive(bomb.get_ref()).is_err());
    }
    #[tokio::test]
    async fn sqlite_import_is_parameterized_and_rolls_back_all_rows() {
        let directory = tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap();
        let path = directory.path().join("import.sqlite");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE people(id INTEGER PRIMARY KEY, name TEXT NOT NULL);").unwrap(); drop(conn);
        let driver = crate::drivers::sqlite::SqliteDriver::new(path.to_str().unwrap()).unwrap();
        driver.import_rows("\"people\"", &["id".into(), "name".into()], &[vec![Some("1".into()),Some("O'Reilly; DROP TABLE people".into())],vec![Some("2".into()),Some("Ben".into())]]).await.unwrap();
        assert!(driver.import_rows("\"people\"", &["id".into(), "name".into()], &[vec![Some("3".into()),Some("Third".into())],vec![Some("1".into()),Some("Duplicate".into())]]).await.is_err());
        let rows = driver.execute_query("SELECT * FROM people ORDER BY id").await.unwrap();
        assert_eq!(rows.len(), 2); assert_eq!(rows[0]["name"], "O'Reilly; DROP TABLE people");
    }
    #[test]
    fn duckdb_queries_csv_json_parquet_and_blocks_external_reads_writes() {
        let directory = tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap();
        for request in [file("people.csv", b"id,name\n1,Ada\n2,Ben\n"), file("people.json", br#"[{"id":1,"name":"Ada"},{"id":2,"name":"Ben"}]"#)] {
            let conn = duckdb::Connection::open_in_memory().unwrap();
            let result = run_local_file(&conn, &request, "SELECT name FROM data WHERE id = 1; -- final comment", directory.path()).unwrap();
            assert_eq!(result.columns, ["name"]); assert_eq!(result.rows[0]["name"], "Ada");
        }
        let parquet = directory.path().join("fixture.parquet");
        let writer = duckdb::Connection::open_in_memory().unwrap();
        writer.execute("COPY (SELECT 1 AS id, 'Ada' AS name) TO ? (FORMAT PARQUET)", [parquet.to_str().unwrap()]).unwrap();
        let conn = duckdb::Connection::open_in_memory().unwrap();
        assert_eq!(run_local_file(&conn, &file("people.parquet", &std::fs::read(parquet).unwrap()), "SELECT * FROM data", directory.path()).unwrap().rows[0]["id"], 1);
        let csv = file("people.csv", b"id\n1\n");
        for query in ["DELETE FROM data", "SELECT 1; DELETE FROM data", "COPY data TO '/tmp/not-created.csv'", "SELECT * FROM read_csv_auto('/etc/passwd')", "SELECT * FROM read_json_auto('https://fixture.invalid/data.json')"] {
            let conn = duckdb::Connection::open_in_memory().unwrap();
            assert!(run_local_file(&conn, &csv, query, directory.path()).is_err(), "{query}");
        }
        let conn = duckdb::Connection::open_in_memory().unwrap();
        let many = file("many.csv", format!("id\n{}", (0..600).map(|id| format!("{id}\n")).collect::<String>()).as_bytes());
        let result = run_local_file(&conn, &many, "SELECT * FROM data", directory.path()).unwrap(); assert_eq!(result.rows.len(), 500); assert!(result.truncated);
    }
}
