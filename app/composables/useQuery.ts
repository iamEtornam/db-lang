import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import type { PaginatedResult } from '~/types/database'
import type { ChartConfig, QueryExplanation, QueryResult, TranslationResult } from '~/types/query'

export function useQuery() {
  const isTranslating = ref(false)
  const isExecuting = ref(false)
  const isGeneratingChart = ref(false)
  const isExplaining = ref(false)

  const naturalQuery = ref('')
  const generatedQuery = ref('')
  const queryLanguage = ref('sql')
  const tablesUsed = ref<string[]>([])
  const translationExplanation = ref('')
  const confidence = ref(0)

  const queryResult = ref<QueryResult | null>(null)
  const queryExplanation = ref<QueryExplanation | null>(null)
  const chartConfig = ref<ChartConfig | null>(null)
  const dataExplanation = ref('')

  const error = ref<string | null>(null)

  async function translateQuery(nl: string, connectionId: string, engine: string): Promise<TranslationResult | null> {
    isTranslating.value = true
    error.value = null

    try {
      const result = await invoke<TranslationResult>('translate_to_query', {
        naturalLanguage: nl,
        connectionId,
        engine,
      })

      generatedQuery.value = result.query
      queryLanguage.value = result.query_language
      tablesUsed.value = result.tables_used
      translationExplanation.value = result.explanation
      confidence.value = result.confidence

      return result
    }
    catch (err) {
      // Fallback to old translate_to_sql for compatibility
      try {
        const sql = await invoke<string>('translate_to_sql', { query: nl })
        generatedQuery.value = sql
        queryLanguage.value = 'sql'
        tablesUsed.value = []
        translationExplanation.value = ''
        confidence.value = 0
        return {
          query: sql,
          query_language: 'sql',
          tables_used: [],
          confidence: 0,
          explanation: '',
        }
      }
      catch (fallbackErr) {
        error.value = fallbackErr as string
        toast.error('Translation failed', { description: fallbackErr as string })
        return null
      }
    }
    finally {
      isTranslating.value = false
    }
  }

  async function executeQuery(
    query: string,
    engine: string,
    connStr: string,
    page = 1,
    pageSize = 50,
  ): Promise<QueryResult | null> {
    isExecuting.value = true
    error.value = null
    queryExplanation.value = null
    chartConfig.value = null
    dataExplanation.value = ''

    const startTime = Date.now()

    try {
      const isSelect = query.trim().toUpperCase().startsWith('SELECT')
        || query.trim().startsWith('[') // MongoDB aggregation pipeline
        || query.trim().startsWith('{') // MongoDB query

      if (isSelect) {
        const result = await invoke<PaginatedResult>('query_db_paginated', {
          engine,
          connStr,
          query,
          page,
          pageSize,
        })

        const rows = JSON.parse(result.data) as Record<string, unknown>[]
        const columns = rows.length > 0 ? Object.keys(rows[0]) : []

        const queryResultData: QueryResult = {
          rows,
          columns,
          total_count: result.total_count,
          page: result.page,
          page_size: result.page_size,
          has_more: result.has_more,
          execution_time_ms: Date.now() - startTime,
        }

        queryResult.value = queryResultData
        return queryResultData
      }
      else {
        const data = await invoke<string>('query_db', { engine, connStr, query })
        const rows = JSON.parse(data) as Record<string, unknown>[]
        const columns = rows.length > 0 ? Object.keys(rows[0]) : []

        const queryResultData: QueryResult = {
          rows,
          columns,
          total_count: rows.length,
          page: 1,
          page_size: rows.length,
          has_more: false,
          execution_time_ms: Date.now() - startTime,
        }

        queryResult.value = queryResultData
        return queryResultData
      }
    }
    catch (err) {
      error.value = err as string
      toast.error('Query failed', { description: err as string })
      return null
    }
    finally {
      isExecuting.value = false
    }
  }

  async function explainQuery(sql: string, dbType = 'SQL'): Promise<QueryExplanation | null> {
    isExplaining.value = true

    try {
      const result = await invoke<QueryExplanation>('explain_query', { sqlQuery: sql })
      queryExplanation.value = result
      return result
    }
    catch (err) {
      toast.error('Explanation failed', { description: err as string })
      return null
    }
    finally {
      isExplaining.value = false
    }
  }

  async function generateChart(
    rows: Record<string, unknown>[],
    columns: string[],
    nl: string,
    engine: string,
  ): Promise<ChartConfig | null> {
    isGeneratingChart.value = true

    try {
      const result = await invoke<ChartConfig>('generate_chart_config', {
        data: JSON.stringify(rows.slice(0, 50)),
        columns,
        query: nl,
        engine,
      })
      chartConfig.value = result
      return result
    }
    catch (err) {
      toast.error('Chart generation failed', { description: err as string })
      return null
    }
    finally {
      isGeneratingChart.value = false
    }
  }

  async function explainData(
    rows: Record<string, unknown>[],
    columns: string[],
    nl: string,
  ): Promise<string | null> {
    try {
      const result = await invoke<string>('explain_data', {
        data: JSON.stringify(rows.slice(0, 100)),
        columns,
        query: nl,
      })
      dataExplanation.value = result
      return result
    }
    catch (err) {
      toast.error('Data explanation failed', { description: err as string })
      return null
    }
  }

  function reset() {
    naturalQuery.value = ''
    generatedQuery.value = ''
    queryResult.value = null
    queryExplanation.value = null
    chartConfig.value = null
    dataExplanation.value = ''
    error.value = null
    tablesUsed.value = []
    translationExplanation.value = ''
    confidence.value = 0
  }

  return {
    isTranslating,
    isExecuting,
    isGeneratingChart,
    isExplaining,
    naturalQuery,
    generatedQuery,
    queryLanguage,
    tablesUsed,
    translationExplanation,
    confidence,
    queryResult,
    queryExplanation,
    chartConfig,
    dataExplanation,
    error,
    translateQuery,
    executeQuery,
    explainQuery,
    generateChart,
    explainData,
    reset,
  }
}
