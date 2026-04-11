<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '~/components/ui/tabs'
import { Badge } from '~/components/ui/badge'
import { Button } from '~/components/ui/button'
import { Textarea } from '~/components/ui/textarea'
import { Separator } from '~/components/ui/separator'
import { useConnectionsStore } from '~/stores/connections'
import { useHistoryStore } from '~/stores/history'
import type { PaginatedResult } from '~/types/database'
import type { QueryResult } from '~/types/query'

useHead({ title: 'Query' })

const connectionsStore = useConnectionsStore()
const historyStore = useHistoryStore()
const {
  activeConnection,
  activeConnectionString,
  schemaContext,
  isLoadingSchema,
  tables,
} = storeToRefs(connectionsStore)

const tableNames = computed(() => tables.value.map(t => t.name))

const naturalQuery = ref('')
const generatedQuery = ref('')
const isTranslating = ref(false)
const isExecuting = ref(false)
const queryResult = ref<QueryResult | null>(null)
const queryError = ref<string | null>(null)
const activeResultTab = ref<'table' | 'chart' | 'insights'>('table')
const showSettingsPrompt = ref(false)

// Chart state
interface ChartConfig {
  chart_type: 'bar' | 'line' | 'area' | 'pie' | 'scatter' | 'table'
  title: string
  x_axis: { field: string; label: string }
  y_axis: { field: string; label: string }
  series: Array<{ field: string; label: string }>
  explanation: string
}
const chartConfig = ref<ChartConfig | null>(null)
const isGeneratingChart = ref(false)

// Insights state
const dataInsight = ref('')
const isGeneratingInsight = ref(false)

// Auto-generate chart/insight when tab is clicked if not yet generated
watch(activeResultTab, async (tab) => {
  if (!queryResult.value) return
  if (tab === 'chart' && !chartConfig.value && !isGeneratingChart.value) {
    await generateChart()
  }
  if (tab === 'insights' && !dataInsight.value && !isGeneratingInsight.value) {
    await generateInsight()
  }
})

// Reset chart/insight when a new query runs
watch(queryResult, () => {
  chartConfig.value = null
  dataInsight.value = ''
})

async function generateChart() {
  if (!queryResult.value || !activeConnection.value) return
  isGeneratingChart.value = true
  try {
    const config = await invoke<ChartConfig>('generate_chart_config', {
      data: JSON.stringify(queryResult.value.rows.slice(0, 50)),
      columns: queryResult.value.columns,
      query: naturalQuery.value,
      engine: activeConnection.value.db_type,
    })
    chartConfig.value = config
  }
  catch (err) {
    toast.error('Could not generate chart', { description: err as string })
  }
  finally {
    isGeneratingChart.value = false
  }
}

async function generateInsight() {
  if (!queryResult.value) return
  isGeneratingInsight.value = true
  try {
    const text = await invoke<string>('explain_data', {
      data: JSON.stringify(queryResult.value.rows.slice(0, 100)),
      columns: queryResult.value.columns,
      query: naturalQuery.value,
    })
    dataInsight.value = text
  }
  catch (err) {
    toast.error('Could not generate insight', { description: err as string })
  }
  finally {
    isGeneratingInsight.value = false
  }
}

// Chart rendering helpers
const chartData = computed(() => {
  if (!chartConfig.value || !queryResult.value) return []
  const { x_axis, y_axis } = chartConfig.value
  return queryResult.value.rows.map(row => ({
    label: String(row[x_axis.field] ?? ''),
    value: Number(row[y_axis.field] ?? 0),
  }))
})

const chartMaxValue = computed(() =>
  chartData.value.length > 0 ? Math.max(...chartData.value.map(d => d.value), 1) : 1,
)

onMounted(async () => {
  await connectionsStore.loadConnections()
})

