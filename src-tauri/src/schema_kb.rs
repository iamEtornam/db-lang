use crate::app_db::{
    get_app_database, RelationshipDescriptionRecord, SchemaSnapshot,
    TableDescriptionRecord,
};
use crate::drivers::{create_driver, ColumnInfo};
use crate::gemini::call_llm_api_pub;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaKnowledgeBase {
    pub snapshot: SchemaSnapshot,
    pub tables: Vec<TableDescriptionRecord>,
    pub relationships: Vec<RelationshipDescriptionRecord>,
}

#[derive(Debug, Clone, Serialize)]
struct KbProgressEvent {
    connection_id: String,
    table_name: String,
    current: usize,
    total: usize,
    status: String,
}

pub fn get_schema_kb(connection_id: &str) -> Result<Option<SchemaKnowledgeBase>, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;

    let snapshot = db
        .get_latest_snapshot(connection_id)
        .map_err(|e| e.to_string())?;

    let Some(snap) = snapshot else {
        return Ok(None);
    };

    let tables = db
        .get_table_descriptions(&snap.id)
        .map_err(|e| e.to_string())?;

    let relationships = db
        .get_relationship_descriptions(&snap.id)
        .map_err(|e| e.to_string())?;

    Ok(Some(SchemaKnowledgeBase {
        snapshot: snap,
        tables,
        relationships,
    }))
}

