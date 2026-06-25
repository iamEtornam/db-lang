<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Label } from '~/components/ui/label'
import { Textarea } from '~/components/ui/textarea'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '~/components/ui/select'
import ChartPreview from '~/components/charts/ChartPreview.vue'
import { buildChartPoints, inferNumericColumns } from '~/lib/chart'
import { useChartsStore } from '~/stores/charts'
import { useConnectionsStore } from '~/stores/connections'
import { useSnippetsStore } from '~/stores/snippets'
import type {
  AggregateFn, Chart, ChartDefinition, ChartType,
} from '~/types/database'

useHead({ title: 'Charts' })

const chartsStore = useChartsStore()
const connectionsStore = useConnectionsStore()
const snippetsStore = useSnippetsStore()
const { charts } = storeToRefs(chartsStore)
const { connections, activeConnection } = storeToRefs(connectionsStore)
const { snippets } = storeToRefs(snippetsStore)

const chartTypes: { value: ChartType; label: string }[] = [
  { value: 'bar', label: 'Bar' },
  { value: 'line', label: 'Line' },
  { value: 'area', label: 'Area' },
  { value: 'pie', label: 'Pie' },
  { value: 'scatter', label: 'Scatter' },
  { value: 'kpi', label: 'KPI / Single value' },
  { value: 'table', label: 'Table of aggregates' },
]

const aggregates: { value: AggregateFn; label: string }[] = [
  { value: 'none', label: 'None (raw rows)' },
  { value: 'sum', label: 'Sum' },
  { value: 'avg', label: 'Average' },
  { value: 'count', label: 'Count' },
  { value: 'min', label: 'Min' },
  { value: 'max', label: 'Max' },
]

// --- Editor state ---
const editingId = ref<string | null>(null)
const name = ref('')
const description = ref('')
const connectionId = ref<string | null>(activeConnection.value?.id ?? null)
const query = ref('')
const def = reactive<ChartDefinition>({
  chartType: 'bar',
  categoryField: '',
  valueField: '',
  aggregate: 'sum',
  topN: 0,
})

// --- Live result state ---
const rows = ref<Record<string, unknown>[]>([])
const columns = ref<string[]>([])
const isRunning = ref(false)
const runError = ref<string | null>(null)

const selectedEngine = computed(
  () => connections.value.find(c => c.id === connectionId.value)?.db_type ?? '',
)

const numericColumns = computed(() => inferNumericColumns(rows.value, columns.value))

const points = computed(() => buildChartPoints(rows.value, def))

const categoryLabel = computed(() => def.categoryField || 'Category')
const valueLabel = computed(() =>
  def.aggregate === 'count' ? 'Count' : (def.valueField || 'Value'),
)

onMounted(() => {
  chartsStore.loadCharts()
  if (connections.value.length === 0) connectionsStore.loadConnections()
  if (snippets.value.length === 0) snippetsStore.loadSnippets()
})

// Re-run preview when the connection switches (live-preview requirement).
watch(connectionId, () => {
  if (rows.value.length || query.value.trim()) runQuery()
})

function applySnippet(snippetId: string) {
  const s = snippets.value.find(sn => sn.id === snippetId)
  if (s) query.value = s.sql_query
}

/** Execute the one-off query against the chosen connection and refresh columns. */
async function runQuery() {
  if (!connectionId.value) {
    runError.value = 'Pick a connection first.'
    return
  }
  if (!query.value.trim()) {
    runError.value = 'Enter or pick a query.'
    return
  }
  isRunning.value = true
  runError.value = null
  try {
    const data = await invoke<string>('query_db', {
      connectionId: connectionId.value,
      query: query.value,
    })
    const parsed = JSON.parse(data) as Record<string, unknown>[]
    rows.value = parsed
    columns.value = parsed.length && parsed[0] ? Object.keys(parsed[0]) : []
    autoPickFields()
  }
  catch (err) {
    runError.value = err as string
    toast.error('Query failed', { description: err as string })
  }
  finally {
    isRunning.value = false
  }
}

/** Default the axis mapping to the first text column / first numeric column. */
function autoPickFields() {
  if (!columns.value.length) return
  if (!def.categoryField || !columns.value.includes(def.categoryField)) {
    def.categoryField = columns.value.find(c => !numericColumns.value.includes(c))
      ?? columns.value[0]!
  }
  if (!def.valueField || !columns.value.includes(def.valueField)) {
    def.valueField = numericColumns.value[0] ?? columns.value[0]!
  }
}

