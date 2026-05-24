<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Badge } from '~/components/ui/badge'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Separator } from '~/components/ui/separator'
import { useConnectionsStore } from '~/stores/connections'
import type { ColumnInfo } from '~/types/database'
import {
  isSqlEngine,
  buildDelete,
  buildBulkDelete,
  buildDropTable,
  buildDropColumn,
  buildUpdate,
  coerceCellInput,
  type PkBinding,
  type ValueBinding,
} from '~/lib/sql'
import { isMongoEngine, buildIdFilter, getDocId as getMongoDocId, prettyJson as prettyJsonMongo } from '~/lib/mongo'
import {
  isFirestoreEngine,
  getDocId as getFirestoreDocId,
  stripMetadata as stripFirestoreMetadata,
  prettyJson as prettyJsonFirestore,
} from '~/lib/firestore'
import {
  isRtdbEngine,
  getRowKey as getRtdbRowKey,
  rowToChildJson,
  prettyJson as prettyJsonRtdb,
} from '~/lib/rtdb'
import {
  isRedisEngine,
  getRedisKey,
  getRedisType,
  defaultValueFor as defaultRedisValueFor,
  prettyJson as prettyJsonRedis,
  type RedisType,
  type RedisKeyView,
} from '~/lib/redis'
import RowEditDialog from '~/components/schema/RowEditDialog.vue'
import SqlConfirmDialog from '~/components/schema/SqlConfirmDialog.vue'
import MongoOpDialog from '~/components/schema/MongoOpDialog.vue'
import FirestoreOpDialog from '~/components/schema/FirestoreOpDialog.vue'
import RtdbOpDialog from '~/components/schema/RtdbOpDialog.vue'
import RedisOpDialog from '~/components/schema/RedisOpDialog.vue'
import BulkDeleteDialog from '~/components/schema/BulkDeleteDialog.vue'
import CreateTableDialog from '~/components/schema/CreateTableDialog.vue'
import AlterColumnDialog from '~/components/schema/AlterColumnDialog.vue'
import BatchConfirmDialog from '~/components/schema/BatchConfirmDialog.vue'
import ExportDialog from '~/components/shared/ExportDialog.vue'

useHead({ title: 'Schema' })

const connectionsStore = useConnectionsStore()
const { activeConnection, tables, tableColumns, isLoadingSchema } = storeToRefs(connectionsStore)

const selectedTableName = ref<string | null>(null)
const activeTab = ref<'columns' | 'preview'>('columns')
const previewData = ref<Record<string, unknown>[]>([])
const isLoadingPreview = ref(false)
const searchTerm = ref('')

onMounted(async () => {
  await connectionsStore.loadConnections()
  // Load schema if connected but not yet loaded
  if (activeConnection.value && tables.value.length === 0) {
    await connectionsStore.loadSchema()
  }
})

watch(activeConnection, async (conn) => {
  selectedTableName.value = null
  if (conn && tables.value.length === 0) {
    await connectionsStore.loadSchema()
  }
})

// Reset selection when tables reload
watch(tables, (t) => {
  if (t.length > 0 && !selectedTableName.value) {
    selectedTableName.value = t[0].name
  }
})

const filteredTables = computed(() => {
  if (!searchTerm.value) return tables.value
  const term = searchTerm.value.toLowerCase()
  return tables.value.filter(t =>
    t.name.toLowerCase().includes(term)
    || t.schema?.toLowerCase().includes(term),
  )
})

const selectedTable = computed(() => {
  if (!selectedTableName.value) return null
  // Top-level case: look up in the sidebar's table list.
  const direct = tables.value.find(t => t.name === selectedTableName.value)
  if (direct) return direct
  // Phase 8: nested Firestore paths (e.g. `users/abc/posts`) are valid
  // CRUD targets but aren't in the sidebar list. Synthesize a TableInfo
  // pointing at the nested path so the right-pane flows light up.
  if (selectedTableName.value.includes('/') && activeConnection.value?.db_type === 'firestore') {
    return {
      name: selectedTableName.value,
      schema: null,
      table_type: 'COLLECTION',
    }
  }
  return null
})

const selectedColumns = computed((): ColumnInfo[] =>
  (selectedTableName.value && tableColumns.value[selectedTableName.value]) || [],
)

const previewColumns = computed(() =>
  previewData.value.length > 0 ? Object.keys(previewData.value[0]) : [],
)

/** Click-to-reveal selection for the per-row Edit / Delete actions. The
 *  action buttons live in a sticky-right column so they're visible without
 *  horizontal scrolling; tapping a row reveals them in case the desktop
 *  hover affordance isn't available (touch / trackpad tap). */
const selectedRowIndex = ref<number | null>(null)

watch(selectedTableName, async (name) => {
  if (!name) return
  previewData.value = []
  selectedRowIndex.value = null
  bulkSelectedIds.value = new Set()
  // Phase 7: pending edits are scoped to a single table — switching tables
  // silently discards them. Users see the unsaved count in the toolbar.
  pendingEdits.value = new Map()
  editingCell.value = null
  activeTab.value = 'columns'

  // Phase 8: nested Firestore paths aren't in the top-level schema cache.
  // Fetch column metadata on demand so columns tab + CRUD flows work.
  if (
    engineKind.value === 'firestore'
    && name.includes('/')
    && !connectionsStore.tableColumns[name]
    && activeConnection.value
  ) {
    try {
      const cols = await invoke<ColumnInfo[]>('get_table_columns', {
        connectionId: activeConnection.value.id,
        tableName: name,
        schemaName: null,
      })
      // Mutating the store directly — this matches how loadSchema populates
      // tableColumns. The schema store ref is reactive so the computed
      // `selectedColumns` picks up the change.
      connectionsStore.tableColumns[name] = cols
    }
    catch (err) {
      toast.error('Could not load subcollection schema', { description: String(err) })
    }
  }
})

watch(previewData, () => {
  selectedRowIndex.value = null
  bulkSelectedIds.value = new Set()
  // Refresh after a write clears pending edits (they're now applied) — but
  // we also reset on every preview reload so a manual Refresh doesn't leave
  // stale pending markers pointing at rows that may have moved.
  pendingEdits.value = new Map()
  editingCell.value = null
})

watch(activeTab, async (tab) => {
  if (tab === 'preview' && selectedTableName.value && previewData.value.length === 0) {
    await loadPreview()
  }
})

async function loadPreview() {
  if (!activeConnection.value || !selectedTableName.value) return
  isLoadingPreview.value = true

  try {
    const data = await invoke<string>('preview_table_data', {
      connectionId: activeConnection.value.id,
      tableName: selectedTableName.value,
      schemaName: selectedTable.value?.schema,
      limit: 50,
    })
    previewData.value = JSON.parse(data)
  }
  catch (err) {
    toast.error('Failed to preview table', { description: err as string })
  }
  finally {
    isLoadingPreview.value = false
  }
}

function formatCellValue(val: unknown): string {
  if (val === null || val === undefined) return ''
  if (typeof val === 'object') return JSON.stringify(val)
  return String(val)
}

async function refreshSchema() {
  selectedTableName.value = null
  previewData.value = []
  connectionsStore.clearSchema()
  await connectionsStore.loadSchema()
}

// ============ CRUD state ============
// Dispatch table by engine:
//   sql        -> RowEditDialog + SqlConfirmDialog (form -> editable SQL -> exec)
//   mongo      -> MongoOpDialog       (JSON doc + JSON filter)
//   firestore  -> FirestoreOpDialog   (doc-id field + JSON doc; filter is URL path)
//   rtdb       -> RtdbOpDialog        (child-key + JSON value of any shape)
//   redis      -> RedisOpDialog       (key + type-picker + JSON value of type-specific shape + TTL)
//   other      -> read-only
type EngineKind = 'sql' | 'mongo' | 'firestore' | 'rtdb' | 'redis' | 'other'

