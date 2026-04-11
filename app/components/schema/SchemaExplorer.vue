<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Input } from '~/components/ui/input'
import { Badge } from '~/components/ui/badge'
import { Button } from '~/components/ui/button'
import type { TableInfo, ColumnInfo } from '~/types/database'

const props = defineProps<{
  engine: string
  connStr: string
}>()

const emit = defineEmits<{
  'generate-query': [query: string]
}>()

const tables = ref<TableInfo[]>([])
const columns = ref<ColumnInfo[]>([])
const previewData = ref<Record<string, unknown>[]>([])
const selectedTable = ref<TableInfo | null>(null)
const activeTab = ref<'columns' | 'preview'>('columns')
const isLoadingTables = ref(false)
const isLoadingColumns = ref(false)
const isLoadingPreview = ref(false)
const searchTerm = ref('')

const filteredTables = computed(() => {
  if (!searchTerm.value) return tables.value
  const term = searchTerm.value.toLowerCase()
  return tables.value.filter(t =>
    t.name.toLowerCase().includes(term) ||
    t.schema?.toLowerCase().includes(term),
  )
})

const previewColumns = computed(() =>
  previewData.value.length > 0 ? Object.keys(previewData.value[0]) : [],
)

watch([() => props.engine, () => props.connStr], () => {
  if (props.engine && props.connStr) {
    loadTables()
  }
}, { immediate: true })

watch(selectedTable, async (table) => {
  if (!table) return
  columns.value = []
  previewData.value = []
  await loadColumns(table)
})

watch(activeTab, async (tab) => {
  if (tab === 'preview' && selectedTable.value && previewData.value.length === 0) {
    await loadPreview(selectedTable.value)
  }
})

async function loadTables() {
  if (!props.engine || !props.connStr) return
  isLoadingTables.value = true

  try {
    tables.value = await invoke<TableInfo[]>('get_tables', {
      engine: props.engine,
      connStr: props.connStr,
    })
  }
  catch (err) {
    toast.error('Failed to load tables', { description: err as string })
  }
  finally {
    isLoadingTables.value = false
  }
}

async function loadColumns(table: TableInfo) {
  isLoadingColumns.value = true

  try {
    columns.value = await invoke<ColumnInfo[]>('get_table_columns', {
      engine: props.engine,
      connStr: props.connStr,
      tableName: table.name,
      schemaName: table.schema,
    })
  }
  catch (err) {
    toast.error('Failed to load columns', { description: err as string })
  }
  finally {
    isLoadingColumns.value = false
  }
}

