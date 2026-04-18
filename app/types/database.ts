export interface Connection {
  id: string
  name: string
  db_type: string
  host: string
  port: string
  database: string
  username: string
  password: string
  ssl_enabled: boolean
  auth_json: string
  created_at: string
  updated_at: string
}

export interface CreateConnectionRequest {
  name: string
  db_type: string
  host: string
  port: string
  database: string
  username: string
  password: string
  ssl_enabled: boolean
  auth_json?: string
}

export interface TableInfo {
  name: string
  schema: string | null
  table_type: string
}

export interface ColumnInfo {
  name: string
  data_type: string
  is_nullable: boolean
  column_default: string | null
  is_primary_key: boolean
  is_foreign_key: boolean
  referenced_table: string | null
  referenced_column: string | null
}

export interface TableSchema {
  table: TableInfo
  columns: ColumnInfo[]
}

export interface PaginatedResult {
  data: string
  total_count: number | null
  page: number
  page_size: number
  has_more: boolean
}

export interface UserSettings {
  theme: string
  default_page_size: number
  query_timeout_seconds: number
  auto_save_history: boolean
  created_at: string
  updated_at: string
}

export interface LlmConfig {
  provider: string
  model: string
  api_key: string
  api_url: string | null
  created_at: string
  updated_at: string
}

export interface QueryHistory {
  id: string
  connection_id: string
  natural_query: string
  sql_query: string
  result_count: number | null
  execution_time_ms: number | null
  status: string
  error_message: string | null
  created_at: string
}

export interface Snippet {
  id: string
  name: string
  description: string | null
  natural_query: string
  sql_query: string
  tags: string
  created_at: string
  updated_at: string
}

// Schema Knowledge Base types
export interface SchemaSnapshot {
  id: string
  connection_id: string
  status: 'generating' | 'ready' | 'error'
  summary: string | null
  created_at: string
  updated_at: string
}

export interface TableDescription {
  id: string
  snapshot_id: string
  table_name: string
  schema_name: string | null
  table_type: string
  ai_description: string | null
  column_metadata: string // JSON
  sample_data: string | null // JSON
}

export interface RelationshipDescription {
  id: string
  snapshot_id: string
  source_table: string
  source_column: string
  target_table: string
  target_column: string
  relationship_type: string | null
  ai_description: string | null
}

export interface SchemaKnowledgeBase {
  snapshot: SchemaSnapshot
  tables: TableDescription[]
  relationships: RelationshipDescription[]
}