const engineKind = computed<EngineKind>(() => {
  const e = activeConnection.value?.db_type ?? ''
  if (isSqlEngine(e)) return 'sql'
  if (isMongoEngine(e)) return 'mongo'
  if (isFirestoreEngine(e)) return 'firestore'
  if (isRtdbEngine(e)) return 'rtdb'
  if (isRedisEngine(e)) return 'redis'
  return 'other'
})

/** PK column names for the current selection. Composite PKs return >1
 *  entry; missing PK returns []. The dialogs and bulk operations target
 *  rows by the whole PK tuple.
 *
 *  Per-engine:
 *    SQL:       all columns with is_primary_key=true (declaration order)
 *    Mongo/FS:  ['_id']   (synthetic, injected by the read path)
 *    RTDB:      ['_key']  (object children only; _index-only nodes -> [])
 *    Redis:     ['key']   (the synthetic column the driver advertises)
 */
const pkColumns = computed<string[]>(() => {
  if (engineKind.value === 'sql') {
    return selectedColumns.value.filter(c => c.is_primary_key).map(c => c.name)
  }
  if (engineKind.value === 'mongo' || engineKind.value === 'firestore') {
    return selectedColumns.value.some(c => c.name === '_id') ? ['_id'] : []
  }
  if (engineKind.value === 'rtdb') {
    // Object children carry `_key`; array children carry `_index`. Both are
    // valid path segments — RTDB REST treats integer keys identically to
    // string keys in URLs. Insert/delete on arrays still has the sparse-
    // index footgun, so the OpDialog insert/delete buttons stay hidden for
    // index-keyed nodes (see canRtdbInsert below); inline edit is fine.
    if (selectedColumns.value.some(c => c.name === '_key')) return ['_key']
    if (selectedColumns.value.some(c => c.name === '_index')) return ['_index']
    return []
  }
  if (engineKind.value === 'redis') {
    return selectedColumns.value.some(c => c.name === 'key') ? ['key'] : []
  }
  return []
})

const canWrite = computed(() => engineKind.value !== 'other' && pkColumns.value.length > 0)
const writeBlockedReason = computed(() => {
  if (engineKind.value === 'other') {
    return 'Editing for this engine is coming in a future update.'
  }
  if (pkColumns.value.length === 0) {
    if (engineKind.value === 'sql') {
      return 'This table has no primary key — edits cannot safely target a single row.'
    }
    if (engineKind.value === 'rtdb') {
      return 'This node has no `_key` or `_index` on its children — editing requires one of those to target a child safely.'
    }
    if (engineKind.value === 'redis') {
      return 'No keys detected under this prefix — nothing to target.'
    }
    return 'No _id field detected in the sampled documents — edits cannot target a single document safely.'
  }
  return ''
})

// SQL dialog state
const rowEditOpen = ref(false)
const rowEditMode = ref<'insert' | 'edit'>('insert')
const rowEditTarget = ref<Record<string, unknown> | null>(null)

const sqlConfirmOpen = ref(false)
const sqlConfirmKind = ref<'insert' | 'update' | 'delete' | 'ddl' | 'drop'>('insert')
const sqlConfirmInitial = ref('')
/** When set, SqlConfirmDialog requires the user to type this exact name
 *  before Execute is enabled. Used for DROP TABLE / DROP COLUMN. */
const sqlConfirmTypedName = ref<string | undefined>(undefined)

// === Schema-editing dialog state (Phase 6) ===
const createTableOpen = ref(false)
const alterColumnOpen = ref(false)
const alterColumnMode = ref<'add' | 'rename' | 'change-type'>('add')
const alterColumnTarget = ref<{ name: string; type: string } | null>(null)

// === Firestore subcollection drill-in (Phase 8) ===
// The schema-page sidebar continues to list only top-level collections.
// When the user clicks "Subcollections" on a Firestore document row, we
// fetch the subs lazily, show them in a small popover, and let the user
// drill in by setting selectedTableName to the nested path (e.g.
// `users/abc/posts`). Firestore REST URLs natively support nested
// collection paths so the existing CRUD methods all work without changes.
//
// `firestoreSubsByDoc` caches results per doc path so re-opening the
// popover on the same doc doesn't re-fetch.
const firestoreSubsByDoc = ref<Map<string, string[]>>(new Map())
const firestoreSubsPopoverFor = ref<string | null>(null)  // row PK id (== doc id) currently showing the popover
const firestoreSubsLoading = ref<string | null>(null)

/** True when the current selectedTableName is a nested Firestore path
 *  (contains a `/`). Used to show the breadcrumb + back-out controls. */
const firestoreIsNested = computed(() =>
  engineKind.value === 'firestore'
  && !!selectedTableName.value
  && selectedTableName.value.includes('/'),
)

/** Render the current Firestore path as breadcrumb segments. Each segment
 *  alternates between collection name and doc id (collection at even
 *  indices, doc at odd). Clicking any segment navigates back to that
 *  level. */
const firestoreBreadcrumb = computed<{ label: string; path: string; isCollection: boolean }[]>(() => {
  if (engineKind.value !== 'firestore' || !selectedTableName.value) return []
  const parts = selectedTableName.value.split('/').filter(Boolean)
  const out: { label: string; path: string; isCollection: boolean }[] = []
  let path = ''
  for (let i = 0; i < parts.length; i++) {
    path = i === 0 ? parts[i]! : `${path}/${parts[i]!}`
    out.push({ label: parts[i]!, path, isCollection: i % 2 === 0 })
  }
  return out
})

function navigateFirestoreToBreadcrumb(idx: number) {
  // Truncate to (and including) the breadcrumb at idx — must land on a
  // collection (even index) so the right pane shows documents, not a
  // single doc.
  if (idx < 0) return
  if (idx % 2 !== 0) return  // odd = doc; click on doc segment is a no-op
  const newPath = firestoreBreadcrumb.value.slice(0, idx + 1).map(s => s.label).join('/')
  selectedTableName.value = newPath
}

async function toggleFirestoreSubsPopover(row: Record<string, unknown>) {
  if (engineKind.value !== 'firestore' || !activeConnection.value || !selectedTableName.value) return
  const docId = getFirestoreDocId(row)
  if (!docId) return
  // The path that uniquely identifies this doc — used both for caching
  // and for forming the subcollection's full nested path on drill-in.
  const docPath = `${selectedTableName.value}/${docId}`

  if (firestoreSubsPopoverFor.value === docPath) {
    firestoreSubsPopoverFor.value = null
    return
  }

  if (!firestoreSubsByDoc.value.has(docPath)) {
    firestoreSubsLoading.value = docPath
    try {
      const subs = await invoke<string[]>('firestore_list_subcollections', {
        connectionId: activeConnection.value.id,
        docPath,
      })
      const next = new Map(firestoreSubsByDoc.value)
      next.set(docPath, subs)
      firestoreSubsByDoc.value = next
    }
    catch (err) {
      toast.error('Could not list subcollections', { description: String(err) })
      firestoreSubsLoading.value = null
      return
    }
    finally {
      firestoreSubsLoading.value = null
    }
  }
  firestoreSubsPopoverFor.value = docPath
}

function drillIntoFirestoreSubcollection(docPath: string, sub: string) {
  selectedTableName.value = `${docPath}/${sub}`
  firestoreSubsPopoverFor.value = null
}

