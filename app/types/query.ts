export interface TranslationResult {
  query: string
  query_language: string
  tables_used: string[]
  confidence: number
  explanation: string
}

export interface ClauseExplanation {
  clause_type: string
  content: string
  explanation: string
}

export interface QueryExplanation {
  summary: string
  clauses: ClauseExplanation[]
  tables_involved: string[]
  potential_issues: string[]
  optimization_tips: string[]
}

export interface ChartConfig {
  chart_type: 'bar' | 'line' | 'area' | 'pie' | 'scatter' | 'table'
  title: string
  x_axis: {
    field: string
    label: string
  }
  y_axis: {
    field: string
    label: string
  }
  series: Array<{
    field: string
    label: string
    color?: string
  }>
  explanation: string
}

export interface QueryResult {
  rows: Record<string, unknown>[]
  columns: string[]
  total_count: number | null
  page: number
  page_size: number
  has_more: boolean
  execution_time_ms: number
}
