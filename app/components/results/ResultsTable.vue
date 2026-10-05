<script setup lang="ts">
import { useVueTable, getCoreRowModel, getSortedRowModel, type Row } from '@tanstack/vue-table'
import { useElementSize } from '@vueuse/core'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '~/components/ui/dialog'
import { cellText, compareCells, columnKind, filterError, filterRows, type ColumnFilter } from '~/lib/result-grid'
import type { QueryResult } from '~/types/query'

const props = defineProps<{ result: QueryResult | null; isLoading?: boolean }>()
const emit = defineEmits<{ 'load-page': [page: number] }>()
const search = ref('')
const filters = ref<ColumnFilter[]>([])
const rows = computed(() => filterRows(props.result?.rows ?? [], search.value, filters.value))
const columns = computed(() => (props.result?.columns ?? []).map((column, index) => ({
  id: `c${index}`, header: column, sortingFn: (left: Row<Record<string, unknown>>, right: Row<Record<string, unknown>>, id: string) => compareCells(left.getValue(id), right.getValue(id)), accessorFn: (row: Record<string, unknown>) => row[column],
})))
const table = useVueTable({
  data: rows, get columns() { return columns.value }, getCoreRowModel: getCoreRowModel(), getSortedRowModel: getSortedRowModel(),
  columnResizeMode: 'onChange', defaultColumn: { size: 180, minSize: 80, maxSize: 640 },
})
const sortedRows = computed(() => table.getRowModel().rows)
const viewport = ref<HTMLElement | null>(null)
const { height } = useElementSize(viewport)
const scrollTop = ref(0)
const virtual = computed(() => sortedRows.value.length > 200)
// Fixed 40px rows keep the window predictable; small paginated results render in full.
const start = computed(() => virtual.value ? Math.max(0, Math.floor((scrollTop.value - 40) / 40) - 8) : 0)
const end = computed(() => virtual.value ? Math.min(sortedRows.value.length, start.value + Math.ceil(height.value / 40) + 17) : sortedRows.value.length)
const visibleRows = computed(() => sortedRows.value.slice(start.value, end.value))
const inspected = ref<{ column: string; row: number; value: unknown } | null>(null)
const copyStatus = ref('')
const kinds = computed(() => new Map((props.result?.columns ?? []).map(column => [column, columnKind(props.result?.rows ?? [], column)])))
const errors = computed(() => filters.value.map(filter => filterError(filter, kinds.value.get(filter.column) ?? 'text')))
watch(() => props.result, () => { search.value = ''; filters.value = []; inspected.value = null; table.resetSorting(); resetScroll() })
watch(() => JSON.stringify(props.result?.columns), () => table.resetColumnSizing())
watch(rows, resetScroll)
watch(() => table.getState().sorting, resetScroll)
watch(inspected, () => { copyStatus.value = '' })
function resetScroll() { scrollTop.value = 0; if (viewport.value) viewport.value.scrollTop = 0 }
function addFilter() { const column = props.result?.columns[0]; if (column !== undefined) filters.value.push({ column, operator: 'equals', value: '' }) }
function resizeKey(event: KeyboardEvent, id: string, size: number) {
  if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return
  event.preventDefault()
  const next = event.key === 'Home' ? 80 : event.key === 'End' ? 640 : size + (event.key === 'ArrowRight' ? 16 : -16)
  table.setColumnSizing(previous => ({ ...previous, [id]: Math.min(640, Math.max(80, next)) }))
}
async function copyCell() {
  if (!inspected.value) return
  try { await navigator.clipboard.writeText(cellText(inspected.value.value, true)); copyStatus.value = 'Copied.' }
  catch { copyStatus.value = 'Could not copy. Select the value and copy it manually.' }
}
</script>

