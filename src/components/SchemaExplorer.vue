<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const props = defineProps<{
  connectionString: string;
  engine: string;
}>();

const emit = defineEmits<{
  selectTable: [tableName: string, schema?: string];
  generateQuery: [query: string];
  close: [];
}>();

interface TableInfo {
  name: string;
  schema: string | null;
  table_type: string;
}

interface ColumnInfo {
  name: string;
  data_type: string;
  is_nullable: boolean;
  column_default: string | null;
  is_primary_key: boolean;
  is_foreign_key: boolean;
  referenced_table: string | null;
  referenced_column: string | null;
}

const tables = ref<TableInfo[]>([]);
const selectedTable = ref<TableInfo | null>(null);
const columns = ref<ColumnInfo[]>([]);
const previewData = ref<any[]>([]);
const loading = ref(false);
const columnsLoading = ref(false);
const previewLoading = ref(false);
const previewError = ref('');
const error = ref('');
const searchQuery = ref('');
const activeTab = ref<'columns' | 'preview' | 'sample'>('columns');
const sampleQueries = ref<string[]>([]);
const sampleLoading = ref(false);

const filteredTables = computed(() => {
  if (!searchQuery.value) return tables.value;
  const query = searchQuery.value.toLowerCase();
  return tables.value.filter(t => 
    t.name.toLowerCase().includes(query) ||
    (t.schema && t.schema.toLowerCase().includes(query))
  );
});

const groupedTables = computed(() => {
  const groups: Record<string, TableInfo[]> = {};
  for (const table of filteredTables.value) {
    const schema = table.schema || 'default';
    if (!groups[schema]) groups[schema] = [];
    groups[schema].push(table);
  }
  return groups;
});