// === Inline cell editing state (Phase 7) ===
// SQL engines only. Pending edits accumulate as the user double-clicks
// cells, types new values, and presses Enter. Save buttons in the toolbar
// flush them all in one atomic transaction.
interface PendingEdit {
  /** PK column -> value tuple, in pkColumns order. Identity for the row. */
  pkTuple: unknown[]
  column: string
  dataType: string
  /** Original row value before this edit. Used to detect "no change" so
   *  we don't emit a no-op UPDATE. */
  originalValue: unknown
  /** Coerced typed value the UPDATE will use. */
  newValue: unknown
  /** Raw text the user typed — preserved so re-opening the cell shows
   *  what they had, not the round-tripped coerced value. */
  textValue: string
}
const pendingEdits = ref<Map<string, PendingEdit>>(new Map())

interface EditingCell {
  rowPkId: string
  column: string
  text: string
}
const editingCell = ref<EditingCell | null>(null)

// Batch-confirm dialog state — opened from "Save N changes" toolbar button.
const batchConfirmOpen = ref(false)
const batchConfirmStatements = ref<string[]>([])

function pendingKey(rowPkId: string, column: string): string {
  return `${rowPkId}|${column}`
}

function colInfoByName(name: string): ColumnInfo | undefined {
  return selectedColumns.value.find(c => c.name === name)
}

/** True when (row, column) supports inline editing: SQL engine, row has a
 *  resolvable PK identity, and the column is not itself a PK column. */
function isCellEditable(row: Record<string, unknown>, column: string): boolean {
  if (engineKind.value !== 'sql' || !canWrite.value) return false
  if (rowPkId(row) == null) return false
  if (pkColumns.value.includes(column)) return false
  return true
}

function startCellEdit(row: Record<string, unknown>, column: string) {
  if (!isCellEditable(row, column)) return
  const id = rowPkId(row)
  if (id == null) return
  const existing = pendingEdits.value.get(pendingKey(id, column))
  const seedText = existing != null
    ? existing.textValue
    : (row[column] == null ? '' : String(row[column]))
  editingCell.value = { rowPkId: id, column, text: seedText }
}

function cancelCellEdit() {
  editingCell.value = null
}

/** Compare coerced edit vs. original — uses the same fuzzy comparison as
 *  the row-edit dialog so reformatted-string edits don't trigger no-op
 *  UPDATEs and the pending-cell badge clears when the user reverts. */
function isSameAsOriginal(coerced: unknown, original: unknown): boolean {
  if (coerced === null || coerced === undefined) {
    return original === null || original === undefined
  }
  if (coerced === original) return true
  return String(coerced) === String(original)
}

function commitCellEdit(row: Record<string, unknown>) {
  const editing = editingCell.value
  if (!editing) return
  editingCell.value = null

  const colInfo = colInfoByName(editing.column)
  if (!colInfo) return
  const key = pendingKey(editing.rowPkId, editing.column)
  const original = row[editing.column]
  const coerced = coerceCellInput(editing.text, false, colInfo.data_type)

  if (isSameAsOriginal(coerced, original)) {
    // User reverted — drop any pending entry for this cell.
    if (pendingEdits.value.has(key)) {
      pendingEdits.value.delete(key)
      // Vue map mutation — trigger reactivity.
      pendingEdits.value = new Map(pendingEdits.value)
    }
    return
  }

  const next = new Map(pendingEdits.value)
  next.set(key, {
    pkTuple: rowPkTuple(row),
    column: editing.column,
    dataType: colInfo.data_type,
    originalValue: original,
    newValue: coerced,
    textValue: editing.text,
  })
  pendingEdits.value = next
}

function pendingEditFor(row: Record<string, unknown>, column: string): PendingEdit | undefined {
  const id = rowPkId(row)
  if (id == null) return undefined
  return pendingEdits.value.get(pendingKey(id, column))
}

function isCellInEditMode(row: Record<string, unknown>, column: string): boolean {
  const ec = editingCell.value
  if (!ec) return false
  return ec.rowPkId === rowPkId(row) && ec.column === column
}

function discardPendingEdits() {
  pendingEdits.value = new Map()
}

function openSavePending() {
  if (engineKind.value !== 'sql') return
  if (!selectedTable.value) return
  if (pendingEdits.value.size === 0) return

  // Group edits by row identity — one UPDATE per row with N SET clauses.
  // Fewer round-trips, cleaner SQL than one UPDATE per cell.
  interface RowGroup { pkBindings: PkBinding[]; sets: ValueBinding[] }
  const byRow = new Map<string, RowGroup>()
  for (const edit of pendingEdits.value.values()) {
    const groupKey = JSON.stringify(edit.pkTuple)
    let entry = byRow.get(groupKey)
    if (!entry) {
      entry = {
        pkBindings: pkColumns.value.map((c, i) => ({ column: c, value: edit.pkTuple[i] })),
        sets: [],
      }
      byRow.set(groupKey, entry)
    }
    entry.sets.push({ column: edit.column, value: edit.newValue, dataType: edit.dataType })
  }

  const statements: string[] = []
  for (const { pkBindings, sets } of byRow.values()) {
    statements.push(buildUpdate(
      activeConnection.value!.db_type,
      selectedTable.value.name,
      selectedTable.value.schema,
      sets,
      pkBindings,
    ))
  }

  batchConfirmStatements.value = statements
  batchConfirmOpen.value = true
}

async function onBatchExecuted() {
  pendingEdits.value = new Map()
  await loadPreview()
}

// Mongo dialog state
const mongoOpDialogOpen = ref(false)
const mongoOp = ref<'insert' | 'replace' | 'delete'>('insert')
const mongoInitialFilter = ref('')
const mongoInitialDocument = ref('')

// Firestore dialog state
const firestoreOpDialogOpen = ref(false)
const firestoreOp = ref<'insert' | 'replace' | 'delete'>('insert')
const firestoreInitialDocId = ref('')
const firestoreInitialDocument = ref('')

// RTDB dialog state
const rtdbOpDialogOpen = ref(false)
const rtdbOp = ref<'insert' | 'replace' | 'delete'>('insert')
const rtdbInitialKey = ref('')
const rtdbInitialValue = ref('')

// Redis dialog state
const redisOpDialogOpen = ref(false)
const redisOp = ref<'insert' | 'replace' | 'delete'>('insert')
const redisInitialKey = ref('')
const redisInitialType = ref<RedisType>('string')
const redisInitialValue = ref('')
const redisInitialTtl = ref<number | undefined>(undefined)

// NoSQL bulk-delete dialog state. SQL bulk still goes through SqlConfirmDialog.
const bulkDeleteOpen = ref(false)
const bulkDeleteEngine = ref<'mongo' | 'firestore' | 'rtdb' | 'redis'>('mongo')
const bulkDeleteContainer = ref('')
const bulkDeleteIds = ref<string[]>([])

// ===== Bulk selection (SQL only for Phase 3) =====
// Identity for selection is the PK tuple JSON-stringified — survives row
// reorder / refresh better than the array index would, and works the same
// for single-column and composite PKs.
const bulkSelectedIds = ref<Set<string>>(new Set())

function rowPkTuple(row: Record<string, unknown>): unknown[] {
  return pkColumns.value.map(c => row[c])
}

function rowPkId(row: Record<string, unknown>): string | null {
  if (pkColumns.value.length === 0) return null
  const tuple = rowPkTuple(row)
  // A null in any PK position means we can't uniquely identify the row;
  // refuse to assign an id so it gets disabled in the checkbox column.
  if (tuple.some(v => v === null || v === undefined)) return null
  return JSON.stringify(tuple)
}

const allVisibleSelected = computed(() => {
  if (previewData.value.length === 0) return false
  for (const row of previewData.value) {
    const id = rowPkId(row)
    if (id == null) continue
    if (!bulkSelectedIds.value.has(id)) return false
  }
  return true
})