<template>
  <div class="flex flex-col gap-2 h-full min-h-0">
    <template v-if="result">
      <div class="flex flex-wrap items-center gap-2 shrink-0">
        <Input v-model="search" aria-label="Search loaded rows" placeholder="Search loaded rows" class="h-8 max-w-xs" />
        <Button variant="ghost" size="sm" @click="addFilter">Add column filter</Button>
        <span class="text-xs text-muted-foreground">{{ rows.length.toLocaleString() }} of {{ result.rows.length.toLocaleString() }} loaded rows · {{ result.execution_time_ms }}ms</span>
        <slot name="actions" />
      </div>
      <p class="text-xs text-muted-foreground">Search, filters, and sorting apply to loaded rows. Click a cell to inspect its full value.</p>
      <div v-if="filters.length" class="space-y-2 shrink-0 max-h-40 overflow-auto">
        <div v-for="(filter, index) in filters" :key="index">
          <div class="flex flex-wrap items-center gap-2">
            <select v-model="filter.column" :aria-label="`Filter ${index + 1} column`" class="h-8 rounded-md bg-background border border-input px-2" @change="filter.operator = 'equals'; filter.value = ''">
              <option v-for="column in result.columns" :key="column" :value="column">{{ column }}</option>
            </select>
            <select v-model="filter.operator" :aria-label="`Filter ${index + 1} operator`" class="h-8 rounded-md bg-background border border-input px-2">
              <option value="equals">Equals</option><option value="contains">Contains</option>
              <option v-if="kinds.get(filter.column) === 'number'" value="greater">Greater than</option>
              <option v-if="kinds.get(filter.column) === 'number'" value="less">Less than</option>
              <option value="is_null">Is null</option><option value="not_null">Is not null</option>
            </select>
            <template v-if="!['is_null', 'not_null'].includes(filter.operator)">
              <select v-if="filter.operator === 'equals' && kinds.get(filter.column) === 'boolean'" v-model="filter.value" :aria-label="`Filter ${index + 1} value`" class="h-8 rounded-md bg-background border border-input px-2">
                <option value="" disabled>Choose value</option><option value="true">true</option><option value="false">false</option>
              </select>
              <Input v-else v-model="filter.value" :aria-label="`Filter ${index + 1} value`" :aria-invalid="!!errors[index]" class="h-8 w-44" />
            </template>
            <Button variant="ghost" size="sm" :aria-label="`Remove filter ${index + 1}`" @click="filters.splice(index, 1)">Remove</Button>
          </div>
          <p v-if="errors[index]" role="alert" class="text-xs text-destructive mt-1">{{ errors[index] }}</p>
        </div>
      </div>
    </template>
    <div v-if="isLoading && !result" role="status" class="p-4">Loading results…</div>
    <div v-else-if="result" class="flex flex-col flex-1 min-h-0 rounded-md border border-border overflow-hidden">
      <div ref="viewport" tabindex="0" aria-label="Query results. Scroll to view more rows." class="overflow-auto flex-1 min-h-0" @scroll="scrollTop = ($event.target as HTMLElement).scrollTop">
        <table v-if="rows.length" class="text-sm table-fixed" :style="{ width: `${table.getTotalSize()}px` }" :aria-rowcount="rows.length + 1">
          <colgroup><col v-for="column in table.getAllLeafColumns()" :key="column.id" :style="{ width: `${column.getSize()}px` }"></colgroup>
          <thead class="sticky top-0 bg-muted z-10">
            <tr v-for="group in table.getHeaderGroups()" :key="group.id" class="h-10">
              <th v-for="header in group.headers" :key="header.id" class="relative text-left px-3 font-medium" :aria-sort="header.column.getIsSorted() === 'asc' ? 'ascending' : header.column.getIsSorted() === 'desc' ? 'descending' : 'none'">
                <button class="block w-full truncate text-left py-2 focus-visible:outline focus-visible:outline-ring" :title="String(header.column.columnDef.header)" @click="header.column.toggleSorting()">{{ header.column.columnDef.header }}{{ header.column.getIsSorted() === 'asc' ? ' ↑' : header.column.getIsSorted() === 'desc' ? ' ↓' : '' }}</button>
                <span role="separator" tabindex="0" aria-orientation="vertical" :aria-label="`Resize ${header.column.columnDef.header}`" :aria-valuemin="80" :aria-valuemax="640" :aria-valuenow="header.getSize()" class="absolute right-0 top-0 bottom-0 w-2 cursor-col-resize border-r border-border hover:bg-muted-foreground/20 focus-visible:bg-muted-foreground/20" @mousedown="header.getResizeHandler()($event)" @touchstart="header.getResizeHandler()($event)" @keydown="resizeKey($event, header.column.id, header.getSize())" />
              </th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="start" aria-hidden="true"><td :colspan="columns.length" :style="{ height: `${start * 40}px`, padding: 0 }" /></tr>
            <tr v-for="(row, index) in visibleRows" :key="row.id" :aria-rowindex="start + index + 2" class="h-10 hover:bg-muted/30">
              <td v-for="cell in row.getVisibleCells()" :key="cell.id" class="h-10 px-3 py-0">
                <button class="block w-full truncate text-left py-2 focus-visible:outline focus-visible:outline-ring" :aria-label="`Inspect row ${start + index + 1}, ${cell.column.columnDef.header}: ${cell.getValue() === undefined ? '(missing)' : cellText(cell.getValue()).slice(0, 160)}`" @click="inspected = { column: String(cell.column.columnDef.header), row: start + index + 1, value: cell.getValue() }">{{ cell.getValue() === undefined ? '(missing)' : cellText(cell.getValue()) }}</button>
              </td>
            </tr>
            <tr v-if="end < sortedRows.length" aria-hidden="true"><td :colspan="columns.length" :style="{ height: `${(sortedRows.length - end) * 40}px`, padding: 0 }" /></tr>
          </tbody>
        </table>
        <p v-else class="p-6 text-sm text-muted-foreground">{{ result.rows.length ? 'No loaded rows match your filters.' : 'Query returned no results.' }}</p>
      </div>
      <div v-if="result.has_more || result.page > 1" class="flex flex-wrap items-center gap-3 p-2 shrink-0 text-sm">
        <span>Page {{ result.page }}<template v-if="result.total_count !== null"> · {{ result.total_count.toLocaleString() }} total rows</template></span>
        <Button variant="ghost" size="sm" :disabled="isLoading || result.page <= 1" @click="emit('load-page', result.page - 1)">Previous page</Button>
        <Button variant="ghost" size="sm" :disabled="isLoading || !result.has_more" @click="emit('load-page', result.page + 1)">Next page</Button>
      </div>
    </div>
    <Dialog :open="!!inspected" @update:open="open => { if (!open) inspected = null }">
      <DialogContent class="sm:max-w-2xl max-h-[90dvh] overflow-y-auto">
        <DialogHeader><DialogTitle class="break-all pr-6">{{ inspected?.column }}</DialogTitle><DialogDescription>Row {{ inspected?.row }} · {{ inspected?.value === null ? 'null' : inspected?.value === undefined ? 'missing value' : typeof inspected?.value }}</DialogDescription></DialogHeader>
        <pre class="max-h-[60vh] overflow-auto whitespace-pre-wrap break-all text-sm p-3 bg-muted rounded-md">{{ inspected?.value === undefined ? '(missing)' : cellText(inspected?.value, true) }}</pre>
        <Button variant="ghost" @click="copyCell">Copy value</Button><p v-if="copyStatus" role="status" class="text-sm">{{ copyStatus }}</p>
      </DialogContent>
    </Dialog>
  </div>
</template>
