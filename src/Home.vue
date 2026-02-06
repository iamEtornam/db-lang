<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useConnectionsStore } from "./stores/connections";
import { useHistoryStore } from "./stores/history";
import { useSnippetsStore } from "./stores/snippets";
import ConnectionModal from "./components/ConnectionModal.vue";
import QueryHistory from "./components/QueryHistory.vue";
import SnippetManager from "./components/SnippetManager.vue";
import ExportModal from "./components/ExportModal.vue";
import SettingsModal from "./components/SettingsModal.vue";
import SchemaExplorer from "./components/SchemaExplorer.vue";
import QueryExplanation from "./components/QueryExplanation.vue";
import PaginationControls from "./components/PaginationControls.vue";
import DataInsights from "./components/DataInsights.vue";
import FilterPanel, { type Filter, type ColumnInfo } from "./components/FilterPanel.vue";

const connectionsStore = useConnectionsStore();
const historyStore = useHistoryStore();
const snippetsStore = useSnippetsStore();

const query = ref("");
const sql = ref("");
const results = ref("");
const error = ref("");
const isLoading = ref(false);
const showConfirmation = ref(false);
const showConnectionModal = ref(false);
const showHistoryModal = ref(false);
const showSnippetsModal = ref(false);
const showExportModal = ref(false);
const showSettingsModal = ref(false);
const showSchemaExplorer = ref(false);
const showQueryExplanation = ref(false);
const showDataInsights = ref(false);
const showFilterPanel = ref(false);
const activeFilters = ref<Filter[]>([]);
const executionStartTime = ref<number | null>(null);

// Pagination state
const currentPage = ref(1);
const pageSize = ref(50);
const totalCount = ref<number | null>(null);
const hasMore = ref(false);
const isPaginated = ref(false);

// Load data on mount
onMounted(async () => {
  await connectionsStore.loadConnections();
});

function autoQuoteIdentifiers(sqlQuery: string): string {
  // Auto-quote table names after FROM, JOIN, UPDATE, INTO, etc. if not already quoted
  // This helps with reserved words like User, Order, Group, etc.
  
  // Pattern: FROM/JOIN/INTO/UPDATE followed by optional schema and table name
  return sqlQuery
    .replace(/\b(FROM|JOIN|INTO|UPDATE)\s+(\w+)\.(\w+)\b/gi, (match, keyword, schema, table) => {
      // Already quoted? Leave it
      if (match.includes('"') || match.includes('`')) return match;
      return `${keyword} "${schema}"."${table}"`;
    })
    .replace(/\b(FROM|JOIN|INTO|UPDATE)\s+(\w+)\b/gi, (match, keyword, identifier) => {
      // Skip keywords like SELECT, WHERE, VALUES, etc.
      const skipWords = ['SELECT', 'WHERE', 'VALUES', 'SET', 'ON', 'USING', 'AS'];
      if (skipWords.includes(identifier.toUpperCase())) return match;
      // Already quoted? Leave it
      if (match.includes('"') || match.includes('`')) return match;
      return `${keyword} "${identifier}"`;
    });
}

async function translateQuery() {
  if (!query.value.trim()) return;
  isLoading.value = true;
  sql.value = "";
  results.value = "";
  error.value = "";
  showConfirmation.value = false;

  try {
    const translatedSql = await invoke("translate_to_sql", {
      query: query.value,
    });
    // Auto-quote identifiers to handle reserved words
    const quotedSql = autoQuoteIdentifiers((translatedSql as string).trim());
    sql.value = quotedSql;
    showConfirmation.value = true;
  } catch (err) {
    error.value = err as string;
    sql.value = "";
  } finally {
    isLoading.value = false;
  }
}

