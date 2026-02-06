use base64::{engine::general_purpose::STANDARD, Engine};
use rust_xlsxwriter::{Format, Workbook};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tauri::command;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExportError {
    #[error("Invalid data format: {0}")]
    InvalidData(String),
    #[error("Export failed: {0}")]
    ExportFailed(String),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Excel error: {0}")]
    ExcelError(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportOptions {
    pub format: String,        // "csv", "json", "xlsx"
    pub columns: Option<Vec<String>>,  // specific columns to export, None means all
    pub include_headers: bool,
    pub delimiter: Option<String>,  // for CSV, default is ","
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            format: "csv".to_string(),
            columns: None,
            include_headers: true,
            delimiter: Some(",".to_string()),
        }
    }
}

/// Export data to CSV format
fn export_to_csv(
    data: &[HashMap<String, Value>],
    columns: &[String],
    include_headers: bool,
    delimiter: &str,
) -> Result<String, ExportError> {
    let mut output = String::new();

    // Write headers
    if include_headers && !columns.is_empty() {
        let header_line: Vec<String> = columns
            .iter()
            .map(|col| escape_csv_field(col, delimiter))
            .collect();
        output.push_str(&header_line.join(delimiter));
        output.push('\n');
    }

    // Write data rows
    for row in data {
        let row_values: Vec<String> = columns
            .iter()
            .map(|col| {
                let value = row.get(col).unwrap_or(&Value::Null);
                let str_value = value_to_string(value);
                escape_csv_field(&str_value, delimiter)
            })
            .collect();
        output.push_str(&row_values.join(delimiter));
        output.push('\n');
    }

    Ok(output)
}

/// Escape a CSV field if necessary
fn escape_csv_field(field: &str, delimiter: &str) -> String {
    let needs_escaping = field.contains(delimiter)
        || field.contains('"')
        || field.contains('\n')
        || field.contains('\r');

    if needs_escaping {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Convert a JSON value to string representation
fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(arr) => serde_json::to_string(arr).unwrap_or_default(),
        Value::Object(obj) => serde_json::to_string(obj).unwrap_or_default(),
    }
}

/// Export data to JSON format
fn export_to_json(
    data: &[HashMap<String, Value>],
    columns: &[String],
) -> Result<String, ExportError> {
    let filtered_data: Vec<HashMap<String, Value>> = data
        .iter()
        .map(|row| {
            columns
                .iter()
                .filter_map(|col| {
                    row.get(col).map(|v| (col.clone(), v.clone()))
                })
                .collect()
        })
        .collect();

    serde_json::to_string_pretty(&filtered_data)
        .map_err(|e| ExportError::ExportFailed(e.to_string()))
}

/// Export data to Excel format (xlsx)
fn export_to_xlsx(
    data: &[HashMap<String, Value>],
    columns: &[String],
    sheet_name: Option<&str>,
) -> Result<Vec<u8>, ExportError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook
        .add_worksheet()
        .set_name(sheet_name.unwrap_or("Query Results"))
        .map_err(|e| ExportError::ExcelError(e.to_string()))?;

    // Create header format
    let header_format = Format::new()
        .set_bold()
        .set_background_color(rust_xlsxwriter::Color::RGB(0x2A3637))
        .set_font_color(rust_xlsxwriter::Color::White)
        .set_border(rust_xlsxwriter::FormatBorder::Thin);

    // Create data format
    let data_format = Format::new()
        .set_border(rust_xlsxwriter::FormatBorder::Thin);

    // Create number format
    let number_format = Format::new()
        .set_border(rust_xlsxwriter::FormatBorder::Thin)
        .set_num_format("#,##0.##");

    // Create date format
    let date_format = Format::new()
        .set_border(rust_xlsxwriter::FormatBorder::Thin)
        .set_num_format("yyyy-mm-dd hh:mm:ss");

    // Write headers
    for (col_idx, col_name) in columns.iter().enumerate() {
        worksheet
            .write_string_with_format(0, col_idx as u16, col_name, &header_format)
            .map_err(|e| ExportError::ExcelError(e.to_string()))?;
        
        // Set reasonable column width based on header length (min 10, max 50)
        let width = (col_name.len() as f64 * 1.2).max(10.0).min(50.0);
        worksheet
            .set_column_width(col_idx as u16, width)
            .map_err(|e| ExportError::ExcelError(e.to_string()))?;
    }

    // Write data rows
    for (row_idx, row) in data.iter().enumerate() {
        let excel_row = (row_idx + 1) as u32; // +1 for header

        for (col_idx, col_name) in columns.iter().enumerate() {
            let col = col_idx as u16;
            let value = row.get(col_name).unwrap_or(&Value::Null);

            match value {
                Value::Null => {
                    worksheet
                        .write_blank(excel_row, col, &data_format)
                        .map_err(|e| ExportError::ExcelError(e.to_string()))?;
                }
                Value::Bool(b) => {
                    worksheet
                        .write_boolean_with_format(excel_row, col, *b, &data_format)
                        .map_err(|e| ExportError::ExcelError(e.to_string()))?;
                }
                Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        worksheet
                            .write_number_with_format(excel_row, col, f, &number_format)
                            .map_err(|e| ExportError::ExcelError(e.to_string()))?;
                    } else if let Some(i) = n.as_i64() {
                        worksheet
                            .write_number_with_format(excel_row, col, i as f64, &number_format)
                            .map_err(|e| ExportError::ExcelError(e.to_string()))?;
                    }
                }
                Value::String(s) => {
                    // Check if it's a date string
                    if is_date_string(s) {
                        worksheet
                            .write_string_with_format(excel_row, col, s, &date_format)
                            .map_err(|e| ExportError::ExcelError(e.to_string()))?;
                    } else {
                        worksheet
                            .write_string_with_format(excel_row, col, s, &data_format)
                            .map_err(|e| ExportError::ExcelError(e.to_string()))?;
                    }
                }
                Value::Array(arr) => {
                    let str_val = serde_json::to_string(arr).unwrap_or_default();
                    worksheet
                        .write_string_with_format(excel_row, col, &str_val, &data_format)
                        .map_err(|e| ExportError::ExcelError(e.to_string()))?;
                }
                Value::Object(obj) => {
                    let str_val = serde_json::to_string(obj).unwrap_or_default();
                    worksheet
                        .write_string_with_format(excel_row, col, &str_val, &data_format)
                        .map_err(|e| ExportError::ExcelError(e.to_string()))?;
                }
            }
        }
    }

    // Freeze the header row
    worksheet
        .set_freeze_panes(1, 0)
        .map_err(|e| ExportError::ExcelError(e.to_string()))?;

    // Save to buffer
    let buffer = workbook
        .save_to_buffer()
        .map_err(|e| ExportError::ExcelError(e.to_string()))?;

    Ok(buffer)
}