function resetEditor() {
  editingId.value = null
  name.value = ''
  description.value = ''
  connectionId.value = activeConnection.value?.id ?? null
  query.value = ''
  Object.assign(def, {
    chartType: 'bar', categoryField: '', valueField: '', aggregate: 'sum', topN: 0,
  })
  rows.value = []
  columns.value = []
  runError.value = null
}

async function loadChart(chart: Chart) {
  editingId.value = chart.id
  name.value = chart.name
  description.value = chart.description ?? ''
  connectionId.value = chart.connection_id
  query.value = chart.query
  try {
    const parsed = JSON.parse(chart.config_json) as Partial<ChartDefinition>
    Object.assign(def, {
      chartType: (parsed.chartType ?? chart.chart_type ?? 'bar') as ChartType,
      categoryField: parsed.categoryField ?? '',
      valueField: parsed.valueField ?? '',
      aggregate: parsed.aggregate ?? 'sum',
      topN: parsed.topN ?? 0,
    })
  }
  catch {
    // Corrupt config — fall back to the stored chart_type.
    Object.assign(def, { chartType: (chart.chart_type as ChartType) ?? 'bar' })
  }
  // Re-render with fresh data on demand.
  if (chart.connection_id) await runQuery()
}

async function save() {
  if (!name.value.trim()) {
    toast.error('Name your chart first')
    return
  }
  // Live preview re-runs on save (per acceptance criteria).
  if (connectionId.value && query.value.trim()) await runQuery()

  const saved = await chartsStore.saveChart({
    id: editingId.value ?? undefined,
    name: name.value,
    description: description.value || null,
    connection_id: connectionId.value,
    engine: selectedEngine.value,
    query: query.value,
    chart_type: def.chartType,
    config_json: JSON.stringify(def),
  })
  if (saved) {
    editingId.value = saved.id
    toast.success('Chart saved')
  }
  else {
    toast.error('Save failed', { description: chartsStore.error ?? undefined })
  }
}

async function removeChart(e: Event, chart: Chart) {
  e.stopPropagation()
  const ok = await chartsStore.deleteChart(chart.id)
  if (ok) {
    if (editingId.value === chart.id) resetEditor()
    toast.success('Chart deleted')
  }
}