async function executeQuery(usePagination = true, page = 1) {
  if (!connectionsStore.activeConnection) {
    error.value = "No active connection. Please select or create a connection.";
    return;
  }

  isLoading.value = true;
  results.value = "";
  error.value = "";
  executionStartTime.value = Date.now();
  currentPage.value = page;

  try {
    const conn = connectionsStore.activeConnection;
    const connStr = connectionsStore.buildConnectionString(conn);
    
    if (usePagination && sql.value.trim().toUpperCase().startsWith('SELECT')) {
      // Use paginated query
      interface PaginatedResult {
        data: string;
        total_count: number | null;
        page: number;
        page_size: number;
        has_more: boolean;
      }
      
      const response = await invoke<PaginatedResult>("query_db_paginated", {
        engine: conn.db_type,
        connStr: connStr,
        query: sql.value,
        page: page,
        pageSize: pageSize.value,
      });
      
      results.value = response.data;
      totalCount.value = response.total_count;
      hasMore.value = response.has_more;
      isPaginated.value = true;
    } else {
      // Regular query for non-SELECT statements
      const response = await invoke("query_db", {
        engine: conn.db_type,
        connStr: connStr,
        query: sql.value,
      });
      results.value = response as string;
      totalCount.value = null;
      hasMore.value = false;
      isPaginated.value = false;
    }
    
    // Save to history
    const executionTime = Date.now() - executionStartTime.value!;
    if (page === 1) {
      await historyStore.addToHistory({
        connection_id: conn.id,
        natural_query: query.value,
        sql_query: sql.value,
        result_count: totalCount.value || parsedResults.value.length,
        execution_time_ms: executionTime,
        status: 'success',
      });
    }
  } catch (err) {
    error.value = err as string;
    
    // Save error to history
    const executionTime = executionStartTime.value ? Date.now() - executionStartTime.value : null;
    if (connectionsStore.activeConnection && page === 1) {
      await historyStore.addToHistory({
        connection_id: connectionsStore.activeConnection.id,
        natural_query: query.value,
        sql_query: sql.value,
        result_count: null,
        execution_time_ms: executionTime,
        status: 'error',
        error_message: err as string,
      });
    }
  } finally {
    isLoading.value = false;
    executionStartTime.value = null;
  }
}

function handlePageChange(page: number) {
  executeQuery(true, page);
}

function handlePageSizeChange(size: number) {
  pageSize.value = size;
  currentPage.value = 1;
  executeQuery(true, 1);
}

function handleSchemaGenerateQuery(generatedSql: string) {
  sql.value = generatedSql;
  showSchemaExplorer.value = false;
  showConfirmation.value = true;
}

function openExplainQuery() {
  if (sql.value.trim()) {
    showQueryExplanation.value = true;
  }
}

function copyToClipboard(text: string) {
  navigator.clipboard.writeText(text);
}

const parsedResults = computed(() => {
  if (!results.value) return [];
  try {
    const data = JSON.parse(results.value);
    return Array.isArray(data) ? data : [];
  } catch (e) {
    return [];
  }
});

const columns = computed(() => {
  if (parsedResults.value.length === 0) {
    return [];
  }
  return Object.keys(parsedResults.value[0]);
});

// Column info with type detection for filtering
const columnsWithTypes = computed<ColumnInfo[]>(() => {
  if (parsedResults.value.length === 0) return [];
  
  return columns.value.map(name => {
    // Sample values to detect type
    const sampleValues = parsedResults.value
      .slice(0, 100)
      .map(row => row[name])
      .filter(v => v !== null && v !== undefined && v !== '');
    
    let type: ColumnInfo['type'] = 'mixed';
    
    if (sampleValues.length > 0) {
      const firstValue = sampleValues[0];
      
      if (typeof firstValue === 'boolean' || sampleValues.every(v => v === true || v === false || v === 'true' || v === 'false')) {
        type = 'boolean';
      } else if (typeof firstValue === 'number' || sampleValues.every(v => !isNaN(Number(v)))) {
        type = 'number';
      } else if (isDateString(String(firstValue))) {
        type = 'date';
      } else {
        type = 'string';
      }
    }
    
    return { name, type };
  });
});

// Check if a string looks like a date
function isDateString(str: string): boolean {
  const datePatterns = [
    /^\d{4}-\d{2}-\d{2}/, // ISO
    /^\d{2}\/\d{2}\/\d{4}/, // US
    /^\d{2}-\d{2}-\d{4}/, // EU
  ];
  return datePatterns.some(p => p.test(str));
}