function toggleRowSelection(row: Record<string, unknown>) {
  const id = rowPkId(row)
  if (id == null) return
  const next = new Set(bulkSelectedIds.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  bulkSelectedIds.value = next
}

function toggleSelectAll() {
  if (allVisibleSelected.value) {
    bulkSelectedIds.value = new Set()
    return
  }
  const next = new Set<string>()
  for (const row of previewData.value) {
    const id = rowPkId(row)
    if (id != null) next.add(id)
  }
  bulkSelectedIds.value = next
}

const exportOpen = ref(false)
const exportData = computed(() => JSON.stringify(previewData.value))

function openInsert() {
  if (engineKind.value === 'sql') {
    rowEditMode.value = 'insert'
    rowEditTarget.value = null
    rowEditOpen.value = true
    return
  }
  if (engineKind.value === 'mongo') {
    mongoOp.value = 'insert'
    mongoInitialFilter.value = ''
    mongoInitialDocument.value = prettyJsonMongo({})
    mongoOpDialogOpen.value = true
    return
  }
  if (engineKind.value === 'firestore') {
    firestoreOp.value = 'insert'
    firestoreInitialDocId.value = ''
    firestoreInitialDocument.value = prettyJsonFirestore({})
    firestoreOpDialogOpen.value = true
    return
  }
  if (engineKind.value === 'rtdb') {
    rtdbOp.value = 'insert'
    rtdbInitialKey.value = ''
    rtdbInitialValue.value = prettyJsonRtdb({})
    rtdbOpDialogOpen.value = true
    return
  }
  if (engineKind.value === 'redis') {
    redisOp.value = 'insert'
    redisInitialKey.value = ''
    redisInitialType.value = 'string'
    redisInitialValue.value = defaultRedisValueFor('string')
    redisInitialTtl.value = undefined
    redisOpDialogOpen.value = true
  }
}

function openEdit(row: Record<string, unknown>) {
  if (!canWrite.value) return
  if (engineKind.value === 'sql') {
    rowEditMode.value = 'edit'
    rowEditTarget.value = row
    rowEditOpen.value = true
    return
  }
  if (engineKind.value === 'mongo') {
    const id = getMongoDocId(row)
    if (id === null || id === undefined) {
      toast.error('_id is missing on this document — cannot edit safely.')
      return
    }
    mongoOp.value = 'replace'
    mongoInitialFilter.value = buildIdFilter(id)
    mongoInitialDocument.value = prettyJsonMongo(row)
    mongoOpDialogOpen.value = true
    return
  }
  if (engineKind.value === 'firestore') {
    const id = getFirestoreDocId(row)
    if (!id) {
      toast.error('_id is missing on this document — cannot edit safely.')
      return
    }
    firestoreOp.value = 'replace'
    firestoreInitialDocId.value = id
    firestoreInitialDocument.value = prettyJsonFirestore(stripFirestoreMetadata(row))
    firestoreOpDialogOpen.value = true
    return
  }
  if (engineKind.value === 'rtdb') {
    const key = getRtdbRowKey(row)
    if (!key) {
      toast.error('_key is missing on this row — cannot edit safely.')
      return
    }
    rtdbOp.value = 'replace'
    rtdbInitialKey.value = key
    rtdbInitialValue.value = prettyJsonRtdb(rowToChildJson(row))
    rtdbOpDialogOpen.value = true
    return
  }
  if (engineKind.value === 'redis') {
    const key = getRedisKey(row)
    const previewType = getRedisType(row)
    if (!key || !previewType) {
      toast.error('Cannot identify this Redis key from the row.')
      return
    }
    // Re-read current state from the server — preview data can be stale,
    // especially with TTLs ticking down. If the read fails (key gone,
    // permissions etc.) fall back to the preview values so the dialog
    // still opens with something sensible.
    invoke<RedisKeyView>('redis_get_key', {
      connectionId: activeConnection.value!.id,
      key,
    })
      .then((view) => {
        if (view.type === 'none') {
          toast.error(`Key '${key}' no longer exists on the server.`)
          return
        }
        redisOp.value = 'replace'
        redisInitialKey.value = view.key
        redisInitialType.value = (view.type as RedisType)
        redisInitialValue.value = prettyJsonRedis(view.value)
        redisInitialTtl.value = view.ttl_seconds
        redisOpDialogOpen.value = true
      })
      .catch((err) => {
        toast.error('Could not fetch the latest value', { description: String(err) })
      })
  }
}

function openDelete(row: Record<string, unknown>) {
  if (!canWrite.value || !selectedTable.value) return
  if (engineKind.value === 'sql' && pkColumns.value.length > 0) {
    const pkBindings: PkBinding[] = []
    for (const col of pkColumns.value) {
      const v = row[col]
      if (v === null || v === undefined) {
        toast.error(`Primary key '${col}' is null on this row — cannot delete safely.`)
        return
      }
      pkBindings.push({ column: col, value: v })
    }
    sqlConfirmInitial.value = buildDelete(
      activeConnection.value!.db_type,
      selectedTable.value.name,
      selectedTable.value.schema,
      pkBindings,
    )
    sqlConfirmKind.value = 'delete'
    sqlConfirmTypedName.value = undefined
    sqlConfirmOpen.value = true
    return
  }
  if (engineKind.value === 'mongo') {
    const id = getMongoDocId(row)
    if (id === null || id === undefined) {
      toast.error('_id is missing on this document — cannot delete safely.')
      return
    }
    mongoOp.value = 'delete'
    mongoInitialFilter.value = buildIdFilter(id)
    mongoInitialDocument.value = ''
    mongoOpDialogOpen.value = true
    return
  }
  if (engineKind.value === 'firestore') {
    const id = getFirestoreDocId(row)
    if (!id) {
      toast.error('_id is missing on this document — cannot delete safely.')
      return
    }
    firestoreOp.value = 'delete'
    firestoreInitialDocId.value = id
    firestoreInitialDocument.value = ''
    firestoreOpDialogOpen.value = true
    return
  }
  if (engineKind.value === 'rtdb') {
    const key = getRtdbRowKey(row)
    if (!key) {
      toast.error('_key is missing on this row — cannot delete safely.')
      return
    }
    rtdbOp.value = 'delete'
    rtdbInitialKey.value = key
    rtdbInitialValue.value = ''
    rtdbOpDialogOpen.value = true
    return
  }
  if (engineKind.value === 'redis') {
    const key = getRedisKey(row)
    const previewType = getRedisType(row)
    if (!key) {
      toast.error('Cannot identify this Redis key from the row.')
      return
    }
    redisOp.value = 'delete'
    redisInitialKey.value = key
    redisInitialType.value = previewType ?? 'string'
    redisInitialValue.value = ''
    redisInitialTtl.value = undefined
    redisOpDialogOpen.value = true
  }
}

function onRowEditConfirm(sql: string) {
  sqlConfirmInitial.value = sql
  sqlConfirmKind.value = rowEditMode.value === 'insert' ? 'insert' : 'update'
  sqlConfirmTypedName.value = undefined
  sqlConfirmOpen.value = true
}

async function onWriteExecuted(_affected: number) {
  // DDL writes change the schema, not just rows — reload the schema cache
  // so the column list / table list reflect the change. Cheap to do
  // unconditionally; the store de-dupes when the cache is still warm.
  if (sqlConfirmKind.value === 'ddl' || sqlConfirmKind.value === 'drop') {
    connectionsStore.clearSchema()
    await connectionsStore.loadSchema()
    // If the user just dropped the selected table, clear the selection so
    // the right pane doesn't try to render a phantom column list.
    if (selectedTable.value && !tables.value.find(t => t.name === selectedTable.value!.name)) {
      selectedTableName.value = null
    }
    return
  }
  await loadPreview()
}

// === Schema-editing handlers (Phase 6) ===

function openCreateTable() {
  if (engineKind.value !== 'sql') return
  createTableOpen.value = true
}

function onCreateTableConfirm(sql: string) {
  sqlConfirmInitial.value = sql
  sqlConfirmKind.value = 'ddl'
  sqlConfirmTypedName.value = undefined
  sqlConfirmOpen.value = true
}

function openDropTable() {
  if (engineKind.value !== 'sql' || !selectedTable.value) return
  sqlConfirmInitial.value = buildDropTable(
    activeConnection.value!.db_type,
    selectedTable.value.name,
    selectedTable.value.schema,
  )
  sqlConfirmKind.value = 'drop'
  sqlConfirmTypedName.value = selectedTable.value.name
  sqlConfirmOpen.value = true
}

function openAddColumn() {
  if (engineKind.value !== 'sql' || !selectedTable.value) return
  alterColumnMode.value = 'add'
  alterColumnTarget.value = null
  alterColumnOpen.value = true
}

function openRenameColumn(col: ColumnInfo) {
  if (engineKind.value !== 'sql' || !selectedTable.value) return
  alterColumnMode.value = 'rename'
  alterColumnTarget.value = { name: col.name, type: col.data_type }
  alterColumnOpen.value = true
}

function openChangeColumnType(col: ColumnInfo) {
  if (engineKind.value !== 'sql' || !selectedTable.value) return
  alterColumnMode.value = 'change-type'
  alterColumnTarget.value = { name: col.name, type: col.data_type }
  alterColumnOpen.value = true
}

function onAlterColumnConfirm(sql: string) {
  sqlConfirmInitial.value = sql
  sqlConfirmKind.value = 'ddl'
  sqlConfirmTypedName.value = undefined
  sqlConfirmOpen.value = true
}

function openDropColumn(col: ColumnInfo) {
  if (engineKind.value !== 'sql' || !selectedTable.value) return
  sqlConfirmInitial.value = buildDropColumn(
    activeConnection.value!.db_type,
    selectedTable.value.name,
    selectedTable.value.schema,
    col.name,
  )
  sqlConfirmKind.value = 'drop'
  sqlConfirmTypedName.value = col.name
  sqlConfirmOpen.value = true
}

/** Open the right bulk-delete UI for the current engine.
 *
 *  SQL goes through SqlConfirmDialog (a single multi-row DELETE statement
 *  shown for review). NoSQL goes through BulkDeleteDialog which calls the
 *  engine-specific *_delete_many command directly. */
function openBulkDelete() {
  if (!canWrite.value || !selectedTable.value) return
  if (bulkSelectedIds.value.size === 0) return

  // Collect rows in display order for stable previews.
  const selectedRows: Record<string, unknown>[] = []
  for (const row of previewData.value) {
    const id = rowPkId(row)
    if (id != null && bulkSelectedIds.value.has(id)) {
      selectedRows.push(row)
    }
  }
  if (selectedRows.length === 0) {
    toast.error('Selected rows are no longer in the preview — refresh and try again.')
    return
  }

  if (engineKind.value === 'sql') {
    const tuples = selectedRows.map(rowPkTuple)
    sqlConfirmInitial.value = buildBulkDelete(
      activeConnection.value!.db_type,
      selectedTable.value.name,
      selectedTable.value.schema,
      pkColumns.value,
      tuples,
    )
    sqlConfirmKind.value = 'delete'
    sqlConfirmTypedName.value = undefined
    sqlConfirmOpen.value = true
    return
  }

  // NoSQL: extract the single-column PK value (already string for our
  // synthetic PKs) and hand off to BulkDeleteDialog.
  const pkCol = pkColumns.value[0]!
  const ids: string[] = []
  for (const row of selectedRows) {
    const v = row[pkCol]
    if (typeof v === 'string') ids.push(v)
    else if (v != null) ids.push(String(v))
  }
  if (ids.length === 0) {
    toast.error('No identifiable rows in the selection.')
    return
  }

  if (engineKind.value === 'mongo') {
    bulkDeleteEngine.value = 'mongo'
    bulkDeleteContainer.value = selectedTable.value.name
  }
  else if (engineKind.value === 'firestore') {
    bulkDeleteEngine.value = 'firestore'
    bulkDeleteContainer.value = selectedTable.value.name
  }
  else if (engineKind.value === 'rtdb') {
    bulkDeleteEngine.value = 'rtdb'
    bulkDeleteContainer.value = selectedTable.value.name
  }
  else if (engineKind.value === 'redis') {
    bulkDeleteEngine.value = 'redis'
    bulkDeleteContainer.value = ''  // Redis keys are global; container unused
  }
  else {
    return
  }
  bulkDeleteIds.value = ids
  bulkDeleteOpen.value = true
}
</script>

<template>
  <div class="flex flex-col h-full overflow-hidden gap-0">
    <!-- Header -->
    <div class="flex items-center justify-between shrink-0 pb-4">
      <div>
        <h1 class="text-lg font-semibold">Schema Explorer</h1>
        <p class="text-sm text-muted-foreground">
          <template v-if="tables.length > 0">
            {{ tables.length }} tables · {{ activeConnection?.name }}
          </template>
          <template v-else>
            Browse tables and columns in your database
          </template>
        </p>
      </div>

      <div class="flex items-center gap-2">
        <!-- Schema-editing entry point: visible for SQL connections only. -->
        <Button
          v-if="activeConnection && engineKind === 'sql'"
          variant="outline"
          size="sm"
          @click="openCreateTable"
        >
          <Icon name="lucide:plus-circle" class="size-4" />
          New table
        </Button>
        <Button
          v-if="activeConnection"
          variant="outline"
          size="sm"
          :disabled="isLoadingSchema"
          @click="refreshSchema"
        >
          <Icon v-if="isLoadingSchema" name="lucide:loader-2" class="size-4 animate-spin" />
          <Icon v-else name="lucide:refresh-cw" class="size-4" />
          Refresh
        </Button>
      </div>
    </div>

    <!-- No connection -->
    <div v-if="!activeConnection" class="flex flex-1 flex-col items-center justify-center text-muted-foreground gap-3 border border-dashed border-border rounded-lg">
      <Icon name="lucide:database" class="size-10" />
      <div class="text-center">
        <p class="font-medium text-foreground">No connection selected</p>
        <p class="text-sm mt-1">Click a connection in the sidebar to connect</p>
      </div>
    </div>

    <!-- Loading schema -->
    <div v-else-if="isLoadingSchema && tables.length === 0" class="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground">
      <Icon name="lucide:loader-2" class="size-8 animate-spin text-primary" />
      <p class="text-sm">Loading database schema...</p>
    </div>

    <!-- Main 2-panel layout -->
    <div v-else-if="tables.length > 0" class="flex-1 overflow-hidden flex gap-0 rounded-lg border border-border">
      <!-- Left: Table list -->
      <div class="w-56 shrink-0 flex flex-col border-r border-border">
        <!-- Search -->
        <div class="p-2 border-b border-border">
          <div class="relative">
            <Icon name="lucide:search" class="absolute left-2 top-2 size-3.5 text-muted-foreground" />
            <Input
              v-model="searchTerm"
              placeholder="Search tables..."
              class="pl-7 h-7 text-xs"
            />
          </div>
        </div>

        <!-- Table list -->
        <div class="overflow-y-auto flex-1 p-1">
          <button
            v-for="table in filteredTables"
            :key="`${table.schema}-${table.name}`"
            class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left transition-colors hover:bg-accent"
            :class="selectedTableName === table.name
              ? 'bg-accent text-foreground font-medium'
              : 'text-muted-foreground'"
            @click="selectedTableName = table.name"
          >
            <Icon
              :name="table.table_type === 'VIEW' ? 'lucide:eye' : 'lucide:table-2'"
              class="size-3.5 shrink-0"
            />
            <span class="truncate text-xs">{{ table.name }}</span>
          </button>

          <p v-if="filteredTables.length === 0" class="text-xs text-muted-foreground p-2 text-center">
            No tables match "{{ searchTerm }}"
          </p>
        </div>
      </div>

      <!-- Right: Table detail -->
      <div class="flex-1 flex flex-col overflow-hidden">
        <template v-if="selectedTable">
          <!-- Table header -->
          <div class="flex items-center justify-between px-4 py-2.5 border-b border-border shrink-0">
            <div class="flex items-center gap-2 min-w-0 flex-1">
              <Icon name="lucide:table-2" class="size-4 text-muted-foreground shrink-0" />
              <!-- Firestore nested path: render as clickable breadcrumb so
                   users can pop back to a parent collection without resetting
                   to the top-level sidebar. -->
              <template v-if="firestoreIsNested">
                <div class="flex items-center gap-1 font-mono text-xs min-w-0">
                  <template v-for="(seg, i) in firestoreBreadcrumb" :key="i">
                    <span v-if="i > 0" class="text-muted-foreground">/</span>
                    <button
                      v-if="seg.isCollection && i < firestoreBreadcrumb.length - 1"
                      class="text-primary hover:underline truncate"
                      @click="navigateFirestoreToBreadcrumb(i)"
                    >
                      {{ seg.label }}
                    </button>
                    <span v-else class="font-medium text-foreground truncate">{{ seg.label }}</span>
                  </template>
                </div>
              </template>
              <span v-else class="font-medium text-sm">{{ selectedTable.name }}</span>
              <Badge v-if="selectedTable.schema" variant="outline" class="text-xs">{{ selectedTable.schema }}</Badge>
              <Badge variant="secondary" class="text-xs">{{ selectedTable.table_type }}</Badge>
            </div>
            <div class="flex items-center gap-2 text-xs text-muted-foreground">
              <span>{{ selectedColumns.length }} columns</span>
              <Button
                v-if="engineKind === 'sql'"
                variant="ghost"
                size="sm"
                class="h-7 text-xs text-destructive hover:text-destructive hover:bg-destructive/10"
                @click="openDropTable"
              >
                <Icon name="lucide:trash-2" class="size-3.5" />
                Drop table
              </Button>
            </div>
          </div>

          <!-- Tabs -->
          <div class="flex border-b border-border shrink-0 px-4">
            <button
              v-for="tab in ['columns', 'preview'] as const"
              :key="tab"
              class="px-3 py-2 text-xs font-medium border-b-2 capitalize transition-colors"
              :class="activeTab === tab
                ? 'border-primary text-foreground'
                : 'border-transparent text-muted-foreground hover:text-foreground'"
              @click="activeTab = tab"
            >
              {{ tab }}
            </button>
          </div>

          <!-- Columns tab -->
          <!-- :key forces a fresh mount per table so scrollTop resets to 0
               when you click a table after scrolling through another's columns. -->
          <div v-if="activeTab === 'columns'" :key="selectedTableName" class="flex-1 flex flex-col overflow-hidden">
            <!-- Schema-editing toolbar (SQL only). Hover actions on each
                 column row let you rename / change type / drop the column. -->
            <div v-if="engineKind === 'sql'" class="flex items-center gap-2 px-3 py-1.5 border-b border-border shrink-0">
              <Button variant="outline" size="sm" class="h-7 text-xs" @click="openAddColumn">
                <Icon name="lucide:plus" class="size-3.5" />
                Add column
              </Button>
            </div>

            <div class="overflow-auto flex-1">
              <table class="w-full text-sm">
                <thead class="sticky top-0 bg-muted/80 backdrop-blur-sm">
                  <tr class="border-b border-border">
                    <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">Column</th>
                    <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">Type</th>
                    <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">Nullable</th>
                    <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">Keys</th>
                    <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">Default</th>
                    <th v-if="engineKind === 'sql'" class="px-2 py-2 w-24" />
                  </tr>
                </thead>
                <tbody class="divide-y divide-border">
                  <tr v-for="col in selectedColumns" :key="col.name" class="group hover:bg-muted/20 transition-colors">
                    <td class="px-4 py-2 font-mono text-xs text-foreground font-medium">{{ col.name }}</td>
                    <td class="px-4 py-2 font-mono text-xs text-muted-foreground">{{ col.data_type }}</td>
                    <td class="px-4 py-2 text-xs text-muted-foreground">{{ col.is_nullable ? 'YES' : 'NO' }}</td>
                    <td class="px-4 py-2">
                      <div class="flex gap-1">
                        <Badge v-if="col.is_primary_key" class="text-[10px] py-0 h-4">PK</Badge>
                        <Badge v-if="col.is_foreign_key" variant="secondary" class="text-[10px] py-0 h-4" :title="`→ ${col.referenced_table}.${col.referenced_column}`">
                          FK
                        </Badge>
                      </div>
                    </td>
                    <td class="px-4 py-2 text-xs text-muted-foreground font-mono">
                      {{ col.column_default ?? '—' }}
                    </td>
                    <td v-if="engineKind === 'sql'" class="px-2 py-1 w-24">
                      <div class="flex items-center justify-end gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                        <button
                          class="rounded p-1 hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                          title="Rename column"
                          @click.stop="openRenameColumn(col)"
                        >
                          <Icon name="lucide:pencil" class="size-3.5" />
                        </button>
                        <button
                          class="rounded p-1 hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                          :title="activeConnection?.db_type === 'sqlite' ? 'Change type (sqlite: not supported in place)' : 'Change column type'"
                          @click.stop="openChangeColumnType(col)"
                        >
                          <Icon name="lucide:type" class="size-3.5" />
                        </button>
                        <button
                          class="rounded p-1 hover:bg-destructive/20 text-muted-foreground hover:text-destructive transition-colors"
                          title="Drop column"
                          @click.stop="openDropColumn(col)"
                        >
                          <Icon name="lucide:trash-2" class="size-3.5" />
                        </button>
                      </div>
                    </td>
                  </tr>
                  <tr v-if="selectedColumns.length === 0">
                    <td :colspan="engineKind === 'sql' ? 6 : 5" class="px-4 py-8 text-center text-xs text-muted-foreground">
                      No column data available
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>

          <!-- Preview tab -->
          <div v-else-if="activeTab === 'preview'" class="flex-1 flex flex-col overflow-hidden">
            <!-- Preview toolbar (sticky above the scrollable table) -->
            <div class="flex items-center justify-between gap-2 px-3 py-1.5 border-b border-border shrink-0">
              <div class="flex items-center gap-1.5">
                <Button
                  v-if="engineKind !== 'other'"
                  variant="outline"
                  size="sm"
                  class="h-7 text-xs"
                  :disabled="!selectedTable"
                  @click="openInsert"
                >
                  <Icon name="lucide:plus" class="size-3.5" />
                  {{
                    engineKind === 'mongo' || engineKind === 'firestore'
                      ? 'Insert document'
                      : engineKind === 'rtdb'
                        ? 'Insert child'
                        : engineKind === 'redis'
                          ? 'Insert key'
                          : 'Insert row'
                  }}
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  class="h-7 text-xs"
                  :disabled="isLoadingPreview || !selectedTable"
                  @click="loadPreview"
                >
                  <Icon v-if="isLoadingPreview" name="lucide:loader-2" class="size-3.5 animate-spin" />
                  <Icon v-else name="lucide:refresh-cw" class="size-3.5" />
                  Refresh
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  class="h-7 text-xs"
                  :disabled="previewData.length === 0"
                  @click="exportOpen = true"
                >
                  <Icon name="lucide:download" class="size-3.5" />
                  Export
                </Button>
                <!-- Bulk delete — Phase 4: SQL goes through SqlConfirmDialog;
                     NoSQL engines go through BulkDeleteDialog. -->
                <Button
                  v-if="canWrite && bulkSelectedIds.size > 0"
                  variant="destructive"
                  size="sm"
                  class="h-7 text-xs"
                  @click="openBulkDelete"
                >
                  <Icon name="lucide:trash-2" class="size-3.5" />
                  Delete {{ bulkSelectedIds.size }} selected
                </Button>
                <!-- Inline-edit pending batch (Phase 7). SQL only. -->
                <Button
                  v-if="engineKind === 'sql' && pendingEdits.size > 0"
                  variant="default"
                  size="sm"
                  class="h-7 text-xs"
                  @click="openSavePending"
                >
                  <Icon name="lucide:save" class="size-3.5" />
                  Save {{ pendingEdits.size }} {{ pendingEdits.size === 1 ? 'change' : 'changes' }}
                </Button>
                <Button
                  v-if="engineKind === 'sql' && pendingEdits.size > 0"
                  variant="outline"
                  size="sm"
                  class="h-7 text-xs"
                  @click="discardPendingEdits"
                >
                  Discard
                </Button>
              </div>
              <p v-if="engineKind !== 'other' && !canWrite && selectedColumns.length > 0" class="text-[11px] text-muted-foreground italic">
                {{ writeBlockedReason }}
              </p>
              <p
                v-else-if="engineKind === 'sql' && canWrite && pendingEdits.size === 0"
                class="text-[11px] text-muted-foreground italic"
              >
                Double-click a cell to edit · Enter saves, Esc cancels
              </p>
            </div>

            <div :key="selectedTableName" class="overflow-auto flex-1">
              <div v-if="isLoadingPreview" class="flex flex-col gap-1.5 p-4">
                <div v-for="i in 5" :key="i" class="h-8 bg-muted/50 rounded animate-pulse" />
              </div>
              <table v-else-if="previewData.length > 0" class="w-full text-sm border-separate border-spacing-0">
                <thead>
                  <tr>
                    <!-- Bulk-select column. SQL only. Sticky-left so it stays
                         visible during horizontal scroll, mirroring the
                         sticky-right actions column. -->
                    <th
                      v-if="canWrite"
                      class="sticky top-0 left-0 z-20 bg-muted/80 backdrop-blur-sm border-b border-r border-border w-10 px-2 py-2"
                    >
                      <input
                        type="checkbox"
                        class="size-3.5 cursor-pointer accent-primary align-middle"
                        :checked="allVisibleSelected"
                        :indeterminate.prop="bulkSelectedIds.size > 0 && !allVisibleSelected"
                        :title="allVisibleSelected ? 'Clear selection' : 'Select all rows'"
                        @click.stop="toggleSelectAll"
                      >
                    </th>
                    <th
                      v-for="col in previewColumns"
                      :key="col"
                      class="sticky top-0 z-10 bg-muted/80 backdrop-blur-sm border-b border-border px-4 py-2 text-left text-xs font-medium text-muted-foreground whitespace-nowrap"
                    >
                      {{ col }}
                    </th>
                    <th
                      v-if="canWrite"
                      class="sticky top-0 right-0 z-20 bg-muted/80 backdrop-blur-sm border-b border-l border-border w-28 px-2 py-2"
                    />
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="(row, i) in previewData"
                    :key="i"
                    class="group cursor-pointer transition-colors"
                    :class="selectedRowIndex === i ? 'bg-muted/40' : 'hover:bg-muted/20'"
                    @click="selectedRowIndex = i"
                  >
                    <td
                      v-if="canWrite"
                      class="sticky left-0 z-10 border-b border-r border-border px-2 py-1 w-10 backdrop-blur-sm transition-colors"
                      :class="selectedRowIndex === i ? 'bg-muted/40' : 'bg-background/80 group-hover:bg-muted/20'"
                    >
                      <input
                        type="checkbox"
                        class="size-3.5 cursor-pointer accent-primary align-middle"
                        :checked="rowPkId(row) != null && bulkSelectedIds.has(rowPkId(row)!)"
                        :disabled="rowPkId(row) == null"
                        :title="rowPkId(row) == null ? 'Cannot select — PK has null values' : ''"
                        @click.stop="toggleRowSelection(row)"
                      >
                    </td>
                    <td
                      v-for="col in previewColumns"
                      :key="col"
                      class="border-b border-border px-4 py-1.5 text-xs font-mono text-foreground max-w-[200px] relative"
                      :class="[
                        isCellInEditMode(row, col) ? 'p-0' : 'truncate',
                        pendingEditFor(row, col) ? 'border-l-2 border-l-primary bg-primary/5' : '',
                        isCellEditable(row, col) ? 'cursor-text' : '',
                      ]"
                      @dblclick.stop="startCellEdit(row, col)"
                    >
                      <!-- Inline edit mode: <input> takes over the cell. -->
                      <input
                        v-if="isCellInEditMode(row, col)"
                        :value="editingCell!.text"
                        class="w-full h-full px-4 py-1.5 text-xs font-mono bg-background border border-primary outline-none"
                        autofocus
                        spellcheck="false"
                        @input="editingCell!.text = ($event.target as HTMLInputElement).value"
                        @keydown.enter.prevent="commitCellEdit(row)"
                        @keydown.escape.prevent="cancelCellEdit"
                        @blur="commitCellEdit(row)"
                        @click.stop
                      >
                      <!-- Pending: show edited value with primary-coloured marker. -->
                      <template v-else-if="pendingEditFor(row, col)">
                        <span
                          :class="pendingEditFor(row, col)!.newValue === null ? 'text-muted-foreground italic' : 'text-primary'"
                        >
                          {{ pendingEditFor(row, col)!.newValue === null ? 'null' : formatCellValue(pendingEditFor(row, col)!.newValue) }}
                        </span>
                      </template>
                      <!-- Read mode. -->
                      <template v-else>
                        <span v-if="row[col] === null" class="text-muted-foreground italic">null</span>
                        <span v-else>{{ formatCellValue(row[col]) }}</span>
                      </template>
                    </td>
                    <td
                      v-if="canWrite"
                      class="sticky right-0 z-10 border-b border-l border-border px-2 py-1 w-28 backdrop-blur-sm transition-colors"
                      :class="selectedRowIndex === i ? 'bg-muted/40' : 'bg-background/80 group-hover:bg-muted/20'"
                    >
                      <!-- Actions are revealed on row hover (desktop) or when the
                           row is selected by click (touch / trackpad tap). The
                           sticky-right positioning keeps them anchored to the
                           viewport edge so wide tables don't hide them behind
                           horizontal scroll. -->
                      <div
                        class="flex items-center justify-end gap-1 transition-opacity relative"
                        :class="selectedRowIndex === i ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'"
                      >
                        <!-- Phase 8: per-row Subcollections drill-in for
                             Firestore documents. The popover is inline-absolute
                             so it floats over neighbouring rows when open. -->
                        <button
                          v-if="engineKind === 'firestore'"
                          class="rounded p-1 hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                          title="View subcollections"
                          @click.stop="toggleFirestoreSubsPopover(row)"
                        >
                          <Icon
                            v-if="firestoreSubsLoading === `${selectedTableName}/${getFirestoreDocId(row)}`"
                            name="lucide:loader-2"
                            class="size-3.5 animate-spin"
                          />
                          <Icon v-else name="lucide:folder-tree" class="size-3.5" />
                        </button>
                        <button
                          class="rounded p-1 hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                          title="Edit row"
                          @click.stop="openEdit(row)"
                        >
                          <Icon name="lucide:pencil" class="size-3.5" />
                        </button>
                        <button
                          class="rounded p-1 hover:bg-destructive/20 text-muted-foreground hover:text-destructive transition-colors"
                          title="Delete row"
                          @click.stop="openDelete(row)"
                        >
                          <Icon name="lucide:trash-2" class="size-3.5" />
                        </button>

                        <!-- Subcollections popover. Anchored to the bottom-right
                             of the action cell; clicking outside dismisses via
                             the transparent overlay backdrop. -->
                        <template
                          v-if="engineKind === 'firestore'
                            && firestoreSubsPopoverFor === `${selectedTableName}/${getFirestoreDocId(row)}`"
                        >
                          <div
                            class="fixed inset-0 z-30"
                            @click="firestoreSubsPopoverFor = null"
                          />
                          <div class="absolute right-0 top-full mt-1 z-40 bg-popover border border-border rounded-md shadow-lg min-w-[180px] py-1">
                            <div class="px-3 py-1.5 text-[10px] uppercase tracking-wide text-muted-foreground border-b border-border">
                              Subcollections
                            </div>
                            <template v-if="(firestoreSubsByDoc.get(`${selectedTableName}/${getFirestoreDocId(row)}`) ?? []).length === 0">
                              <div class="px-3 py-2 text-xs text-muted-foreground italic">
                                No subcollections
                              </div>
                            </template>
                            <button
                              v-for="sub in (firestoreSubsByDoc.get(`${selectedTableName}/${getFirestoreDocId(row)}`) ?? [])"
                              :key="sub"
                              class="block w-full text-left px-3 py-1.5 text-xs font-mono hover:bg-accent transition-colors"
                              @click="drillIntoFirestoreSubcollection(`${selectedTableName}/${getFirestoreDocId(row)}`, sub)"
                            >
                              <Icon name="lucide:folder" class="size-3 inline mr-1 text-muted-foreground" />
                              {{ sub }}
                            </button>
                          </div>
                        </template>
                      </div>
                    </td>
                  </tr>
                </tbody>
              </table>
              <div v-else class="flex flex-col items-center justify-center flex-1 py-12 text-muted-foreground">
                <Icon name="lucide:table" class="size-8 mb-3" />
                <p class="text-sm">No preview data</p>
              </div>
            </div>
          </div>
        </template>

        <!-- Nothing selected -->
        <div v-else class="flex flex-1 items-center justify-center text-muted-foreground">
          <div class="text-center">
            <Icon name="lucide:arrow-left" class="size-6 mx-auto mb-2" />
            <p class="text-sm">Select a table to explore</p>
          </div>
        </div>
      </div>
    </div>

    <!-- Connected but no tables -->
    <div v-else class="flex flex-1 flex-col items-center justify-center text-muted-foreground gap-3 border border-dashed border-border rounded-lg">
      <Icon name="lucide:table-2" class="size-10" />
      <div class="text-center">
        <p class="font-medium text-foreground">No tables found</p>
        <p class="text-sm mt-1">Connect to a database to explore its schema</p>
      </div>
    </div>

    <!-- SQL CRUD dialogs (postgres / mysql / sqlite) -->
    <RowEditDialog
      v-if="selectedTable && activeConnection && engineKind === 'sql'"
      v-model:open="rowEditOpen"
      :mode="rowEditMode"
      :engine="activeConnection.db_type"
      :table-name="selectedTable.name"
      :schema="selectedTable.schema ?? null"
      :columns="selectedColumns"
      :pk-columns="pkColumns"
      :initial-row="rowEditTarget"
      @confirm="onRowEditConfirm"
    />
    <SqlConfirmDialog
      v-if="activeConnection && engineKind === 'sql'"
      v-model:open="sqlConfirmOpen"
      :kind="sqlConfirmKind"
      :connection-id="activeConnection.id"
      :initial-sql="sqlConfirmInitial"
      :require-typed-confirmation="sqlConfirmTypedName"
      @executed="onWriteExecuted"
    />
    <!-- Inline-edit batch save (Phase 7). SQL engines only. -->
    <BatchConfirmDialog
      v-if="activeConnection && engineKind === 'sql'"
      v-model:open="batchConfirmOpen"
      :connection-id="activeConnection.id"
      :statements="batchConfirmStatements"
      @executed="onBatchExecuted"
    />

    <!-- Mongo CRUD dialog (insert / replace / delete in one component) -->
    <MongoOpDialog
      v-if="activeConnection && selectedTable && engineKind === 'mongo'"
      v-model:open="mongoOpDialogOpen"
      :op="mongoOp"
      :connection-id="activeConnection.id"
      :collection="selectedTable.name"
      :initial-filter="mongoInitialFilter"
      :initial-document="mongoInitialDocument"
      @executed="onWriteExecuted"
    />

    <!-- Firestore CRUD dialog -->
    <FirestoreOpDialog
      v-if="activeConnection && selectedTable && engineKind === 'firestore'"
      v-model:open="firestoreOpDialogOpen"
      :op="firestoreOp"
      :connection-id="activeConnection.id"
      :collection="selectedTable.name"
      :initial-doc-id="firestoreInitialDocId"
      :initial-document="firestoreInitialDocument"
      @executed="onWriteExecuted"
    />

    <!-- RTDB CRUD dialog -->
    <RtdbOpDialog
      v-if="activeConnection && selectedTable && engineKind === 'rtdb'"
      v-model:open="rtdbOpDialogOpen"
      :op="rtdbOp"
      :connection-id="activeConnection.id"
      :node="selectedTable.name"
      :initial-key="rtdbInitialKey"
      :initial-value="rtdbInitialValue"
      @executed="onWriteExecuted"
    />

    <!-- Redis CRUD dialog -->
    <RedisOpDialog
      v-if="activeConnection && engineKind === 'redis'"
      v-model:open="redisOpDialogOpen"
      :op="redisOp"
      :connection-id="activeConnection.id"
      :initial-key="redisInitialKey"
      :initial-type="redisInitialType"
      :initial-value="redisInitialValue"
      :initial-ttl="redisInitialTtl"
      @executed="onWriteExecuted"
    />

    <!-- Schema-editing dialogs (SQL engines only) -->
    <CreateTableDialog
      v-if="activeConnection && engineKind === 'sql'"
      v-model:open="createTableOpen"
      :engine="activeConnection.db_type"
      :default-schema="selectedTable?.schema ?? null"
      @confirm="onCreateTableConfirm"
    />
    <AlterColumnDialog
      v-if="activeConnection && selectedTable && engineKind === 'sql'"
      v-model:open="alterColumnOpen"
      :mode="alterColumnMode"
      :engine="activeConnection.db_type"
      :table-name="selectedTable.name"
      :schema="selectedTable.schema ?? null"
      :column-name="alterColumnTarget?.name"
      :current-type="alterColumnTarget?.type"
      @confirm="onAlterColumnConfirm"
    />

    <ExportDialog
      v-model:open="exportOpen"
      :data="exportData"
      :columns="previewColumns"
    />

    <!-- NoSQL bulk-delete dialog (SQL bulk goes through SqlConfirmDialog). -->
    <BulkDeleteDialog
      v-if="activeConnection"
      v-model:open="bulkDeleteOpen"
      :engine="bulkDeleteEngine"
      :connection-id="activeConnection.id"
      :container="bulkDeleteContainer"
      :ids="bulkDeleteIds"
      @executed="onWriteExecuted"
    />
  </div>
</template>