/** Re-render a saved chart with fresh data via the backend run_chart command. */
async function refreshSaved(chart: Chart) {
  if (editingId.value !== chart.id) await loadChart(chart)
  if (!chart.connection_id) {
    toast.error('This chart has no connection to re-run')
    return
  }
  isRunning.value = true
  try {
    const fresh = await chartsStore.runChart(chart.id)
    rows.value = fresh
    columns.value = fresh.length && fresh[0] ? Object.keys(fresh[0]) : []
    toast.success('Re-rendered with fresh data')
  }
  catch (err) {
    toast.error('Re-run failed', { description: err as string })
  }
  finally {
    isRunning.value = false
  }
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-lg font-semibold">Charts</h1>
        <p class="text-sm text-muted-foreground">Build and save custom charts from your query results</p>
      </div>
      <Button variant="outline" size="sm" @click="resetEditor">
        <Icon name="lucide:plus" class="size-4" />
        New chart
      </Button>
    </div>

    <div class="grid grid-cols-1 gap-4 lg:grid-cols-[220px_minmax(0,1fr)]">
      <!-- Saved charts list -->
      <aside class="flex flex-col gap-1.5">
        <Label class="text-xs text-muted-foreground">Saved charts</Label>
        <p v-if="charts.length === 0" class="text-xs text-muted-foreground py-2">
          No saved charts yet.
        </p>
        <button
          v-for="chart in charts"
          :key="chart.id"
          class="group flex items-center gap-2 rounded-md border border-border px-2.5 py-2 text-left text-sm transition-colors hover:bg-accent"
          :class="editingId === chart.id ? 'bg-accent border-primary/40' : ''"
          @click="loadChart(chart)"
        >
          <Icon name="lucide:bar-chart-3" class="size-4 shrink-0 text-muted-foreground" />
          <span class="min-w-0 flex-1 truncate">{{ chart.name }}</span>
          <Icon
            name="lucide:refresh-cw"
            class="size-3.5 shrink-0 text-muted-foreground opacity-0 transition-opacity hover:text-foreground group-hover:opacity-100"
            title="Re-render with fresh data"
            @click.stop="refreshSaved(chart)"
          />
          <Icon
            name="lucide:trash-2"
            class="size-3.5 shrink-0 text-muted-foreground opacity-0 transition-opacity hover:text-destructive group-hover:opacity-100"
            title="Delete chart"
            @click.stop="removeChart($event, chart)"
          />
        </button>
      </aside>

      <!-- Editor + preview -->
      <div class="grid grid-cols-1 gap-4 xl:grid-cols-2">
        <!-- Controls -->
        <div class="flex flex-col gap-4 rounded-md border border-border p-4">
          <div class="grid grid-cols-2 gap-3">
            <div class="space-y-1.5">
              <Label>Name</Label>
              <Input v-model="name" placeholder="Monthly revenue" />
            </div>
            <div class="space-y-1.5">
              <Label>Connection</Label>
              <Select v-model="connectionId">
                <SelectTrigger>
                  <SelectValue placeholder="Pick connection" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="c in connections" :key="c.id" :value="c.id">
                    {{ c.name }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <div class="space-y-1.5">
            <Label>Description</Label>
            <Input v-model="description" placeholder="Optional" />
          </div>

          <div class="space-y-1.5">
            <div class="flex items-center justify-between">
              <Label>Data source query</Label>
              <Select v-if="snippets.length" @update:model-value="(v) => applySnippet(String(v))">
                <SelectTrigger class="h-7 w-44 text-xs">
                  <SelectValue placeholder="From snippet…" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="s in snippets" :key="s.id" :value="s.id">
                    {{ s.name }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
            <Textarea v-model="query" rows="4" class="font-mono text-xs" placeholder="SELECT category, SUM(amount) FROM sales GROUP BY category" />
          </div>

          <div class="flex items-center gap-2">
            <Button size="sm" :disabled="isRunning" @click="runQuery">
              <Icon :name="isRunning ? 'lucide:loader-2' : 'lucide:play'" :class="isRunning ? 'size-4 animate-spin' : 'size-4'" />
              Run query
            </Button>
            <span v-if="columns.length" class="text-xs text-muted-foreground">
              {{ rows.length }} rows · {{ columns.length }} columns
            </span>
            <span v-if="runError" class="text-xs text-destructive">{{ runError }}</span>
          </div>

          <div class="grid grid-cols-2 gap-3 border-t border-border pt-3">
            <div class="space-y-1.5">
              <Label>Chart type</Label>
              <Select v-model="def.chartType">
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="t in chartTypes" :key="t.value" :value="t.value">
                    {{ t.label }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div class="space-y-1.5">
              <Label>Aggregate</Label>
              <Select v-model="def.aggregate">
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="a in aggregates" :key="a.value" :value="a.value">
                    {{ a.label }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div class="space-y-1.5">
              <Label>Category / X (group by)</Label>
              <Select v-model="def.categoryField" :disabled="!columns.length">
                <SelectTrigger>
                  <SelectValue placeholder="Column" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="c in columns" :key="c" :value="c">{{ c }}</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div class="space-y-1.5">
              <Label>Value / Y</Label>
              <Select v-model="def.valueField" :disabled="!columns.length || def.aggregate === 'count'">
                <SelectTrigger>
                  <SelectValue placeholder="Column" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="c in columns" :key="c" :value="c">
                    {{ c }}{{ numericColumns.includes(c) ? ' (#)' : '' }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div class="space-y-1.5">
              <Label>Top N (0 = all)</Label>
              <Input v-model.number="def.topN" type="number" min="0" />
            </div>
          </div>

          <Button class="mt-1" :disabled="!name.trim()" @click="save">
            <Icon name="lucide:save" class="size-4" />
            {{ editingId ? 'Update chart' : 'Save chart' }}
          </Button>
        </div>

        <!-- Live preview -->
        <div class="flex min-h-[320px] flex-col gap-2 rounded-md border border-border p-4">
          <div class="flex items-center justify-between">
            <h3 class="text-sm font-medium">{{ name || 'Preview' }}</h3>
            <span class="text-xs text-muted-foreground">{{ points.length }} points</span>
          </div>
          <div class="min-h-0 flex-1">
            <ChartPreview
              :points="points"
              :chart-type="def.chartType"
              :category-label="categoryLabel"
              :value-label="valueLabel"
            />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