// Apply filters to parsed results
const filteredResults = computed(() => {
  if (activeFilters.value.length === 0) {
    return parsedResults.value;
  }
  
  return parsedResults.value.filter(row => {
    return activeFilters.value.every(filter => {
      const value = row[filter.column];
      const filterValue = filter.value;
      
      // Handle null checks
      if (filter.operator === 'is_null') {
        return value === null || value === undefined || value === '';
      }
      if (filter.operator === 'is_not_null') {
        return value !== null && value !== undefined && value !== '';
      }
      
      // Skip if value is null for other operators
      if (value === null || value === undefined) return false;
      
      const strValue = String(value).toLowerCase();
      const strFilterValue = String(filterValue).toLowerCase();
      
      switch (filter.operator) {
        case 'eq':
          return strValue === strFilterValue;
        case 'ne':
          return strValue !== strFilterValue;
        case 'contains':
          return strValue.includes(strFilterValue);
        case 'not_contains':
          return !strValue.includes(strFilterValue);
        case 'starts_with':
          return strValue.startsWith(strFilterValue);
        case 'ends_with':
          return strValue.endsWith(strFilterValue);
        case 'gt':
          return Number(value) > Number(filterValue);
        case 'lt':
          return Number(value) < Number(filterValue);
        case 'gte':
          return Number(value) >= Number(filterValue);
        case 'lte':
          return Number(value) <= Number(filterValue);
        case 'between':
          if (Array.isArray(filterValue)) {
            const numValue = Number(value);
            return numValue >= Number(filterValue[0]) && numValue <= Number(filterValue[1]);
          }
          return false;
        default:
          return true;
      }
    });
  });
});

function handleFiltersChange(filters: Filter[]) {
  activeFilters.value = filters;
}

function openConnectionModal() {
  showConnectionModal.value = true;
}

function closeConnectionModal() {
  showConnectionModal.value = false;
}

async function handleConnect(connectionConfig: any) {
  await connectionsStore.addConnection({
    name: connectionConfig.name,
    db_type: connectionConfig.type,
    host: connectionConfig.host,
    port: connectionConfig.port,
    database: connectionConfig.database,
    username: connectionConfig.username,
    password: connectionConfig.password,
    ssl_enabled: connectionConfig.sslEnabled,
  });
  closeConnectionModal();
}

function switchConnection(connectionId: string) {
  connectionsStore.setActiveConnection(connectionId);
}

async function deleteConnection(connectionId: string) {
  await connectionsStore.deleteConnection(connectionId);
}

function handleHistorySelect(historyItem: any) {
  query.value = historyItem.natural_query;
  sql.value = historyItem.sql_query;
  showConfirmation.value = true;
  showHistoryModal.value = false;
}

function handleSnippetSelect(snippet: any) {
  query.value = snippet.natural_query;
  sql.value = snippet.sql_query;
  showConfirmation.value = true;
  showSnippetsModal.value = false;
}

async function saveAsSnippet() {
  if (!sql.value.trim()) return;
  
  const name = prompt('Enter a name for this snippet:');
  if (!name) return;
  
  await snippetsStore.createSnippet({
    name,
    natural_query: query.value,
    sql_query: sql.value,
  });
}

</script>

