<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Badge } from '~/components/ui/badge'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Separator } from '~/components/ui/separator'
import { useConnectionsStore } from '~/stores/connections'
import type { ColumnInfo } from '~/types/database'

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

const selectedTable = computed(() =>
  tables.value.find(t => t.name === selectedTableName.value) ?? null,
)

const selectedColumns = computed((): ColumnInfo[] =>
  (selectedTableName.value && tableColumns.value[selectedTableName.value]) || [],
)

const previewColumns = computed(() =>
  previewData.value.length > 0 ? Object.keys(previewData.value[0]) : [],
)

watch(selectedTableName, async (name) => {
  if (!name) return
  previewData.value = []
  activeTab.value = 'columns'
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
            <div class="flex items-center gap-2">
              <Icon name="lucide:table-2" class="size-4 text-muted-foreground" />
              <span class="font-medium text-sm">{{ selectedTable.name }}</span>
              <Badge v-if="selectedTable.schema" variant="outline" class="text-xs">{{ selectedTable.schema }}</Badge>
              <Badge variant="secondary" class="text-xs">{{ selectedTable.table_type }}</Badge>
            </div>
            <div class="flex items-center gap-1.5 text-xs text-muted-foreground">
              <span>{{ selectedColumns.length }} columns</span>
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
          <div v-if="activeTab === 'columns'" class="overflow-auto flex-1">
            <table class="w-full text-sm">
              <thead class="sticky top-0 bg-muted/80 backdrop-blur-sm">
                <tr class="border-b border-border">
                  <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">Column</th>
                  <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">Type</th>
                  <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">Nullable</th>
                  <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">Keys</th>
                  <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">Default</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-border">
                <tr v-for="col in selectedColumns" :key="col.name" class="hover:bg-muted/20 transition-colors">
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
                </tr>
                <tr v-if="selectedColumns.length === 0">
                  <td colspan="5" class="px-4 py-8 text-center text-xs text-muted-foreground">
                    No column data available
                  </td>
                </tr>
              </tbody>
            </table>
          </div>

          <!-- Preview tab -->
          <div v-else-if="activeTab === 'preview'" class="overflow-auto flex-1">
            <div v-if="isLoadingPreview" class="flex flex-col gap-1.5 p-4">
              <div v-for="i in 5" :key="i" class="h-8 bg-muted/50 rounded animate-pulse" />
            </div>
            <table v-else-if="previewData.length > 0" class="w-full text-sm">
              <thead class="sticky top-0 bg-muted/80 backdrop-blur-sm">
                <tr class="border-b border-border">
                  <th
                    v-for="col in previewColumns"
                    :key="col"
                    class="px-4 py-2 text-left text-xs font-medium text-muted-foreground whitespace-nowrap"
                  >
                    {{ col }}
                  </th>
                </tr>
              </thead>
              <tbody class="divide-y divide-border">
                <tr v-for="(row, i) in previewData" :key="i" class="hover:bg-muted/20 transition-colors">
                  <td
                    v-for="col in previewColumns"
                    :key="col"
                    class="px-4 py-1.5 text-xs font-mono text-foreground max-w-[200px] truncate"
                  >
                    <span v-if="row[col] === null" class="text-muted-foreground italic">null</span>
                    <span v-else>{{ formatCellValue(row[col]) }}</span>
                  </td>
                </tr>
              </tbody>
            </table>
            <div v-else class="flex flex-col items-center justify-center flex-1 py-12 text-muted-foreground">
              <Icon name="lucide:table" class="size-8 mb-3" />
              <p class="text-sm">No preview data</p>
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
  </div>
</template>
