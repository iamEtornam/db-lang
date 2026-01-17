<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import ConnectionModal from "./components/ConnectionModal.vue";

interface Connection {
  id: string;
  name: string;
  type: string;
  host: string;
  port: string;
  username: string;
  password: string;
  sslEnabled: boolean;
}

const query = ref("");
const sql = ref("");
const results = ref("");
const error = ref("");
const isLoading = ref(false);
const showConfirmation = ref(false);
const showConnectionModal = ref(false);
const connections = ref<Connection[]>([]);
const activeConnectionId = ref<string | null>(null);

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
    sql.value = (translatedSql as string).replace(/`/g, "").replace("sql", "");
    showConfirmation.value = true;
  } catch (err) {
    error.value = err as string;
    sql.value = "";
  } finally {
    isLoading.value = false;
  }
}

async function executeQuery() {
  if (!activeConnection.value) {
    error.value = "No active connection. Please select or create a connection.";
    return;
  }

  isLoading.value = true;
  results.value = "";
  error.value = "";

  try {
    const conn = activeConnection.value;
    const connStr = buildConnectionString(conn);
    
    const response = await invoke("query_db", {
      engine: conn.type,
      connStr: connStr,
      query: sql.value,
    });
    results.value = response as string;
  } catch (err) {
    error.value = err as string;
  } finally {
    isLoading.value = false;
  }
}

function buildConnectionString(conn: Connection): string {
  const { type, host, port, username, password } = conn;
  
  switch (type) {
    case "postgres":
      return `postgresql://${username}:${password}@${host}:${port}/postgres`;
    case "mysql":
      return `mysql://${username}:${password}@${host}:${port}/mysql`;
    case "sqlite":
      return host; // For SQLite, host is the file path
    case "mssql":
      return `mssql://${username}:${password}@${host}:${port}/master`;
    default:
      return "";
  }
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

const activeConnection = computed(() => {
  return connections.value.find(c => c.id === activeConnectionId.value) || null;
});

// Load connections from localStorage on mount
onMounted(() => {
  loadConnections();
});

function loadConnections() {
  const stored = localStorage.getItem("db-connections");
  if (stored) {
    try {
      const data = JSON.parse(stored);
      connections.value = data.connections || [];
      activeConnectionId.value = data.activeId || null;
    } catch (e) {
      console.error("Failed to load connections:", e);
    }
  }
}

function saveConnections() {
  localStorage.setItem(
    "db-connections",
    JSON.stringify({
      connections: connections.value,
      activeId: activeConnectionId.value,
    })
  );
}

function openConnectionModal() {
  showConnectionModal.value = true;
}

function closeConnectionModal() {
  showConnectionModal.value = false;
}

function handleConnect(connectionConfig: any) {
  const newConnection: Connection = {
    id: Date.now().toString(),
    name: connectionConfig.name,
    type: connectionConfig.type,
    host: connectionConfig.host,
    port: connectionConfig.port,
    username: connectionConfig.username,
    password: connectionConfig.password,
    sslEnabled: connectionConfig.sslEnabled,
  };

  connections.value.push(newConnection);
  activeConnectionId.value = newConnection.id;
  saveConnections();
  closeConnectionModal();
}

function switchConnection(connectionId: string) {
  activeConnectionId.value = connectionId;
  saveConnections();
}