<template>
  <div class="flex h-screen w-full bg-background-light dark:bg-background-dark text-white antialiased overflow-hidden">
    <!-- Sidebar -->
    <aside class="w-64 flex flex-col border-r border-[#2a3637] bg-background-dark shrink-0">
      <div class="p-6 flex items-center gap-3">
        <div class="size-8 bg-primary rounded-lg flex items-center justify-center">
          <span class="material-symbols-outlined text-white">database</span>
        </div>
        <h1 class="text-white text-lg font-bold tracking-tight">Query Studio</h1>
      </div>
      <nav class="flex-1 px-4 flex flex-col gap-6">
        <div>
          <p class="text-[#9fb4b7] text-[10px] uppercase tracking-widest font-bold mb-4 px-2">Connections</p>
          <div class="flex flex-col gap-1">
            <!-- Dynamic Connections List -->
            <div
              v-if="connectionsStore.connections.length === 0"
              class="px-3 py-4 text-center text-[#5d6f71] text-xs italic"
            >
              No connections yet. Click below to add one.
            </div>
            <div
              v-for="connection in connectionsStore.connections"
              :key="connection.id"
              @click="switchConnection(connection.id)"
              class="flex items-center gap-3 px-3 py-2 rounded-lg transition-colors cursor-pointer group relative"
              :class="
                connection.id === connectionsStore.activeConnectionId
                  ? 'bg-primary/20 border border-primary/30'
                  : 'hover:bg-white/5'
              "
            >
              <span
                class="material-symbols-outlined text-[20px]"
                :class="connection.id === connectionsStore.activeConnectionId ? 'text-primary' : 'text-[#9fb4b7] group-hover:text-white'"
              >
                {{ connection.db_type === 'postgres' ? 'storage' : connection.db_type === 'mysql' ? 'dns' : 'database' }}
              </span>
              <p
                class="text-sm font-medium flex-1 truncate"
                :class="connection.id === connectionsStore.activeConnectionId ? 'text-white' : 'text-[#9fb4b7] group-hover:text-white'"
              >
                {{ connection.name }}
              </p>
              <button
                v-if="connection.id === connectionsStore.activeConnectionId"
                @click.stop="deleteConnection(connection.id)"
                class="opacity-0 group-hover:opacity-100 p-1 hover:bg-red-500/20 rounded transition-all"
                title="Delete connection"
              >
                <span class="material-symbols-outlined text-[16px] text-red-400">delete</span>
              </button>
            </div>
            <!-- New Connection Button -->
            <button
              @click="openConnectionModal"
              class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-primary/10 transition-colors cursor-pointer group border border-dashed border-[#3d4f51] hover:border-primary mt-2"
            >
              <span class="material-symbols-outlined text-[#9fb4b7] group-hover:text-primary text-[20px]">add_circle</span>
              <p class="text-[#9fb4b7] group-hover:text-primary text-sm font-medium">New Connection</p>
            </button>
          </div>
        </div>
        <div>
          <p class="text-[#9fb4b7] text-[10px] uppercase tracking-widest font-bold mb-4 px-2">Workspace</p>
          <div class="flex flex-col gap-1">
            <div 
              @click="showHistoryModal = true"
              class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-white/5 transition-colors cursor-pointer group"
            >
              <span class="material-symbols-outlined text-[#9fb4b7] group-hover:text-white text-[20px]">history</span>
              <p class="text-[#9fb4b7] group-hover:text-white text-sm font-medium">Query History</p>
            </div>
            <div 
              @click="showSnippetsModal = true"
              class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-white/5 transition-colors cursor-pointer group"
            >
              <span class="material-symbols-outlined text-[#9fb4b7] group-hover:text-white text-[20px]">bookmark</span>
              <p class="text-[#9fb4b7] group-hover:text-white text-sm font-medium">Saved Snippets</p>
            </div>
            <div 
              v-if="connectionsStore.activeConnection"
              @click="showSchemaExplorer = true"
              class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-white/5 transition-colors cursor-pointer group"
            >
              <span class="material-symbols-outlined text-[#9fb4b7] group-hover:text-white text-[20px]">schema</span>
              <p class="text-[#9fb4b7] group-hover:text-white text-sm font-medium">Schema Explorer</p>
            </div>
            <div 
              v-if="parsedResults.length > 0"
              @click="showDataInsights = true"
              class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-white/5 transition-colors cursor-pointer group"
            >
              <span class="material-symbols-outlined text-[#9fb4b7] group-hover:text-white text-[20px]">analytics</span>
              <p class="text-[#9fb4b7] group-hover:text-white text-sm font-medium">Data Insights</p>
            </div>
            <div 
              v-else
              class="flex items-center gap-3 px-3 py-2 rounded-lg cursor-not-allowed opacity-50"
            >
              <span class="material-symbols-outlined text-[#9fb4b7] text-[20px]">analytics</span>
              <p class="text-[#9fb4b7] text-sm font-medium">Data Insights</p>
              <span class="text-[8px] bg-[#2a3637] text-[#5d6f71] px-1.5 py-0.5 rounded">Run query first</span>
            </div>
          </div>
        </div>
      </nav>
      <div class="p-4 border-t border-[#2a3637]">
        <div class="flex items-center gap-3 p-2">
          <div
            class="bg-primary/20 rounded-full size-8 flex items-center justify-center text-primary font-bold text-sm"
          >
            <span class="material-symbols-outlined text-[18px]">database</span>
          </div>
          <div class="flex flex-col overflow-hidden flex-1">
            <p class="text-white text-xs font-medium truncate">Query Studio</p>
            <p class="text-[#9fb4b7] text-[10px] truncate">Local workspace</p>
          </div>
          <button 
            @click="showSettingsModal = true"
            class="p-1.5 hover:bg-white/10 rounded-lg transition-colors"
            title="Settings"
          >
            <span class="material-symbols-outlined text-[#9fb4b7] hover:text-white text-[18px]">settings</span>
          </button>
        </div>
      </div>
    </aside>
    <!-- Main Content -->
    <main class="flex-1 flex flex-col min-w-0 bg-background-dark">
      <!-- Header -->
      <header class="flex items-center justify-between border-b border-[#2a3637] px-8 py-4">
        <div class="flex items-center gap-2 text-sm">
          <span class="text-[#9fb4b7]">Projects</span>
          <span class="text-[#9fb4b7]">/</span>
          <span class="text-white font-medium">{{ connectionsStore.activeConnection?.name || 'No Connection' }}</span>
          <span
            v-if="connectionsStore.activeConnection"
            class="ml-2 flex items-center gap-1.5 bg-emerald-500/10 text-emerald-500 px-2 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider"
          >
            <span class="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse"></span>
            Connected
          </span>
          <span
            v-else
            class="ml-2 flex items-center gap-1.5 bg-amber-500/10 text-amber-500 px-2 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider"
          >
            <span class="w-1.5 h-1.5 rounded-full bg-amber-500"></span>
            Not Connected
          </span>
        </div>
        <div class="flex items-center gap-4">
          <button 
            v-if="sql"
            @click="saveAsSnippet"
            class="p-2 text-[#9fb4b7] hover:text-primary transition-colors"
            title="Save as snippet"
          >
            <span class="material-symbols-outlined text-[22px]">bookmark_add</span>
          </button>
          <button class="p-2 text-[#9fb4b7] hover:text-white transition-colors opacity-50 cursor-not-allowed" title="Coming soon">
            <span class="material-symbols-outlined text-[22px]">share</span>
          </button>
        </div>
      </header>
      <div class="flex-1 flex flex-col overflow-y-auto custom-scrollbar">
        <!-- Composer Section -->
        <div class="px-8 pt-8 pb-4">
          <div class="max-w-5xl mx-auto w-full">
            <div class="flex flex-col gap-2 mb-4">
              <h2 class="text-xl font-bold">Natural Language Prompt</h2>
              <p class="text-[#9fb4b7] text-sm">Describe what data you need in plain English.</p>
            </div>
            <div class="relative group">
              <div
                class="absolute -inset-0.5 bg-gradient-to-r from-primary to-emerald-500 rounded-xl opacity-20 group-focus-within:opacity-40 transition duration-300"
              ></div>
              <div class="relative flex flex-col border border-[#3d4f51] bg-surface-dark rounded-xl overflow-hidden">
                <textarea
                  v-model="query"
                  @keyup.enter="translateQuery"
                  class="w-full bg-transparent border-none focus:ring-0 text-lg text-white placeholder:text-[#587174] p-6 resize-none min-h-[120px]"
                  placeholder="Find all users who signed up in the last 30 days and have spent more than $500..."
                ></textarea>
                <div class="flex items-center justify-between px-6 pb-4">
                  <div class="flex items-center gap-2">
                    <button class="p-2 text-[#9fb4b7] hover:text-white hover:bg-white/5 rounded-lg transition-colors">
                      <span class="material-symbols-outlined text-[20px]">attach_file</span>
                    </button>
                    <button class="p-2 text-[#9fb4b7] hover:text-white hover:bg-white/5 rounded-lg transition-colors">
                      <span class="material-symbols-outlined text-[20px]">auto_fix</span>
                    </button>
                    <span class="h-4 w-[1px] bg-[#3d4f51] mx-1"></span>
                    <span class="text-[11px] text-[#587174] font-medium uppercase tracking-widest">Powered by Gemini Pro</span>
                  </div>
                  <button
                    @click="translateQuery"
                    :disabled="isLoading || !query.trim()"
                    class="flex items-center gap-2 bg-primary hover:bg-primary/90 text-white px-5 py-2 rounded-lg font-bold text-sm transition-all shadow-lg shadow-primary/20"
                  >
                    <span v-if="isLoading" class="animate-spin material-symbols-outlined text-[18px]">progress_activity</span>
                    <span v-else class="material-symbols-outlined text-[18px]">bolt</span>
                    Generate SQL
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
        <!-- Split Workspace -->
        <div v-if="sql || isLoading" class="px-8 py-4 grid grid-cols-1 lg:grid-cols-2 gap-6 max-w-5xl mx-auto w-full">
          <!-- SQL Preview Panel -->
          <div class="flex flex-col border border-[#3d4f51] bg-surface-dark rounded-xl overflow-hidden min-h-[300px]">
            <div class="flex items-center justify-between px-4 py-3 bg-surface-dark-alt border-b border-[#3d4f51]">
              <div class="flex items-center gap-2">
                <span class="material-symbols-outlined text-primary text-[18px]">code</span>
                <span class="text-xs font-bold uppercase tracking-widest text-[#9fb4b7]">Generated SQL</span>
              </div>
              <div class="flex items-center gap-4">
                <button
                  @click="executeQuery(true, 1)"
                  :disabled="isLoading"
                  class="text-[10px] font-bold uppercase tracking-widest text-emerald-400 hover:text-emerald-300 transition-colors flex items-center gap-1"
                >
                  <span class="material-symbols-outlined text-[14px]">play_arrow</span>
                  Run Query
                </button>
                <button 
                  @click="openExplainQuery"
                  class="text-[10px] font-bold uppercase tracking-widest text-amber-400 hover:text-amber-300 transition-colors flex items-center gap-1"
                >
                  <span class="material-symbols-outlined text-[14px]">auto_awesome</span>
                  Explain
                </button>
                <button 
                  @click="copyToClipboard(sql)"
                  class="text-[10px] font-bold uppercase tracking-widest text-[#9fb4b7] hover:text-white transition-colors"
                >
                  Copy code
                </button>
              </div>
            </div>
            <div class="p-5 font-mono text-sm leading-relaxed overflow-x-auto flex-1">
              <pre v-if="isLoading && !sql" class="text-slate-500 animate-pulse">Generating SQL...</pre>
              <textarea
                v-else
                v-model="sql"
                class="w-full h-full min-h-[180px] bg-transparent border-none outline-none resize-none text-white font-mono text-sm leading-relaxed"
                spellcheck="false"
                placeholder="Write or paste your SQL query here..."
              ></textarea>
            </div>
          </div>
          <!-- Explain Query Panel -->
          <div class="flex flex-col border border-[#3d4f51] bg-[#1d2526] rounded-xl overflow-hidden min-h-[300px]">
            <div class="flex items-center gap-2 px-4 py-3 bg-[#242e2f] border-b border-[#3d4f51]">
              <span class="material-symbols-outlined text-emerald-400 text-[18px]">psychology</span>
              <span class="text-xs font-bold uppercase tracking-widest text-emerald-400/80">LLM Explanation</span>
            </div>
            <div class="p-5 text-sm leading-relaxed text-[#9fb4b7] space-y-4">
              <p v-if="isLoading && !sql" class="text-slate-500 animate-pulse">Analyzing logic...</p>
              <template v-else-if="sql">
                <p><strong class="text-white">Logic Breakdown:</strong></p>
                <ul class="space-y-3">
                  <li class="flex gap-3">
                    <span class="text-primary font-bold">01</span>
                    <span>Analyzes the schema to identify relevant tables and relationships.</span>
                  </li>
                  <li class="flex gap-3">
                    <span class="text-primary font-bold">02</span>
                    <span>Constructs the query using appropriate filters and aggregations.</span>
                  </li>
                  <li class="flex gap-3">
                    <span class="text-primary font-bold">03</span>
                    <span>Optimizes for performance based on typical indexing patterns.</span>
                  </li>
                </ul>
              </template>
              <p v-else class="text-[#587174] italic">Generate SQL to see an explanation of the logic.</p>
            </div>
          </div>
        </div>
        <!-- Results Table Section -->
        <div v-if="results || error || isLoading" class="px-8 py-6 max-w-5xl mx-auto w-full mb-12">
          <div class="flex items-center justify-between mb-4">
            <div class="flex items-center gap-3">
              <h3 class="font-bold text-lg">Query Results</h3>
              <span v-if="filteredResults.length" class="text-xs bg-white/5 text-[#9fb4b7] px-2 py-1 rounded">
                {{ filteredResults.length }} rows
                <template v-if="activeFilters.length > 0 && filteredResults.length !== parsedResults.length">
                  ({{ parsedResults.length }} total)
                </template>
              </span>
            </div>
            <div class="flex gap-2">
              <button
                v-if="parsedResults.length > 0"
                @click="showDataInsights = true"
                class="flex items-center gap-2 bg-surface-dark border border-[#3d4f51] px-3 py-1.5 rounded-lg text-xs font-medium hover:bg-white/5 hover:border-primary transition-colors"
              >
                <span class="material-symbols-outlined text-[16px]">analytics</span>
                Insights
              </button>
              <button
                v-if="parsedResults.length > 0"
                @click="showExportModal = true"
                class="flex items-center gap-2 bg-surface-dark border border-[#3d4f51] px-3 py-1.5 rounded-lg text-xs font-medium hover:bg-white/5 hover:border-primary transition-colors"
              >
                <span class="material-symbols-outlined text-[16px]">download</span>
                Export
              </button>
              <button
                @click="showFilterPanel = true"
                class="flex items-center gap-2 bg-surface-dark border border-[#3d4f51] px-3 py-1.5 rounded-lg text-xs font-medium hover:bg-white/5 hover:border-primary transition-colors"
                :class="{ 'border-primary bg-primary/10': activeFilters.length > 0 }"
              >
                <span class="material-symbols-outlined text-[16px]">filter_list</span>
                Filter
                <span v-if="activeFilters.length > 0" class="bg-primary text-white text-[10px] px-1.5 py-0.5 rounded-full font-bold">
                  {{ activeFilters.length }}
                </span>
              </button>
            </div>
          </div>
          <div class="border border-[#3d4f51] rounded-xl overflow-hidden bg-surface-dark shadow-xl">
            <div v-if="isLoading" class="p-12 text-center text-[#9fb4b7] flex flex-col items-center gap-3">
              <span class="animate-spin material-symbols-outlined text-3xl text-primary">progress_activity</span>
              <span>Executing query and fetching results...</span>
            </div>
            <div v-else-if="error" class="p-12 text-center text-red-400 bg-red-400/5">
              <span class="material-symbols-outlined text-3xl mb-2">error</span>
              <p class="font-medium">{{ error }}</p>
            </div>
            <div v-else-if="filteredResults.length" class="overflow-x-auto">
              <table class="w-full text-left text-sm border-collapse">
                <thead>
                  <tr class="bg-surface-dark-alt border-b border-[#3d4f51]">
                    <th
                      v-for="column in columns"
                      :key="column"
                      class="px-6 py-4 font-bold text-[#9fb4b7] uppercase tracking-wider text-[10px] cursor-pointer hover:text-white transition-colors"
                    >
                      <div class="flex items-center gap-2">
                        {{ column }}
                        <span class="material-symbols-outlined text-[14px]">expand_more</span>
                      </div>
                    </th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-[#3d4f51]">
                  <tr v-for="(row, index) in filteredResults" :key="index" class="hover:bg-primary/5 transition-colors">
                    <td
                      v-for="column in columns"
                      :key="column"
                      class="px-6 py-4 text-white font-medium"
                      :class="{ 'font-mono text-xs text-primary': column.toLowerCase().includes('id') }"
                    >
                      {{ row[column] }}
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
            <div v-else-if="parsedResults.length && activeFilters.length > 0" class="p-12 text-center text-[#587174]">
              <span class="material-symbols-outlined text-3xl mb-2 text-amber-400">filter_list_off</span>
              <p class="italic">No results match your filters</p>
              <button @click="activeFilters = []" class="mt-3 text-primary hover:text-primary/80 text-sm font-medium">
                Clear all filters
              </button>
            </div>
            <div v-else class="p-12 text-center text-[#587174] italic">No results found for this query.</div>
            <PaginationControls
              v-if="filteredResults.length"
              :current-page="currentPage"
              :page-size="pageSize"
              :total-count="totalCount"
              :has-more="hasMore"
              :loading="isLoading"
              @page-change="handlePageChange"
              @page-size-change="handlePageSizeChange"
            />
          </div>
        </div>
      </div>
    </main>

    <!-- Connection Modal -->
    <ConnectionModal :is-open="showConnectionModal" @close="closeConnectionModal" @connect="handleConnect" />
    
    <!-- Query History Modal -->
    <QueryHistory 
      v-if="showHistoryModal" 
      @close="showHistoryModal = false" 
      @select-query="handleHistorySelect" 
    />
    
    <!-- Snippets Modal -->
    <SnippetManager 
      v-if="showSnippetsModal" 
      @close="showSnippetsModal = false" 
      @select-snippet="handleSnippetSelect" 
    />
    
    <!-- Export Modal -->
    <ExportModal 
      v-if="showExportModal && results" 
      :data="results"
      :columns="columns"
      @close="showExportModal = false" 
    />
    
    <!-- Settings Modal -->
    <SettingsModal 
      v-if="showSettingsModal" 
      @close="showSettingsModal = false"
      @save="showSettingsModal = false"
    />
    
    <!-- Schema Explorer -->
    <SchemaExplorer 
      v-if="showSchemaExplorer && connectionsStore.activeConnection"
      :connection-string="connectionsStore.buildConnectionString(connectionsStore.activeConnection)"
      :engine="connectionsStore.activeConnection.db_type"
      @close="showSchemaExplorer = false"
      @select-table="(t) => console.log('Selected table:', t)"
      @generate-query="handleSchemaGenerateQuery"
    />
    
    <!-- Query Explanation -->
    <QueryExplanation 
      v-if="showQueryExplanation && sql"
      :sql-query="sql"
      :db-type="connectionsStore.activeConnection?.db_type"
      @close="showQueryExplanation = false"
    />
    
    <!-- Data Insights -->
    <DataInsights 
      v-if="showDataInsights && results"
      :data="results"
      :columns="columns"
      @close="showDataInsights = false"
    />
    
    <!-- Filter Panel -->
    <FilterPanel 
      v-if="showFilterPanel && parsedResults.length > 0"
      :columns="columnsWithTypes"
      :data="parsedResults"
      @close="showFilterPanel = false"
      @filters-change="handleFiltersChange"
    />
  </div>
</template>

<style>
.glass-sidebar {
  backdrop-filter: blur(20px);
  background: rgba(255, 255, 255, 0.4);
  border-right: 1px solid rgba(0, 0, 0, 0.05);
}

.glass-card {
  backdrop-filter: blur(10px);
  background: rgba(255, 255, 255, 0.8);
  border: 1px solid rgba(0, 0, 0, 0.08);
  box-shadow: 0 4px 24px -1px rgba(0, 0, 0, 0.04);
}
</style>