/// Check if a string looks like a date
fn is_date_string(s: &str) -> bool {
    // Simple date pattern checks
    let patterns = [
        r"^\d{4}-\d{2}-\d{2}", // ISO date
        r"^\d{2}/\d{2}/\d{4}", // US date
        r"^\d{2}-\d{2}-\d{4}", // EU date
    ];
    
    for pattern in patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if re.is_match(s) {
                return true;
            }
        }
    }
    false
}

/// Get all column names from data
fn get_columns_from_data(data: &[HashMap<String, Value>]) -> Vec<String> {
    if data.is_empty() {
        return Vec::new();
    }
    data[0].keys().cloned().collect()
}

/// Export result for xlsx (returns base64 encoded data)
#[derive(Serialize)]
pub struct ExportResult {
    pub data: String,
    pub is_binary: bool,
    pub filename_extension: String,
}

/// Export query results to a specified format
#[command]
pub async fn export_data(
    data_json: String,
    format: String,
    columns: Option<Vec<String>>,
    include_headers: Option<bool>,
    delimiter: Option<String>,
    sheet_name: Option<String>,
) -> Result<ExportResult, String> {
    // Parse the input data
    let data: Vec<HashMap<String, Value>> = serde_json::from_str(&data_json)
        .map_err(|e| format!("Failed to parse data: {}", e))?;

    if data.is_empty() {
        return Err("No data to export".to_string());
    }

    // Determine columns to export
    let export_columns = columns.unwrap_or_else(|| get_columns_from_data(&data));
    
    if export_columns.is_empty() {
        return Err("No columns to export".to_string());
    }

    let include_headers = include_headers.unwrap_or(true);
    let delimiter = delimiter.unwrap_or_else(|| ",".to_string());

    match format.to_lowercase().as_str() {
        "csv" => {
            let result = export_to_csv(&data, &export_columns, include_headers, &delimiter)
                .map_err(|e| e.to_string())?;
            Ok(ExportResult {
                data: result,
                is_binary: false,
                filename_extension: "csv".to_string(),
            })
        }
        "json" => {
            let result = export_to_json(&data, &export_columns)
                .map_err(|e| e.to_string())?;
            Ok(ExportResult {
                data: result,
                is_binary: false,
                filename_extension: "json".to_string(),
            })
        }
        "tsv" => {
            let result = export_to_csv(&data, &export_columns, include_headers, "\t")
                .map_err(|e| e.to_string())?;
            Ok(ExportResult {
                data: result,
                is_binary: false,
                filename_extension: "tsv".to_string(),
            })
        }
        "xlsx" | "excel" => {
            let sheet = sheet_name.as_deref();
            let buffer = export_to_xlsx(&data, &export_columns, sheet)
                .map_err(|e| e.to_string())?;
            
            // Encode as base64 for transfer to frontend
            let base64_data = STANDARD.encode(&buffer);
            
            Ok(ExportResult {
                data: base64_data,
                is_binary: true,
                filename_extension: "xlsx".to_string(),
            })
        }
        _ => Err(format!("Unsupported export format: {}", format)),
    }
}

/// Get available columns from data
#[command]
pub async fn get_export_columns(data_json: String) -> Result<Vec<String>, String> {
    let data: Vec<HashMap<String, Value>> = serde_json::from_str(&data_json)
        .map_err(|e| format!("Failed to parse data: {}", e))?;
    
    Ok(get_columns_from_data(&data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_export() {
        let mut row1 = HashMap::new();
        row1.insert("id".to_string(), Value::Number(1.into()));
        row1.insert("name".to_string(), Value::String("Alice".to_string()));
        
        let mut row2 = HashMap::new();
        row2.insert("id".to_string(), Value::Number(2.into()));
        row2.insert("name".to_string(), Value::String("Bob".to_string()));

        let data = vec![row1, row2];
        let columns = vec!["id".to_string(), "name".to_string()];

        let result = export_to_csv(&data, &columns, true, ",").unwrap();
        assert!(result.contains("id,name"));
        assert!(result.contains("1,Alice"));
        assert!(result.contains("2,Bob"));
    }

    #[test]
    fn test_csv_escape() {
        assert_eq!(escape_csv_field("simple", ","), "simple");
        assert_eq!(escape_csv_field("with,comma", ","), "\"with,comma\"");
        assert_eq!(escape_csv_field("with\"quote", ","), "\"with\"\"quote\"");
    }
}
