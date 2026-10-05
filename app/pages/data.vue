<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Label } from '~/components/ui/label'
import { Textarea } from '~/components/ui/textarea'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '~/components/ui/dialog'
import ResultsTable from '~/components/results/ResultsTable.vue'
import { useConnectionsStore } from '~/stores/connections'
import type { TableInfo, ColumnInfo } from '~/types/database'
import type { QueryResult } from '~/types/query'

useHead({ title: 'Data files' })
interface FileRequest { filename: string; content_base64: string; sheet: string | null }
interface Preview { columns: string[]; rows: (string | null)[][]; row_count: number; sheets: string[]; sheet: string | null; clipped_preview: boolean }
interface LocalResult { columns: string[]; rows: Record<string, unknown>[]; truncated: boolean; execution_time_ms: number }
const store = useConnectionsStore()
const sqlConnections = computed(() => store.connections.filter(connection => ['postgres', 'mysql', 'mariadb', 'sqlite'].includes(connection.db_type)))
const mode = ref('import')
const file = ref<FileRequest | null>(null)
const preview = ref<Preview | null>(null)
const connectionId = ref('')
const tables = ref<TableInfo[]>([])
const tableIndex = ref('')
const targetColumns = ref<ColumnInfo[]>([])
const mapping = ref<string[]>([])
const emptyAsNull = ref(false)
const fileLoading = ref(false)
const tablesLoading = ref(false)
const columnsLoading = ref(false)
const importing = ref(false)
const querying = ref(false)
const imported = ref(false)
const error = ref('')
const query = ref('SELECT * FROM data LIMIT 100')
const result = ref<QueryResult | null>(null)
const truncated = ref(false)
const confirmOpen = ref(false)
const confirmation = ref('')
let fileRevision = 0
let tablesRevision = 0
let columnsRevision = 0
let queryRevision = 0
const busy = computed(() => fileLoading.value || importing.value || querying.value)
const selectedTable = computed(() => tables.value[Number(tableIndex.value)] && tableIndex.value !== '' ? tables.value[Number(tableIndex.value)]! : null)
const selectedConnection = computed(() => sqlConnections.value.find(connection => connection.id === connectionId.value))
const mappedColumns = computed(() => mapping.value.map((target, source) => ({ source, target })).filter(entry => entry.target))
const repeatedTargets = computed(() => new Set(mappedColumns.value.map(entry => entry.target)).size !== mappedColumns.value.length)
const canImport = computed(() => !busy.value && !columnsLoading.value && !tablesLoading.value && !imported.value && !!selectedTable.value && !!preview.value?.row_count && mappedColumns.value.length > 0 && !repeatedTargets.value && !selectedConnection.value?.options?.read_only)