async function loadTables() {
  loading.value = true;
  error.value = '';
  
  try {
    tables.value = await invoke<TableInfo[]>('get_tables', {
      engine: props.engine,
      connStr: props.connectionString,
    });
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
}

async function selectTableItem(table: TableInfo) {
  selectedTable.value = table;
  columnsLoading.value = true;
  columns.value = [];
  previewData.value = [];
  previewError.value = '';
  sampleQueries.value = [];
  
  try {
    columns.value = await invoke<ColumnInfo[]>('get_table_columns', {
      engine: props.engine,
      connStr: props.connectionString,
      tableName: table.name,
      schemaName: table.schema,
    });
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    columnsLoading.value = false;
  }
}

async function loadPreview() {
  if (!selectedTable.value) return;
  
  previewLoading.value = true;
  previewError.value = '';
  previewData.value = [];
  
  try {
    const result = await invoke<string>('preview_table_data', {
      engine: props.engine,
      connStr: props.connectionString,
      tableName: selectedTable.value.name,
      schemaName: selectedTable.value.schema,
      limit: 50,
    });
    
    const parsed = JSON.parse(result);
    previewData.value = Array.isArray(parsed) ? parsed : [];
  } catch (e) {
    previewError.value = e instanceof Error ? e.message : String(e);
    console.error('Preview data error:', e);
  } finally {
    previewLoading.value = false;
  }
}

async function loadSampleQueries() {
  if (!selectedTable.value || columns.value.length === 0) return;
  
  sampleLoading.value = true;
  
  try {
    const columnNames = columns.value.map(c => c.name);
    sampleQueries.value = await invoke<string[]>('generate_sample_queries', {
      tableName: selectedTable.value.name,
      columns: columnNames,
      dbType: props.engine,
    });
  } catch (e) {
    // Silently handle error - AI features are optional
    sampleQueries.value = [
      `SELECT * FROM ${selectedTable.value.name} LIMIT 10;`,
      `SELECT COUNT(*) FROM ${selectedTable.value.name};`,
    ];
  } finally {
    sampleLoading.value = false;
  }
}

function quoteIdentifier(name: string): string {
  // Use double quotes for PostgreSQL, backticks for MySQL
  if (props.engine === 'mysql') {
    return `\`${name}\``;
  }
  return `"${name}"`;
}

function getFullTableName(): string {
  if (!selectedTable.value) return '';
  const quoted = quoteIdentifier(selectedTable.value.name);
  if (selectedTable.value.schema) {
    return `${quoteIdentifier(selectedTable.value.schema)}.${quoted}`;
  }
  return quoted;
}

function generateSelectAll() {
  if (!selectedTable.value) return;
  emit('generateQuery', `SELECT * FROM ${getFullTableName()} LIMIT 100;`);
}

function generateSelectColumns() {
  if (!selectedTable.value || columns.value.length === 0) return;
  const cols = columns.value.map(c => quoteIdentifier(c.name)).join(', ');
  emit('generateQuery', `SELECT ${cols}\nFROM ${getFullTableName()}\nLIMIT 100;`);
}

function useSampleQuery(query: string) {
  emit('generateQuery', query);
}

function getTypeIcon(dataType: string): string {
  const type = dataType.toLowerCase();
  if (type.includes('int') || type.includes('numeric') || type.includes('decimal') || type.includes('float') || type.includes('double')) {
    return 'tag';
  }
  if (type.includes('char') || type.includes('text') || type.includes('varchar')) {
    return 'text_fields';
  }
  if (type.includes('date') || type.includes('time') || type.includes('timestamp')) {
    return 'schedule';
  }
  if (type.includes('bool')) {
    return 'toggle_on';
  }
  if (type.includes('json')) {
    return 'data_object';
  }
  return 'category';
}

watch(() => props.connectionString, () => {
  if (props.connectionString) {
    loadTables();
  }
}, { immediate: true });

watch(activeTab, (tab) => {
  if (tab === 'preview' && previewData.value.length === 0 && !previewLoading.value && selectedTable.value) {
    loadPreview();
  }
  if (tab === 'sample' && sampleQueries.value.length === 0 && !sampleLoading.value && selectedTable.value) {
    loadSampleQueries();
  }
});
</script>

<template>
  <div class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50" @click.self="emit('close')">
    <div class="bg-surface-dark border border-[#3d4f51] rounded-2xl w-full max-w-5xl h-[80vh] shadow-2xl overflow-hidden flex flex-col">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-4 border-b border-[#2a3637] shrink-0">
        <div class="flex items-center gap-3">
          <span class="material-symbols-outlined text-primary">schema</span>
          <h2 class="text-lg font-semibold text-white">Schema Explorer</h2>
          <span class="text-xs text-[#5d6f71] bg-[#2a3637] px-2 py-0.5 rounded">
            {{ tables.length }} tables
          </span>
        </div>
        <button
          @click="emit('close')"
          class="p-1.5 hover:bg-white/10 rounded-lg transition-colors"
        >
          <span class="material-symbols-outlined text-[#9fb4b7]">close</span>
        </button>
      </div>

      <div class="flex flex-1 overflow-hidden">
        <!-- Tables sidebar -->
        <div class="w-72 border-r border-[#2a3637] flex flex-col">
          <!-- Search -->
          <div class="p-4 border-b border-[#2a3637]">
            <div class="relative">
              <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-[#5d6f71] text-[18px]">search</span>
              <input
                v-model="searchQuery"
                type="text"
                placeholder="Search tables..."
                class="w-full pl-9 pr-4 py-2 bg-background-dark border border-[#3d4f51] rounded-lg text-white text-sm placeholder:text-[#5d6f71] focus:outline-none focus:border-primary"
              />
            </div>
          </div>

          <!-- Tables list -->
          <div class="flex-1 overflow-y-auto p-2">
            <div v-if="loading" class="flex items-center justify-center py-12">
              <div class="w-6 h-6 border-2 border-primary border-t-transparent rounded-full animate-spin"></div>
            </div>

            <div v-else-if="error" class="p-4 text-red-400 text-sm">
              {{ error }}
            </div>

            <div v-else>
              <div v-for="(schemaTables, schemaName) in groupedTables" :key="schemaName" class="mb-4">
                <div v-if="Object.keys(groupedTables).length > 1" class="px-3 py-1 text-[10px] uppercase tracking-widest text-[#5d6f71] font-bold">
                  {{ schemaName }}
                </div>
                <button
                  v-for="table in schemaTables"
                  :key="table.name"
                  @click="selectTableItem(table)"
                  class="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-left transition-colors"
                  :class="selectedTable?.name === table.name ? 'bg-primary/20 text-white' : 'text-[#9fb4b7] hover:bg-white/5'"
                >
                  <span class="material-symbols-outlined text-[18px]">
                    {{ table.table_type === 'VIEW' ? 'view_cozy' : 'table_chart' }}
                  </span>
                  <span class="text-sm truncate flex-1">{{ table.name }}</span>
                  <span v-if="table.table_type === 'VIEW'" class="text-[10px] bg-amber-500/20 text-amber-400 px-1 py-0.5 rounded">
                    VIEW
                  </span>
                </button>
              </div>
            </div>
          </div>
        </div>

        <!-- Details panel -->
        <div class="flex-1 flex flex-col overflow-hidden">
          <div v-if="!selectedTable" class="flex-1 flex items-center justify-center text-[#5d6f71]">
            <div class="text-center">
              <span class="material-symbols-outlined text-4xl mb-2">table_chart</span>
              <p>Select a table to view its schema</p>
            </div>
          </div>

          <template v-else>
            <!-- Table header -->
            <div class="px-6 py-4 border-b border-[#2a3637] shrink-0">
              <div class="flex items-center justify-between">
                <div>
                  <h3 class="text-white font-semibold">{{ selectedTable.name }}</h3>
                  <p class="text-xs text-[#5d6f71]">
                    {{ columns.length }} columns
                    <span v-if="selectedTable.schema"> · {{ selectedTable.schema }}</span>
                  </p>
                </div>
                <div class="flex gap-2">
                  <button
                    @click="generateSelectAll"
                    class="flex items-center gap-2 px-3 py-1.5 bg-primary/20 text-primary text-xs font-medium rounded-lg hover:bg-primary/30 transition-colors"
                  >
                    <span class="material-symbols-outlined text-[16px]">code</span>
                    SELECT *
                  </button>
                  <button
                    @click="generateSelectColumns"
                    class="flex items-center gap-2 px-3 py-1.5 bg-surface-dark border border-[#3d4f51] text-white text-xs font-medium rounded-lg hover:border-primary transition-colors"
                  >
                    <span class="material-symbols-outlined text-[16px]">checklist</span>
                    SELECT columns
                  </button>
                </div>
              </div>

              <!-- Tabs -->
              <div class="flex gap-1 mt-4">
                <button
                  @click="activeTab = 'columns'"
                  class="px-4 py-1.5 text-sm font-medium rounded-lg transition-colors"
                  :class="activeTab === 'columns' ? 'bg-primary/20 text-primary' : 'text-[#9fb4b7] hover:text-white'"
                >
                  Columns
                </button>
                <button
                  @click="activeTab = 'preview'"
                  class="px-4 py-1.5 text-sm font-medium rounded-lg transition-colors"
                  :class="activeTab === 'preview' ? 'bg-primary/20 text-primary' : 'text-[#9fb4b7] hover:text-white'"
                >
                  Preview Data
                </button>
                <button
                  @click="activeTab = 'sample'"
                  class="px-4 py-1.5 text-sm font-medium rounded-lg transition-colors flex items-center gap-1"
                  :class="activeTab === 'sample' ? 'bg-primary/20 text-primary' : 'text-[#9fb4b7] hover:text-white'"
                >
                  <span class="material-symbols-outlined text-[14px]">auto_awesome</span>
                  Sample Queries
                </button>
              </div>
            </div>

            <!-- Content -->
            <div class="flex-1 overflow-y-auto p-6">
              <!-- Columns tab -->
              <div v-if="activeTab === 'columns'">
                <div v-if="columnsLoading" class="flex items-center justify-center py-12">
                  <div class="w-6 h-6 border-2 border-primary border-t-transparent rounded-full animate-spin"></div>
                </div>

                <div v-else class="space-y-2">
                  <div
                    v-for="column in columns"
                    :key="column.name"
                    class="flex items-center gap-4 p-3 bg-background-dark rounded-lg border border-[#2a3637]"
                  >
                    <span 
                      class="material-symbols-outlined text-[18px]"
                      :class="column.is_primary_key ? 'text-amber-400' : column.is_foreign_key ? 'text-blue-400' : 'text-[#5d6f71]'"
                    >
                      {{ column.is_primary_key ? 'key' : column.is_foreign_key ? 'link' : getTypeIcon(column.data_type) }}
                    </span>
                    
                    <div class="flex-1">
                      <div class="flex items-center gap-2">
                        <span class="text-white font-medium text-sm">{{ column.name }}</span>
                        <span v-if="column.is_primary_key" class="text-[10px] bg-amber-500/20 text-amber-400 px-1.5 py-0.5 rounded font-medium">PK</span>
                        <span v-if="column.is_foreign_key" class="text-[10px] bg-blue-500/20 text-blue-400 px-1.5 py-0.5 rounded font-medium">FK</span>
                        <span v-if="!column.is_nullable" class="text-[10px] bg-red-500/20 text-red-400 px-1.5 py-0.5 rounded font-medium">NOT NULL</span>
                      </div>
                      <div class="flex items-center gap-2 mt-0.5">
                        <span class="text-xs text-[#9fb4b7]">{{ column.data_type }}</span>
                        <span v-if="column.column_default" class="text-xs text-[#5d6f71]">= {{ column.column_default }}</span>
                        <span v-if="column.referenced_table" class="text-xs text-blue-400">
                          → {{ column.referenced_table }}.{{ column.referenced_column }}
                        </span>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              <!-- Preview tab -->
              <div v-else-if="activeTab === 'preview'">
                <div v-if="previewLoading" class="flex items-center justify-center py-12">
                  <div class="w-6 h-6 border-2 border-primary border-t-transparent rounded-full animate-spin"></div>
                </div>

                <div v-else-if="previewError" class="text-center py-12">
                  <span class="material-symbols-outlined text-3xl text-red-400 mb-2">error</span>
                  <p class="text-red-400 text-sm mb-3">{{ previewError }}</p>
                  <button @click="loadPreview" class="px-4 py-2 bg-primary/20 text-primary rounded-lg text-sm hover:bg-primary/30 transition-colors">
                    Retry
                  </button>
                </div>

                <div v-else-if="previewData.length === 0" class="text-center text-[#5d6f71] py-12">
                  <p class="mb-3">No data loaded yet</p>
                  <button @click="loadPreview" class="px-4 py-2 bg-primary/20 text-primary rounded-lg text-sm hover:bg-primary/30 transition-colors">
                    Load Preview
                  </button>
                </div>

                <div v-else>
                  <div class="flex items-center justify-between mb-3">
                    <span class="text-xs text-[#5d6f71]">{{ previewData.length }} rows loaded</span>
                    <button
                      @click="loadPreview"
                      class="flex items-center gap-1 text-xs text-primary hover:text-primary/80 transition-colors"
                    >
                      <span class="material-symbols-outlined text-[14px]">refresh</span>
                      Refresh
                    </button>
                  </div>
                  <div class="overflow-x-auto border border-[#2a3637] rounded-lg">
                    <table class="w-full text-sm">
                      <thead>
                        <tr class="bg-background-dark border-b border-[#2a3637]">
                          <th v-for="key in Object.keys(previewData[0])" :key="key" class="text-left py-2.5 px-3 text-[#9fb4b7] font-medium text-xs uppercase tracking-wider whitespace-nowrap">
                            {{ key }}
                          </th>
                        </tr>
                      </thead>
                      <tbody>
                        <tr v-for="(row, idx) in previewData.slice(0, 50)" :key="idx" class="border-b border-[#2a3637]/50 hover:bg-white/5">
                          <td v-for="key in Object.keys(previewData[0])" :key="key" class="py-2 px-3 text-white whitespace-nowrap">
                            <span v-if="row[key] === null || row[key] === undefined" class="text-[#5d6f71] italic">NULL</span>
                            <span v-else class="truncate max-w-[250px] inline-block">{{ row[key] }}</span>
                          </td>
                        </tr>
                      </tbody>
                    </table>
                  </div>
                  <p v-if="previewData.length > 50" class="text-xs text-[#5d6f71] mt-3 text-center">
                    Showing 50 of {{ previewData.length }} rows
                  </p>
                </div>
              </div>

              <!-- Sample queries tab -->
              <div v-else-if="activeTab === 'sample'">
                <div v-if="sampleLoading" class="flex items-center justify-center py-12">
                  <div class="w-6 h-6 border-2 border-primary border-t-transparent rounded-full animate-spin"></div>
                </div>

                <div v-else class="space-y-3">
                  <div
                    v-for="(query, idx) in sampleQueries"
                    :key="idx"
                    class="p-4 bg-background-dark rounded-lg border border-[#2a3637] group"
                  >
                    <pre class="text-sm text-[#9fb4b7] font-mono whitespace-pre-wrap mb-3">{{ query }}</pre>
                    <button
                      @click="useSampleQuery(query)"
                      class="flex items-center gap-2 text-xs text-primary hover:text-primary/80 transition-colors opacity-0 group-hover:opacity-100"
                    >
                      <span class="material-symbols-outlined text-[14px]">play_arrow</span>
                      Use this query
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>