async function loadPreview(table: TableInfo) {
  isLoadingPreview.value = true

  try {
    const data = await invoke<string>('preview_table_data', {
      engine: props.engine,
      connStr: props.connStr,
      tableName: table.name,
      schemaName: table.schema,
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

function generateSelectQuery(table: TableInfo) {
  const q = props.engine === 'postgres'
    ? `SELECT * FROM "${table.schema}"."${table.name}" LIMIT 100`
    : `SELECT * FROM \`${table.name}\` LIMIT 100`
  emit('generate-query', q)
}

function formatCellValue(val: unknown): string {
  if (val === null || val === undefined) return ''
  if (typeof val === 'object') return JSON.stringify(val)
  return String(val)
}
</script>

<template>
  <div class="flex h-full gap-0 overflow-hidden rounded-md border border-border">
    <!-- Table list -->
    <div class="w-52 flex flex-col border-r border-border shrink-0">
      <div class="p-2 border-b border-border">
        <div class="relative">
          <Icon name="lucide:search" class="absolute left-2 top-2 size-3.5 text-muted-foreground" />
          <Input v-model="searchTerm" placeholder="Search tables..." class="pl-7 h-7 text-xs" />
        </div>
      </div>
      <div class="overflow-y-auto flex-1 p-1">
        <div v-if="isLoadingTables" class="flex flex-col gap-1 p-1">
          <div v-for="i in 6" :key="i" class="h-7 bg-muted/50 rounded animate-pulse" />
        </div>
        <button
          v-for="table in filteredTables"
          :key="`${table.schema}-${table.name}`"
          class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm transition-colors hover:bg-accent text-left"
          :class="selectedTable?.name === table.name ? 'bg-accent text-foreground' : 'text-muted-foreground'"
          @click="selectedTable = table; activeTab = 'columns'"
        >
          <Icon
            :name="table.table_type === 'VIEW' ? 'lucide:eye' : 'lucide:table-2'"
            class="size-3.5 shrink-0"
          />
          <span class="truncate">{{ table.name }}</span>
        </button>
        <p v-if="!isLoadingTables && filteredTables.length === 0" class="text-xs text-muted-foreground p-2 text-center">
          No tables found
        </p>
      </div>
    </div>

    <!-- Detail panel -->
    <div class="flex flex-1 flex-col overflow-hidden">
      <template v-if="selectedTable">
        <!-- Table header -->
        <div class="flex items-center justify-between px-4 py-2 border-b border-border">
          <div class="flex items-center gap-2">
            <Icon name="lucide:table-2" class="size-4 text-muted-foreground" />
            <span class="font-medium text-sm">{{ selectedTable.name }}</span>
            <Badge v-if="selectedTable.schema" variant="outline" class="text-xs">{{ selectedTable.schema }}</Badge>
          </div>
          <Button size="sm" variant="outline" class="h-7 text-xs gap-1" @click="generateSelectQuery(selectedTable)">
            <Icon name="lucide:play" class="size-3.5" />
            Query
          </Button>
        </div>

        <!-- Tabs -->
        <div class="flex border-b border-border px-4">
          <button
            v-for="tab in ['columns', 'preview'] as const"
            :key="tab"
            class="px-3 py-1.5 text-sm border-b-2 transition-colors capitalize"
            :class="activeTab === tab
              ? 'border-primary text-foreground'
              : 'border-transparent text-muted-foreground hover:text-foreground'"
            @click="activeTab = tab"
          >
            {{ tab }}
          </button>
        </div>

        <!-- Columns tab -->
        <div v-if="activeTab === 'columns'" class="overflow-auto flex-1">
          <div v-if="isLoadingColumns" class="flex flex-col gap-1 p-3">
            <div v-for="i in 5" :key="i" class="h-8 bg-muted/50 rounded animate-pulse" />
          </div>
          <table v-else class="w-full text-sm">
            <thead class="sticky top-0 bg-muted/80 backdrop-blur-sm">
              <tr class="border-b border-border">
                <th class="px-3 py-2 text-left text-xs text-muted-foreground font-medium">Column</th>
                <th class="px-3 py-2 text-left text-xs text-muted-foreground font-medium">Type</th>
                <th class="px-3 py-2 text-left text-xs text-muted-foreground font-medium">Nullable</th>
                <th class="px-3 py-2 text-left text-xs text-muted-foreground font-medium">Keys</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border">
              <tr v-for="col in columns" :key="col.name" class="hover:bg-muted/30">
                <td class="px-3 py-2 font-mono text-xs text-foreground">{{ col.name }}</td>
                <td class="px-3 py-2 font-mono text-xs text-muted-foreground">{{ col.data_type }}</td>
                <td class="px-3 py-2 text-xs text-muted-foreground">{{ col.is_nullable ? 'YES' : 'NO' }}</td>
                <td class="px-3 py-2">
                  <div class="flex gap-1">
                    <Badge v-if="col.is_primary_key" variant="default" class="text-xs py-0">PK</Badge>
                    <Badge v-if="col.is_foreign_key" variant="secondary" class="text-xs py-0">FK</Badge>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <!-- Preview tab -->
        <div v-else-if="activeTab === 'preview'" class="overflow-auto flex-1">
          <div v-if="isLoadingPreview" class="flex flex-col gap-1 p-3">
            <div v-for="i in 5" :key="i" class="h-8 bg-muted/50 rounded animate-pulse" />
          </div>
          <table v-else class="w-full text-sm">
            <thead class="sticky top-0 bg-muted/80 backdrop-blur-sm">
              <tr class="border-b border-border">
                <th v-for="col in previewColumns" :key="col" class="px-3 py-2 text-left text-xs text-muted-foreground font-medium whitespace-nowrap">
                  {{ col }}
                </th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border">
              <tr v-for="(row, i) in previewData" :key="i" class="hover:bg-muted/30">
                <td v-for="col in previewColumns" :key="col" class="px-3 py-2 text-xs font-mono text-foreground max-w-32 truncate">
                  {{ formatCellValue(row[col]) }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </template>

      <!-- No table selected -->
      <div v-else class="flex flex-1 items-center justify-center text-muted-foreground">
        <div class="text-center">
          <Icon name="lucide:arrow-left" class="size-6 mx-auto mb-2" />
          <p class="text-sm">Select a table to explore</p>
        </div>
      </div>
    </div>
  </div>
</template>