function resetFile() {
  fileRevision++; queryRevision++
  file.value = null; preview.value = null; mapping.value = []; result.value = null
  fileLoading.value = false; imported.value = false; truncated.value = false; error.value = ''
}
watch(mode, resetFile)
watch([mapping, emptyAsNull], () => { imported.value = false }, { deep: true })
function setDefaultMapping() {
  mapping.value = preview.value?.columns.map(name => targetColumns.value.some(column => column.name === name) ? name : '') ?? []
}
async function previewFile() {
  if (!file.value) return
  const revision = ++fileRevision
  fileLoading.value = true; error.value = ''; preview.value = null; imported.value = false
  try {
    const loaded = await invoke<Preview>('preview_import_file', { file: { ...file.value } })
    if (revision !== fileRevision) return
    preview.value = loaded
    file.value!.sheet = loaded.sheet
    setDefaultMapping()
  }
  catch (cause) { if (revision === fileRevision) error.value = String(cause) }
  finally { if (revision === fileRevision) fileLoading.value = false }
}
async function chooseFile(event: Event) {
  const input = event.target as HTMLInputElement
  const chosen = input.files?.[0]
  input.value = ''
  resetFile()
  if (!chosen) return
  if (chosen.size === 0 || chosen.size > 8 * 1024 * 1024) { error.value = 'Choose a nonempty file of at most 8 MiB.'; return }
  const revision = ++fileRevision
  fileLoading.value = true
  try {
    const bytes = new Uint8Array(await chosen.arrayBuffer())
    if (revision !== fileRevision) return
    let binary = ''
    for (let offset = 0; offset < bytes.length; offset += 8192) binary += String.fromCharCode(...bytes.subarray(offset, offset + 8192))
    file.value = { filename: chosen.name, content_base64: btoa(binary), sheet: null }
    if (mode.value === 'import') await previewFile()
  }
  catch (cause) { if (revision === fileRevision) error.value = String(cause) }
  finally { if (revision === fileRevision || mode.value === 'local') fileLoading.value = false }
}
watch(connectionId, async () => {
  const revision = ++tablesRevision
  columnsRevision++
  tables.value = []; tableIndex.value = ''; targetColumns.value = []; mapping.value = []; imported.value = false
  columnsLoading.value = false; tablesLoading.value = false; error.value = ''
  if (!connectionId.value) return
  tablesLoading.value = true
  try {
    const loaded = await invoke<TableInfo[]>('get_tables', { connectionId: connectionId.value })
    if (revision === tablesRevision) tables.value = loaded.filter(table => !table.table_type.toLowerCase().includes('view'))
  }
  catch (cause) { if (revision === tablesRevision) error.value = String(cause) }
  finally { if (revision === tablesRevision) tablesLoading.value = false }
})
watch(tableIndex, async () => {
  const revision = ++columnsRevision
  targetColumns.value = []; mapping.value = []; imported.value = false; error.value = ''; columnsLoading.value = false
  const table = selectedTable.value
  if (!table) return
  columnsLoading.value = true
  try {
    const loaded = await invoke<ColumnInfo[]>('get_table_columns', { connectionId: connectionId.value, tableName: table.name, schemaName: table.schema })
    if (revision === columnsRevision) { targetColumns.value = loaded; setDefaultMapping() }
  }
  catch (cause) { if (revision === columnsRevision) error.value = String(cause) }
  finally { if (revision === columnsRevision) columnsLoading.value = false }
})
function reviewImport() { confirmation.value = ''; confirmOpen.value = true }
async function commitImport() {
  const table = selectedTable.value
  if (!canImport.value || !file.value || !table || confirmation.value !== table.name) return
  const target = [table.schema, table.name].filter(Boolean).join('.')
  const request = { file: { ...file.value }, connection_id: connectionId.value, table: table.name, schema: table.schema, mapping: mappedColumns.value.map(entry => ({ ...entry })), empty_as_null: emptyAsNull.value }
  importing.value = true; error.value = ''
  try {
    const count = await invoke<number>('import_table_file', { request })
    imported.value = true; confirmOpen.value = false
    toast.success(`Imported ${count.toLocaleString()} rows into ${target}`)
  }
  catch (cause) { error.value = String(cause); confirmOpen.value = false; toast.error('Import failed. Review the error before retrying.') }
  finally { importing.value = false }
}
async function runQuery() {
  if (!file.value || !query.value.trim() || busy.value) return
  const revision = ++queryRevision
  querying.value = true; error.value = ''; result.value = null; truncated.value = false
  try {
    const loaded = await invoke<LocalResult>('query_local_file', { file: { ...file.value }, query: query.value })
    if (revision !== queryRevision) return
    truncated.value = loaded.truncated
    result.value = { ...loaded, total_count: loaded.truncated ? null : loaded.rows.length, page: 1, page_size: 500, has_more: false }
  }
  catch (cause) { if (revision === queryRevision) error.value = String(cause) }
  finally { if (revision === queryRevision) querying.value = false }
}
onMounted(async () => {
  if (!store.connections.length) await store.loadConnections()
  connectionId.value = sqlConnections.value.find(connection => connection.id === store.activeConnection?.id)?.id ?? sqlConnections.value[0]?.id ?? ''
})
onBeforeUnmount(() => { fileRevision++; tablesRevision++; columnsRevision++; queryRevision++ })
</script>