async function onAskQuery() {
  if (!activeConnection.value || !naturalQuery.value.trim()) return

  const isConfigured = await invoke<boolean>('check_llm_configured')
  if (!isConfigured) {
    showSettingsPrompt.value = true
    toast.error('AI model not configured', {
      description: 'Please set up your AI provider and API key in Settings first.',
    })
    return
  }

  // Ensure schema is loaded
  if (!schemaContext.value) {
    toast.info('Loading database schema...')
    const loaded = await connectionsStore.loadSchema()
    if (!loaded || !schemaContext.value) {
      toast.error('Could not load database schema. Check your connection.')
      return
    }
  }

  isTranslating.value = true
  queryError.value = null
  generatedQuery.value = ''
  queryResult.value = null

  try {
    const sql = await invoke<string>('translate_with_schema', {
      query: naturalQuery.value,
      schemaContext: schemaContext.value,
      tableNames: tableNames.value,
      engine: activeConnection.value.db_type,
    })
    generatedQuery.value = sql

    await executeQuery()
  }
  catch (err) {
    queryError.value = err as string
    toast.error('Failed to generate query', { description: err as string })
  }
  finally {
    isTranslating.value = false
  }
}

async function executeQuery(page = 1) {
  if (!activeConnection.value || !generatedQuery.value.trim()) return
  isExecuting.value = true
  queryError.value = null

  const startTime = Date.now()

  try {
    const isSelect = generatedQuery.value.trim().toUpperCase().startsWith('SELECT')

    if (isSelect) {
      const result = await invoke<PaginatedResult>('query_db_paginated', {
        engine: activeConnection.value.db_type,
        connStr: activeConnectionString.value,
        query: generatedQuery.value,
        page,
        pageSize: 50,
      })

      const rows = JSON.parse(result.data) as Record<string, unknown>[]
      const columns = rows.length > 0 ? Object.keys(rows[0]) : []

      queryResult.value = {
        rows,
        columns,
        total_count: result.total_count,
        page: result.page,
        page_size: result.page_size,
        has_more: result.has_more,
        execution_time_ms: Date.now() - startTime,
      }
    }
    else {
      const data = await invoke<string>('query_db', {
        engine: activeConnection.value.db_type,
        connStr: activeConnectionString.value,
        query: generatedQuery.value,
      })

      const rows = JSON.parse(data) as Record<string, unknown>[]
      const columns = rows.length > 0 ? Object.keys(rows[0]) : []

      queryResult.value = {
        rows,
        columns,
        total_count: rows.length,
        page: 1,
        page_size: rows.length,
        has_more: false,
        execution_time_ms: Date.now() - startTime,
      }
    }

    activeResultTab.value = 'table'

    await historyStore.addToHistory({
      connection_id: activeConnection.value.id,
      natural_query: naturalQuery.value,
      sql_query: generatedQuery.value,
      result_count: queryResult.value.total_count ?? queryResult.value.rows.length,
      execution_time_ms: queryResult.value.execution_time_ms,
      status: 'success',
    })
  }
  catch (err) {
    queryError.value = err as string
    toast.error('Query failed', { description: err as string })

    if (activeConnection.value) {
      await historyStore.addToHistory({
        connection_id: activeConnection.value.id,
        natural_query: naturalQuery.value,
        sql_query: generatedQuery.value,
        status: 'error',
        error_message: err as string,
      })
    }
  }
  finally {
    isExecuting.value = false
  }
}

function onKeyDown(e: KeyboardEvent) {
  if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
    e.preventDefault()
    if (generatedQuery.value) {
      executeQuery()
    }
    else {
      onAskQuery()
    }
  }
}

function onEnterKey(e: KeyboardEvent) {
  if (!e.shiftKey) {
    e.preventDefault()
    onAskQuery()
  }
}

function copyQuery() {
  navigator.clipboard.writeText(generatedQuery.value)
  toast.success('Copied to clipboard')
}

function formatCellValue(val: unknown): string {
  if (val === null || val === undefined) return ''
  if (typeof val === 'object') return JSON.stringify(val)
  return String(val)
}

const router = useRouter()
function goToSettings() {
  router.push('/settings')
}
</script>