function deleteConnection(connectionId: string) {
  connections.value = connections.value.filter(c => c.id !== connectionId);
  if (activeConnectionId.value === connectionId) {
    activeConnectionId.value = connections.value[0]?.id || null;
  }
  saveConnections();
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
              v-if="connections.length === 0"
              class="px-3 py-4 text-center text-[#5d6f71] text-xs italic"
            >
              No connections yet. Click below to add one.
            </div>
            <div
              v-for="connection in connections"
              :key="connection.id"
              @click="switchConnection(connection.id)"
              class="flex items-center gap-3 px-3 py-2 rounded-lg transition-colors cursor-pointer group relative"
              :class="
                connection.id === activeConnectionId
                  ? 'bg-primary/20 border border-primary/30'
                  : 'hover:bg-white/5'
              "
            >
              <span
                class="material-symbols-outlined text-[20px]"
                :class="connection.id === activeConnectionId ? 'text-primary' : 'text-[#9fb4b7] group-hover:text-white'"
              >
                {{ connection.type === 'postgres' ? 'storage' : connection.type === 'mysql' ? 'dns' : 'database' }}
              </span>
              <p
                class="text-sm font-medium flex-1 truncate"
                :class="connection.id === activeConnectionId ? 'text-white' : 'text-[#9fb4b7] group-hover:text-white'"
              >
                {{ connection.name }}
              </p>
              <button
                v-if="connection.id === activeConnectionId"
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
            <div class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-white/5 transition-colors cursor-pointer group">
              <span class="material-symbols-outlined text-[#9fb4b7] group-hover:text-white text-[20px]">history</span>
              <p class="text-[#9fb4b7] group-hover:text-white text-sm font-medium">Query History</p>
            </div>
            <div class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-white/5 transition-colors cursor-pointer group">
              <span class="material-symbols-outlined text-[#9fb4b7] group-hover:text-white text-[20px]">bookmark</span>
              <p class="text-[#9fb4b7] group-hover:text-white text-sm font-medium">Saved Snippets</p>
            </div>
            <div class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-white/5 transition-colors cursor-pointer group">
              <span class="material-symbols-outlined text-[#9fb4b7] group-hover:text-white text-[20px]">analytics</span>
              <p class="text-[#9fb4b7] group-hover:text-white text-sm font-medium">Data Insights</p>
            </div>
          </div>
        </div>
      </nav>
      <div class="p-4 border-t border-[#2a3637]">
        <div class="flex items-center gap-3 p-2">
          <div
            class="bg-center bg-no-repeat aspect-square bg-cover rounded-full size-8"
            data-alt="User profile avatar"
            style='background-image: url("https://lh3.googleusercontent.com/aida-public/AB6AXuDnV1S31a7HvhuWJCczFZOPHPw-nBNTNXg5J7nAI8VYvLfrHhPjAcINWn9cbhFz06p8ZmqKS4lJGXGqRlkDC-DpQs5VCimfCgjoSvB7voSiqMEpGj1zw1XTjosvfXy8ETMisWxuUJZNE8LGYKNwju55cVlmdbDkNezDWQj7sbTdWZ9fqyxKv0qlRayGhXDITat_J90AqlNBR-ydIb41ZAosgllj0pSDW4k2kZqkxeYJ83zK6Gjmip39vik8bGo3u_UNBtNGWtd-pZA");'
          ></div>
          <div class="flex flex-col overflow-hidden">
            <p class="text-white text-xs font-medium truncate">Alex Rivera</p>
            <p class="text-[#9fb4b7] text-[10px] truncate">Pro Plan</p>
          </div>
          <span class="material-symbols-outlined text-[#9fb4b7] text-[18px] ml-auto cursor-pointer">settings</span>
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
          <span class="text-white font-medium">{{ activeConnection?.name || 'No Connection' }}</span>
          <span
            v-if="activeConnection"
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
          <button class="p-2 text-[#9fb4b7] hover:text-white transition-colors">
            <span class="material-symbols-outlined text-[22px]">notifications</span>
          </button>
          <button
            class="flex items-center gap-2 bg-surface-dark border border-[#3d4f51] px-3 py-1.5 rounded-lg text-sm font-medium hover:border-primary transition-all"
          >
            <span class="material-symbols-outlined text-[18px]">share</span>
            Share
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
                  @click="executeQuery"
                  :disabled="isLoading"
                  class="text-[10px] font-bold uppercase tracking-widest text-emerald-400 hover:text-emerald-300 transition-colors flex items-center gap-1"
                >
                  <span class="material-symbols-outlined text-[14px]">play_arrow</span>
                  Run Query
                </button>
                <button class="text-[10px] font-bold uppercase tracking-widest text-[#9fb4b7] hover:text-white transition-colors">
                  Copy code
                </button>
              </div>
            </div>
            <div class="p-5 font-mono text-sm leading-relaxed overflow-x-auto">
              <pre v-if="isLoading && !sql" class="text-slate-500 animate-pulse">Generating SQL...</pre>
              <pre v-else><code>{{ sql }}</code></pre>
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
              <span v-if="parsedResults.length" class="text-xs bg-white/5 text-[#9fb4b7] px-2 py-1 rounded">
                {{ parsedResults.length }} rows found
              </span>
            </div>
            <div class="flex gap-2">
              <button
                class="flex items-center gap-2 bg-surface-dark border border-[#3d4f51] px-3 py-1.5 rounded-lg text-xs font-medium hover:bg-white/5"
              >
                <span class="material-symbols-outlined text-[16px]">download</span>
                Export CSV
              </button>
              <button
                class="flex items-center gap-2 bg-surface-dark border border-[#3d4f51] px-3 py-1.5 rounded-lg text-xs font-medium hover:bg-white/5"
              >
                <span class="material-symbols-outlined text-[16px]">filter_list</span>
                Filter
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
            <div v-else-if="parsedResults.length" class="overflow-x-auto">
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
                  <tr v-for="(row, index) in parsedResults" :key="index" class="hover:bg-primary/5 transition-colors">
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
            <div v-else class="p-12 text-center text-[#587174] italic">No results found for this query.</div>
            <div v-if="parsedResults.length" class="px-6 py-4 bg-surface-dark-alt flex items-center justify-between border-t border-[#3d4f51]">
              <p class="text-[10px] text-[#9fb4b7] uppercase tracking-widest font-bold">Showing all {{ parsedResults.length }} rows</p>
              <div class="flex gap-2">
                <button class="p-1 hover:text-white text-[#9fb4b7] disabled:opacity-30" disabled>
                  <span class="material-symbols-outlined">chevron_left</span>
                </button>
                <button class="p-1 hover:text-white text-[#9fb4b7]">
                  <span class="material-symbols-outlined">chevron_right</span>
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </main>

    <!-- Connection Modal -->
    <ConnectionModal :is-open="showConnectionModal" @close="closeConnectionModal" @connect="handleConnect" />
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