<template>
  <main class="flex flex-1 flex-col gap-6 overflow-y-auto p-4 sm:p-6">
    <div><h1 class="text-xl font-semibold">Data files</h1><p class="mt-1 text-sm text-muted-foreground">Preview and map an import, or query a file locally.</p></div>
    <div class="grid max-w-3xl gap-4 sm:grid-cols-2">
      <div class="space-y-2"><Label for="file-mode">Operation</Label><select id="file-mode" v-model="mode" :disabled="busy" class="h-9 w-full rounded-md bg-background px-3 text-sm ring-1 ring-border"><option value="import">Import into an existing table</option><option value="local">Query a local file</option></select></div>
      <div class="space-y-2"><Label for="data-file">{{ mode === 'import' ? 'CSV or Excel workbook' : 'CSV, JSON, or Parquet file' }}</Label><input id="data-file" type="file" :accept="mode === 'import' ? '.csv,.xlsx' : '.csv,.json,.parquet'" :disabled="busy" class="block w-full text-sm file:mr-3 file:rounded-md file:border-0 file:bg-muted file:px-3 file:py-2 file:text-foreground" @change="chooseFile"><p v-if="file" class="break-words text-sm">{{ file.filename }}</p></div>
    </div>
    <p v-if="fileLoading || tablesLoading || columnsLoading" role="status" class="text-sm text-muted-foreground">{{ fileLoading ? 'Reading file…' : 'Loading table metadata…' }}</p>
    <p v-if="error" role="alert" class="max-w-3xl whitespace-pre-wrap break-words text-sm text-destructive">{{ error }}</p>
    <template v-if="mode === 'import'">
      <div class="grid max-w-3xl gap-4 sm:grid-cols-2">
        <div class="space-y-2"><Label for="import-connection">Saved connection</Label><select id="import-connection" v-model="connectionId" :disabled="busy" class="h-9 w-full rounded-md bg-background px-3 text-sm ring-1 ring-border"><option value="">Choose a SQL connection</option><option v-for="connection in sqlConnections" :key="connection.id" :value="connection.id">{{ connection.name }}{{ connection.options?.read_only ? ' (read-only)' : '' }}</option></select></div>
        <div class="space-y-2"><Label for="import-table">Destination table</Label><select id="import-table" v-model="tableIndex" :disabled="busy || tablesLoading" class="h-9 w-full rounded-md bg-background px-3 text-sm ring-1 ring-border"><option value="">Choose a table</option><option v-for="(table, index) in tables" :key="`${table.schema}:${table.name}`" :value="String(index)">{{ [table.schema, table.name].filter(Boolean).join('.') }}</option></select></div>
      </div>
      <p v-if="selectedConnection?.options?.read_only" class="text-sm text-muted-foreground">This saved connection is read-only. Choose a writable connection to import.</p>
      <div v-if="preview" class="flex max-w-3xl flex-col gap-4">
        <div v-if="preview.sheets.length > 1" class="space-y-2"><Label for="import-sheet">Worksheet</Label><select id="import-sheet" v-model="file!.sheet" :disabled="busy" class="h-9 w-full rounded-md bg-background px-3 text-sm ring-1 ring-border" @change="previewFile"><option v-for="sheet in preview.sheets" :key="sheet" :value="sheet">{{ sheet }}</option></select></div>
        <p class="text-sm">{{ preview.row_count.toLocaleString() }} rows. Preview shows the first {{ preview.rows.length }}. First row is the header. Excel formulas use cached values and are not evaluated.</p>
        <p v-if="preview.clipped_preview" class="text-sm text-muted-foreground">Long preview cells are shortened; the import uses their full values.</p>
        <div class="overflow-x-auto"><table class="w-full text-sm"><thead><tr><th v-for="(column, index) in preview.columns" :key="index" scope="col" class="px-3 py-2 text-left font-medium">{{ column }}</th></tr></thead><tbody><tr v-for="(row, index) in preview.rows" :key="index" class="odd:bg-muted/30"><td v-for="(cell, column) in row" :key="column" class="max-w-xs whitespace-pre-wrap break-words px-3 py-2 align-top">{{ cell === null ? 'NULL' : cell }}</td></tr></tbody></table></div>
        <div v-if="targetColumns.length" class="space-y-3"><h2 class="font-medium">Column mapping</h2><div v-for="(column, index) in preview.columns" :key="index" class="grid items-center gap-2 sm:grid-cols-2"><Label :for="`map-${index}`" class="break-words">{{ column }}</Label><select :id="`map-${index}`" v-model="mapping[index]" :disabled="busy" class="h-9 w-full rounded-md bg-background px-3 text-sm ring-1 ring-border"><option value="">Omit this column</option><option v-for="target in targetColumns" :key="target.name" :value="target.name">{{ target.name }} ({{ target.data_type }})</option></select></div></div>
        <p v-if="repeatedTargets" role="alert" class="text-sm text-destructive">Each destination column can be mapped only once.</p>
        <label class="flex items-center gap-2 text-sm"><input v-model="emptyAsNull" type="checkbox" :disabled="busy">Convert empty text cells to NULL</label>
        <p class="text-sm text-muted-foreground">Up to 10,000 rows and 128 columns. Unmapped columns use database defaults. All rows are inserted in one transaction. MySQL/MariaDB requires InnoDB.</p>
        <div><Button :disabled="!canImport" @click="reviewImport">{{ imported ? 'Import complete' : 'Review import' }}</Button></div>
      </div>
    </template>
    <template v-else>
      <div class="flex max-w-3xl flex-col gap-3"><Label for="local-query">SQL query</Label><Textarea id="local-query" v-model="query" :disabled="busy" rows="5" spellcheck="false" class="font-mono"/><p class="text-sm text-muted-foreground">The file is available as <code>data</code> in an in-memory DuckDB database. External file/network access is disabled after loading. Maximum 500 result rows and 30 seconds.</p><div><Button :disabled="!file || busy || !query.trim()" @click="runQuery">{{ querying ? 'Running query…' : 'Run local query' }}</Button></div></div>
      <p v-if="truncated" role="status" class="text-sm text-muted-foreground">Showing the first 500 rows. Refine the query to see a smaller result.</p>
      <div v-if="result" class="h-[min(60dvh,40rem)] min-h-64"><ResultsTable :result="result"/></div>
    </template>
    <Dialog v-model:open="confirmOpen"><DialogContent class="max-h-[90dvh] overflow-y-auto"><DialogHeader><DialogTitle>Confirm import</DialogTitle><DialogDescription>Insert {{ preview?.row_count.toLocaleString() }} rows into {{ selectedConnection?.name }} / {{ [selectedTable?.schema, selectedTable?.name].filter(Boolean).join('.') }} using {{ mappedColumns.length }} mapped columns. This adds rows to the existing table.</DialogDescription></DialogHeader><div class="space-y-2"><Label for="confirm-import">Type the table name to confirm: {{ selectedTable?.name }}</Label><Input id="confirm-import" v-model="confirmation" :disabled="importing" autocomplete="off"/></div><DialogFooter><Button variant="ghost" :disabled="importing" @click="confirmOpen = false">Cancel</Button><Button :disabled="!canImport || confirmation !== selectedTable?.name" @click="commitImport">{{ importing ? 'Importing…' : 'Import rows' }}</Button></DialogFooter></DialogContent></Dialog>
  </main>
</template>