<template>
  <div class="flex flex-col h-full overflow-hidden">
    <!-- Query Editor (fixed at top, never grows) -->
    <div class="shrink-0 space-y-3 pb-3 border-b border-border">
      <!-- Natural language input -->
      <div class="space-y-1.5">
        <div class="flex items-center justify-between">
          <label class="text-sm font-medium">Ask in plain English</label>
          <div class="flex items-center gap-2">
            <span v-if="isLoadingSchema" class="text-xs text-muted-foreground flex items-center gap-1.5">
              <Icon name="lucide:loader-2" class="size-3 animate-spin" />
              Loading schema...
            </span>
            <span v-else-if="tables.length > 0 && activeConnection" class="text-xs text-muted-foreground flex items-center gap-1.5">
              <Icon name="lucide:check-circle" class="size-3 text-green-500" />
              {{ tables.length }} tables
            </span>
            <span v-else-if="activeConnection && !schemaContext && !isLoadingSchema" class="text-xs text-yellow-500 flex items-center gap-1.5 cursor-pointer" @click="connectionsStore.loadSchema()">
              <Icon name="lucide:alert-triangle" class="size-3" />
              Schema not loaded — click to retry
            </span>
            <span v-if="activeConnection" class="text-xs text-muted-foreground flex items-center gap-1.5">
              <span class="size-1.5 rounded-full bg-green-500 inline-block" />
              {{ activeConnection.name }}
            </span>
            <span v-else class="text-xs text-destructive">No connection</span>
          </div>
        </div>

        <div class="relative">
          <Textarea
            v-model="naturalQuery"
            placeholder="e.g. List all students between age 20 and 30 who have paid more than 50% of their fees"
            class="min-h-[80px] resize-none pr-28"
            :disabled="!activeConnection"
            @keydown="onKeyDown"
            @keydown.enter="onEnterKey"
          />
          <div class="absolute bottom-2.5 right-2.5 flex gap-1.5">
            <Button
              size="sm"
              class="h-7 px-3 text-xs gap-1.5"
              :disabled="!naturalQuery.trim() || isTranslating || isExecuting || !activeConnection"
              @click="onAskQuery"
            >
              <Icon v-if="isTranslating || isExecuting" name="lucide:loader-2" class="size-3.5 animate-spin" />
              <Icon v-else name="lucide:sparkles" class="size-3.5" />
              {{ isTranslating ? 'Generating...' : isExecuting ? 'Running...' : 'Ask' }}
            </Button>
          </div>
        </div>
      </div>

      <!-- AI not configured prompt -->
      <div v-if="showSettingsPrompt" class="rounded-md bg-yellow-500/10 border border-yellow-500/20 p-3 text-sm flex items-center justify-between gap-3">
        <div class="flex items-start gap-2 text-yellow-600 dark:text-yellow-400">
          <Icon name="lucide:alert-triangle" class="size-4 shrink-0 mt-0.5" />
          <span>AI model not configured. Set up your API key to use natural language queries.</span>
        </div>
        <Button size="sm" variant="outline" class="shrink-0 h-7 text-xs" @click="goToSettings">
          <Icon name="lucide:settings" class="size-3.5" />
          Configure
        </Button>
      </div>

      <!-- Generated SQL -->
      <div v-if="generatedQuery" class="space-y-2">
        <div class="flex items-center justify-between">
          <label class="text-sm font-medium">Generated SQL</label>
          <div class="flex items-center gap-1">
            <Button size="sm" variant="ghost" class="h-7 px-2 text-xs" @click="copyQuery">
              <Icon name="lucide:copy" class="size-3.5" />
            </Button>
            <Button size="sm" variant="ghost" class="h-7 px-2 text-xs text-destructive" @click="generatedQuery = ''; queryResult = null; queryError = null">
              <Icon name="lucide:x" class="size-3.5" />
            </Button>
          </div>
        </div>

        <Textarea
          v-model="generatedQuery"
          class="min-h-[60px] resize-none font-mono text-sm"
        />

        <div class="flex items-center gap-2">
          <Button
            :disabled="!generatedQuery.trim() || isExecuting"
            class="gap-1.5"
            @click="executeQuery()"
          >
            <Icon v-if="isExecuting" name="lucide:loader-2" class="size-4 animate-spin" />
            <Icon v-else name="lucide:play" class="size-4" />
            Re-run
            <kbd class="hidden sm:inline-flex ml-1 pointer-events-none h-5 select-none items-center gap-1 rounded border border-primary-foreground/30 bg-primary-foreground/20 px-1.5 font-mono text-[10px] text-primary-foreground/80">
              ⌘↵
            </kbd>
          </Button>
        </div>
      </div>

      <!-- Error -->
      <div v-if="queryError" class="rounded-md bg-destructive/10 border border-destructive/20 p-3 text-sm text-destructive flex items-start gap-2">
        <Icon name="lucide:alert-circle" class="size-4 shrink-0 mt-0.5" />
        <span>{{ queryError }}</span>
      </div>
    </div>

    <!-- Lower area: fills remaining height -->
    <div class="flex-1 overflow-hidden flex flex-col min-h-0 pt-3">
    <!-- No connection -->
    <div v-if="!activeConnection" class="flex-1 flex flex-col items-center justify-center text-muted-foreground gap-3 border border-dashed border-border rounded-lg">
      <Icon name="lucide:database" class="size-10" />
      <div class="text-center">
        <p class="font-medium text-foreground">No connection selected</p>
        <p class="text-sm mt-1">Click a connection in the sidebar to connect</p>
      </div>
    </div>

    <!-- Results -->
    <div v-else-if="queryResult || isExecuting" class="flex-1 overflow-hidden flex flex-col min-h-0">
      <Tabs v-model="activeResultTab" class="flex-1 flex flex-col overflow-hidden">
        <TabsList class="shrink-0">
          <TabsTrigger value="table">
            <Icon name="lucide:table-2" class="size-3.5 mr-1.5" />
            Table
            <Badge v-if="queryResult" variant="secondary" class="ml-1.5 text-xs">
              {{ queryResult.total_count?.toLocaleString() ?? queryResult.rows.length }}
            </Badge>
          </TabsTrigger>
          <TabsTrigger value="chart">
            <Icon name="lucide:bar-chart-3" class="size-3.5 mr-1.5" />
            Chart
          </TabsTrigger>
          <TabsTrigger value="insights">
            <Icon name="lucide:sparkles" class="size-3.5 mr-1.5" />
            Insights
          </TabsTrigger>
        </TabsList>

        <TabsContent value="table" class="flex-1 overflow-hidden mt-2">
          <div v-if="isExecuting && !queryResult" class="flex flex-col gap-2 p-4">
            <div v-for="i in 5" :key="i" class="h-10 bg-muted/50 rounded animate-pulse" />
          </div>

          <div v-else-if="queryResult && queryResult.rows.length > 0" class="flex flex-col h-full overflow-hidden rounded-md border border-border">
            <div class="flex items-center gap-3 px-3 py-2 border-b border-border text-sm text-muted-foreground shrink-0">
              <span>{{ queryResult.total_count?.toLocaleString() ?? queryResult.rows.length }} rows</span>
              <Separator orientation="vertical" class="h-4" />
              <span>{{ queryResult.columns.length }} columns</span>
              <Separator orientation="vertical" class="h-4" />
              <Badge variant="outline" class="text-xs">{{ queryResult.execution_time_ms }}ms</Badge>
            </div>

            <div class="overflow-auto flex-1">
              <table class="w-full text-sm">
                <thead class="sticky top-0 bg-muted/90 backdrop-blur-sm">
                  <tr class="border-b border-border">
                    <th
                      v-for="col in queryResult.columns"
                      :key="col"
                      class="px-3 py-2 text-left text-xs font-medium text-muted-foreground whitespace-nowrap"
                    >
                      {{ col }}
                    </th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-border">
                  <tr
                    v-for="(row, ri) in queryResult.rows"
                    :key="ri"
                    class="hover:bg-muted/30 transition-colors"
                  >
                    <td
                      v-for="col in queryResult.columns"
                      :key="col"
                      class="px-3 py-1.5 whitespace-nowrap max-w-[300px] truncate"
                    >
                      <template v-if="row[col] === null">
                        <span class="text-muted-foreground italic text-xs">null</span>
                      </template>
                      <template v-else>
                        {{ formatCellValue(row[col]) }}
                      </template>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>

            <div v-if="queryResult.has_more || queryResult.page > 1" class="flex items-center justify-between border-t border-border px-3 py-2 text-sm text-muted-foreground shrink-0">
              <span>Page {{ queryResult.page }}</span>
              <div class="flex gap-1">
                <Button variant="ghost" size="sm" class="h-7" :disabled="queryResult.page <= 1" @click="executeQuery(queryResult.page - 1)">
                  <Icon name="lucide:chevron-left" class="size-4" />
                </Button>
                <Button variant="ghost" size="sm" class="h-7" :disabled="!queryResult.has_more" @click="executeQuery(queryResult.page + 1)">
                  <Icon name="lucide:chevron-right" class="size-4" />
                </Button>
              </div>
            </div>
          </div>

          <div v-else-if="queryResult && queryResult.rows.length === 0" class="flex flex-col items-center justify-center flex-1 py-12 text-muted-foreground">
            <Icon name="lucide:table" class="size-8 mb-3" />
            <p class="text-sm">Query returned no results</p>
          </div>
        </TabsContent>

        <!-- Chart tab -->
        <TabsContent value="chart" class="flex-1 overflow-auto mt-2">
          <!-- Generating -->
          <div v-if="isGeneratingChart" class="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground">
            <Icon name="lucide:loader-2" class="size-8 animate-spin text-primary" />
            <p class="text-sm">AI is analyzing your data...</p>
          </div>

          <!-- Chart ready -->
          <div v-else-if="chartConfig" class="flex flex-col gap-4 p-2 h-full">
            <div class="flex items-start justify-between gap-4">
              <div>
                <h3 class="font-medium text-sm">{{ chartConfig.title }}</h3>
                <p class="text-xs text-muted-foreground mt-0.5">{{ chartConfig.explanation }}</p>
              </div>
              <Button size="sm" variant="outline" class="shrink-0" @click="generateChart">
                <Icon name="lucide:refresh-cw" class="size-3.5" />
                Regenerate
              </Button>
            </div>

            <!-- Bar / Line / Area chart -->
            <div v-if="['bar','line','area','scatter'].includes(chartConfig.chart_type)" class="flex-1 rounded-lg border border-border p-4 overflow-hidden">
              <div class="flex flex-col h-full gap-2">
                <!-- Y-axis label -->
                <p class="text-xs text-muted-foreground">{{ chartConfig.y_axis.label }}</p>
                <!-- Bars -->
                <div class="flex-1 flex items-end gap-1 min-h-0">
                  <div
                    v-for="(item, i) in chartData.slice(0, 30)"
                    :key="i"
                    class="flex-1 flex flex-col items-center gap-1 min-w-0 group relative"
                    style="min-width: 12px"
                  >
                    <!-- Value tooltip on hover -->
                    <div class="absolute bottom-full mb-1 bg-popover border border-border text-popover-foreground text-[10px] px-1.5 py-0.5 rounded shadow-md whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-10">
                      {{ item.value.toLocaleString() }}
                    </div>
                    <div
                      class="w-full rounded-t-sm bg-primary/80 hover:bg-primary transition-colors cursor-default"
                      :style="{ height: `${Math.max(4, (item.value / chartMaxValue) * 100)}%` }"
                    />
                  </div>
                </div>
                <!-- X-axis labels -->
                <div class="flex items-start gap-1 shrink-0">
                  <div
                    v-for="(item, i) in chartData.slice(0, 30)"
                    :key="i"
                    class="flex-1 min-w-0"
                    style="min-width: 12px"
                  >
                    <span class="text-[9px] text-muted-foreground truncate block text-center" :title="item.label">
                      {{ item.label.length > 8 ? item.label.slice(0, 8) + '…' : item.label }}
                    </span>
                  </div>
                </div>
                <!-- X-axis label -->
                <p class="text-xs text-muted-foreground text-center shrink-0">{{ chartConfig.x_axis.label }}</p>
              </div>
            </div>

            <!-- Pie / Donut chart -->
            <div v-else-if="chartConfig.chart_type === 'pie'" class="flex-1 rounded-lg border border-border p-4 flex items-center gap-8">
              <div class="shrink-0 flex flex-col gap-2">
                <div
                  v-for="(item, i) in chartData.slice(0, 10)"
                  :key="i"
                  class="flex items-center gap-2 text-sm"
                >
                  <div
                    class="size-3 rounded-sm shrink-0"
                    :style="{ backgroundColor: `hsl(${(i * 37) % 360}, 65%, 55%)` }"
                  />
                  <span class="text-muted-foreground truncate max-w-[140px]">{{ item.label }}</span>
                  <span class="font-mono text-xs ml-auto pl-4">{{ item.value.toLocaleString() }}</span>
                </div>
              </div>
            </div>

            <!-- No visual chart possible -->
            <div v-else class="flex-1 rounded-lg border border-dashed border-border flex flex-col items-center justify-center gap-3 text-muted-foreground p-8">
              <Icon name="lucide:info" class="size-8" />
              <div class="text-center max-w-sm">
                <p class="text-sm font-medium text-foreground">Not enough numeric data to chart</p>
                <p class="text-xs mt-1">{{ chartConfig.explanation }}</p>
                <p class="text-xs mt-2">Try a query that returns numeric columns alongside a category — e.g. totals by group, counts by date, etc.</p>
              </div>
            </div>
          </div>

          <!-- No chart yet (shouldn't normally show since we auto-generate) -->
          <div v-else class="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground">
            <Icon name="lucide:bar-chart-3" class="size-8" />
            <Button @click="generateChart">
              <Icon name="lucide:sparkles" class="size-4" />
              Generate Chart
            </Button>
          </div>
        </TabsContent>

        <!-- Insights tab -->
        <TabsContent value="insights" class="flex-1 overflow-auto mt-2">
          <!-- Generating -->
          <div v-if="isGeneratingInsight" class="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground">
            <Icon name="lucide:loader-2" class="size-8 animate-spin text-primary" />
            <p class="text-sm">AI is reading your data...</p>
          </div>

          <!-- Insight ready -->
          <div v-else-if="dataInsight" class="flex flex-col gap-3 p-2">
            <div class="flex items-center justify-between">
              <h3 class="text-sm font-medium flex items-center gap-2">
                <Icon name="lucide:sparkles" class="size-4 text-primary" />
                AI Analysis
              </h3>
              <Button size="sm" variant="ghost" @click="generateInsight">
                <Icon name="lucide:refresh-cw" class="size-3.5" />
                Refresh
              </Button>
            </div>
            <div class="rounded-lg border border-border bg-muted/20 p-4">
              <p class="text-sm text-foreground leading-relaxed whitespace-pre-wrap">{{ dataInsight }}</p>
            </div>
          </div>

          <!-- No insight yet -->
          <div v-else class="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground">
            <Icon name="lucide:sparkles" class="size-8" />
            <Button @click="generateInsight">
              <Icon name="lucide:sparkles" class="size-4" />
              Explain Data
            </Button>
          </div>
        </TabsContent>
      </Tabs>
    </div>

    <!-- Ready state -->
    <div v-else class="flex-1 flex flex-col items-center justify-center text-muted-foreground gap-2">
      <Icon name="lucide:terminal-square" class="size-10 mb-1" />
      <p class="font-medium text-foreground">Ready to query</p>
      <p class="text-sm text-center max-w-md">
        Type a question in plain English and press Enter. The AI will generate the correct SQL using your database schema and run it automatically.
      </p>
    </div>
    </div><!-- end lower area -->
  </div>
</template>