pub async fn generate_schema_kb(
    connection_id: &str,
    engine: &str,
    conn_str: &str,
    app: &tauri::AppHandle,
) -> Result<String, String> {
    let db = get_app_database().map_err(|e| e.to_string())?;

    // Delete old snapshot
    db.delete_snapshot_for_connection(connection_id)
        .map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let snapshot_id = Uuid::new_v4().to_string();

    let snapshot = SchemaSnapshot {
        id: snapshot_id.clone(),
        connection_id: connection_id.to_string(),
        status: "generating".to_string(),
        summary: None,
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    db.upsert_schema_snapshot(&snapshot)
        .map_err(|e| e.to_string())?;

    // Connect to target DB
    let driver = create_driver(engine, conn_str)
        .await
        .map_err(|e| e.to_string())?;

    // Get tables and relationships
    let tables = driver.get_tables().await.map_err(|e| e.to_string())?;
    let relationships = driver.get_relationships().await.unwrap_or_default();
    let total = tables.len();

    let mut table_descriptions: Vec<TableDescriptionRecord> = Vec::new();

    // Process tables in batches of 10
    for (i, table) in tables.iter().enumerate() {
        let _ = app.emit(
            "schema_kb_progress",
            KbProgressEvent {
                connection_id: connection_id.to_string(),
                table_name: table.name.clone(),
                current: i + 1,
                total,
                status: "processing".to_string(),
            },
        );

        let columns = driver
            .get_table_columns(&table.name, table.schema.as_deref())
            .await
            .unwrap_or_default();

        let sample_rows = driver
            .preview_table_data(&table.name, table.schema.as_deref(), 5)
            .await
            .ok()
            .map(|rows| serde_json::to_string(&rows).unwrap_or_default());

        let column_metadata = serde_json::to_string(&columns).unwrap_or_default();

        // Ask LLM to describe the table
        let ai_description = describe_table_with_llm(
            &table.name,
            table.schema.as_deref(),
            &columns,
            sample_rows.as_deref(),
            engine,
        )
        .await
        .ok();

        let td_now = chrono::Utc::now().to_rfc3339();
        let td = TableDescriptionRecord {
            id: Uuid::new_v4().to_string(),
            snapshot_id: snapshot_id.clone(),
            table_name: table.name.clone(),
            schema_name: table.schema.clone(),
            table_type: table.table_type.clone(),
            ai_description,
            column_metadata,
            sample_data: sample_rows,
            created_at: td_now.clone(),
            updated_at: td_now,
        };

        db.upsert_table_description(&td)
            .map_err(|e| e.to_string())?;
        table_descriptions.push(td);
    }

    // Persist relationships
    for rel in &relationships {
        let rel_now = chrono::Utc::now().to_rfc3339();
        let rel_desc = RelationshipDescriptionRecord {
            id: Uuid::new_v4().to_string(),
            snapshot_id: snapshot_id.clone(),
            source_table: rel.source_table.clone(),
            source_column: rel.source_column.clone(),
            target_table: rel.target_table.clone(),
            target_column: rel.target_column.clone(),
            relationship_type: rel.relationship_type.clone(),
            ai_description: None,
            created_at: rel_now,
        };
        db.upsert_relationship_description(&rel_desc)
            .map_err(|e| e.to_string())?;
    }

    // Generate overall database summary
    let summary = generate_db_summary(&table_descriptions, &relationships, engine)
        .await
        .ok();

    // Update snapshot to ready
    let updated_snapshot = SchemaSnapshot {
        id: snapshot_id.clone(),
        connection_id: connection_id.to_string(),
        status: "ready".to_string(),
        summary,
        created_at: now,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    db.upsert_schema_snapshot(&updated_snapshot)
        .map_err(|e| e.to_string())?;

    let _ = app.emit(
        "schema_kb_progress",
        KbProgressEvent {
            connection_id: connection_id.to_string(),
            table_name: "".to_string(),
            current: total,
            total,
            status: "done".to_string(),
        },
    );

    Ok(snapshot_id)
}

pub async fn refresh_schema_kb(
    connection_id: &str,
    engine: &str,
    conn_str: &str,
    app: &tauri::AppHandle,
) -> Result<String, String> {
    generate_schema_kb(connection_id, engine, conn_str, app).await
}

pub fn update_table_description(table_desc_id: &str, description: &str) -> Result<(), String> {
    let db = get_app_database().map_err(|e| e.to_string())?;
    db.update_table_description_text(table_desc_id, description)
        .map_err(|e| e.to_string())
}

async fn describe_table_with_llm(
    table_name: &str,
    schema_name: Option<&str>,
    columns: &[ColumnInfo],
    sample_data: Option<&str>,
    engine: &str,
) -> Result<String, String> {
    let full_name = match schema_name {
        Some(s) => format!("{}.{}", s, table_name),
        None => table_name.to_string(),
    };

    let cols_desc: Vec<String> = columns
        .iter()
        .map(|c| {
            let mut desc = format!("  - {} ({})", c.name, c.data_type);
            if c.is_primary_key {
                desc.push_str(" [PRIMARY KEY]");
            }
            if c.is_foreign_key {
                if let (Some(ref_table), Some(ref_col)) =
                    (&c.referenced_table, &c.referenced_column)
                {
                    desc.push_str(&format!(" [FK -> {}.{}]", ref_table, ref_col));
                }
            }
            if !c.is_nullable {
                desc.push_str(" NOT NULL");
            }
            desc
        })
        .collect();

    let mut prompt = format!(
        r#"You are a database documentation expert. Analyze this {} database table and provide a concise, helpful description.

Table: {}
Columns:
{}
"#,
        engine,
        full_name,
        cols_desc.join("\n")
    );

    if let Some(sample) = sample_data {
        if !sample.is_empty() && sample != "[]" {
            prompt.push_str(&format!("\nSample data (first few rows):\n{}\n", sample));
        }
    }

    prompt.push_str(
        r#"
Provide a 2-3 sentence description covering:
1. What this table represents (business entity/concept)
2. What each key column stores
3. How it relates to other tables (if FK relationships are visible)

Return ONLY the description text, no JSON, no markdown, no prefix."#,
    );

    call_llm_api_pub(&prompt).await.map_err(|e| e.to_string())
}

async fn generate_db_summary(
    tables: &[TableDescriptionRecord],
    relationships: &[crate::drivers::Relationship],
    engine: &str,
) -> Result<String, String> {
    if tables.is_empty() {
        return Ok("Empty database with no tables.".to_string());
    }

    let table_list: Vec<String> = tables
        .iter()
        .map(|t| {
            let desc = t
                .ai_description
                .as_deref()
                .unwrap_or("No description")
                .split(". ")
                .next()
                .unwrap_or("");
            format!("- {}: {}", t.table_name, desc)
        })
        .collect();

    let rel_list: Vec<String> = relationships
        .iter()
        .map(|r| {
            format!(
                "- {}.{} -> {}.{}",
                r.source_table, r.source_column, r.target_table, r.target_column
            )
        })
        .collect();

    let prompt = format!(
        r#"You are a database documentation expert. Given the following {} database tables and relationships, write a concise 2-3 paragraph summary of what this database is for.

Tables ({} total):
{}

Foreign Key Relationships:
{}

Write a clear summary that:
1. Identifies the main domain/purpose of this database
2. Describes the core entities and their roles
3. Explains the key relationships between tables

Return ONLY the summary text, no JSON, no markdown."#,
        engine,
        tables.len(),
        table_list.join("\n"),
        if rel_list.is_empty() { "None detected".to_string() } else { rel_list.join("\n") }
    );

    call_llm_api_pub(&prompt).await.map_err(|e| e.to_string())
}

/// Build schema context string for AI query translation
pub fn build_schema_context(kb: &SchemaKnowledgeBase) -> String {
    let mut ctx = String::new();

    if let Some(ref summary) = kb.snapshot.summary {
        ctx.push_str(&format!("## Database Overview\n{}\n\n", summary));
    }

    ctx.push_str("## Tables\n");
    for table in &kb.tables {
        ctx.push_str(&format!("### {}", table.table_name));
        if let Some(ref schema) = table.schema_name {
            ctx.push_str(&format!(" (schema: {})", schema));
        }
        ctx.push('\n');

        if let Some(ref desc) = table.ai_description {
            ctx.push_str(&format!("{}\n", desc));
        }

        // Parse column metadata
        if let Ok(cols) = serde_json::from_str::<Vec<ColumnInfo>>(&table.column_metadata) {
            ctx.push_str("Columns:\n");
            for col in &cols {
                let mut col_desc = format!("  - {} ({})", col.name, col.data_type);
                if col.is_primary_key {
                    col_desc.push_str(" PK");
                }
                if col.is_foreign_key {
                    if let (Some(ref_t), Some(ref_c)) = (&col.referenced_table, &col.referenced_column) {
                        col_desc.push_str(&format!(" FK->{}.{}", ref_t, ref_c));
                    }
                }
                ctx.push_str(&col_desc);
                ctx.push('\n');
            }
        }
        ctx.push('\n');
    }

    if !kb.relationships.is_empty() {
        ctx.push_str("## Relationships\n");
        for rel in &kb.relationships {
            ctx.push_str(&format!(
                "- {}.{} -> {}.{} ({})\n",
                rel.source_table,
                rel.source_column,
                rel.target_table,
                rel.target_column,
                rel.relationship_type.as_deref().unwrap_or("FK")
            ));
        }
    }

    ctx
}
