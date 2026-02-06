<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { useHistoryStore, type QueryHistory } from '../stores/history';

const emit = defineEmits<{
  close: [];
  selectQuery: [history: QueryHistory];
}>();

const historyStore = useHistoryStore();

const searchTerm = ref('');
const searchResults = ref<QueryHistory[] | null>(null);

onMounted(() => {
  historyStore.loadHistory(true);
});

const displayedHistory = computed(() => {
  if (searchResults.value !== null) {
    return searchResults.value;
  }
  return historyStore.history;
});

async function handleSearch() {
  if (!searchTerm.value.trim()) {
    searchResults.value = null;
    return;
  }
  searchResults.value = await historyStore.searchHistory(searchTerm.value);
}

function clearSearch() {
  searchTerm.value = '';
  searchResults.value = null;
}

function selectHistory(history: QueryHistory) {
  emit('selectQuery', history);
  emit('close');
}

function formatDate(dateString: string): string {
  const date = new Date(dateString);
  return date.toLocaleString();
}

function getStatusColor(status: string): string {
  switch (status) {
    case 'success':
      return 'text-emerald-400';
    case 'error':
      return 'text-red-400';
    default:
      return 'text-amber-400';
  }
}

async function handleClearOld() {
  if (confirm('Clear history older than 30 days?')) {
    const count = await historyStore.clearOldHistory(30);
    if (count > 0) {
      alert(`Cleared ${count} old entries`);
    }
  }
}
</script>

<template>
  <div class="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4" @click.self="emit('close')">
    <div class="w-full max-w-3xl bg-surface-dark rounded-xl shadow-[0_20px_70px_rgba(0,0,0,0.55)] overflow-hidden border border-[#2a3637] flex flex-col max-h-[80vh]">
      <!-- Header -->
      <header class="flex items-center justify-between px-6 py-4 border-b border-[#2a3637]">
        <div class="flex items-center gap-3">
          <span class="material-symbols-outlined text-primary text-[20px]">history</span>
          <h2 class="text-lg font-bold text-white">Query History</h2>
        </div>
        <button @click="emit('close')" class="p-1 hover:bg-white/5 rounded-lg transition-colors">
          <span class="material-symbols-outlined text-[#9fb4b7] text-[20px]">close</span>
        </button>
      </header>

      <!-- Search Bar -->
      <div class="px-6 py-4 border-b border-[#2a3637]">
        <div class="flex gap-3">
          <div class="relative flex-1">
            <input
              v-model="searchTerm"
              @input="handleSearch"
              class="w-full h-10 pl-10 pr-4 bg-[#1d2526] border border-[#3d4f51] rounded-lg text-white placeholder:text-[#5d6f71] focus:border-primary focus:ring-0 outline-none text-sm"
              placeholder="Search queries..."
            />
            <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-[#5d6f71] text-[18px]">search</span>
          </div>
          <button
            v-if="searchTerm"
            @click="clearSearch"
            class="px-3 h-10 bg-[#1d2526] border border-[#3d4f51] rounded-lg text-[#9fb4b7] hover:text-white text-sm font-medium transition-colors"
          >
            Clear
          </button>
          <button
            @click="handleClearOld"
            class="px-3 h-10 bg-[#1d2526] border border-[#3d4f51] rounded-lg text-[#9fb4b7] hover:text-red-400 text-sm font-medium transition-colors"
            title="Clear entries older than 30 days"
          >
            <span class="material-symbols-outlined text-[18px]">delete_sweep</span>
          </button>
        </div>
      </div>

      <!-- History List -->
      <div class="flex-1 overflow-y-auto">
        <div v-if="historyStore.isLoading" class="p-8 text-center text-[#9fb4b7]">
          <span class="material-symbols-outlined animate-spin text-3xl text-primary mb-2">progress_activity</span>
          <p>Loading history...</p>
        </div>

        <div v-else-if="displayedHistory.length === 0" class="p-8 text-center text-[#5d6f71]">
          <span class="material-symbols-outlined text-4xl mb-3">search_off</span>
          <p v-if="searchTerm">No matching queries found</p>
          <p v-else>No query history yet</p>
        </div>

        <div v-else class="divide-y divide-[#2a3637]">
          <div
            v-for="item in displayedHistory"
            :key="item.id"
            @click="selectHistory(item)"
            class="px-6 py-4 hover:bg-white/5 cursor-pointer transition-colors group"
          >
            <div class="flex items-start justify-between gap-4 mb-2">
              <p class="text-white text-sm font-medium line-clamp-1 flex-1">{{ item.natural_query }}</p>
              <span 
                class="text-[10px] font-bold uppercase tracking-wider px-2 py-0.5 rounded"
                :class="[getStatusColor(item.status), item.status === 'success' ? 'bg-emerald-500/10' : 'bg-red-500/10']"
              >
                {{ item.status }}
              </span>
            </div>
            <pre class="text-xs text-[#9fb4b7] font-mono bg-[#1d2526] rounded px-3 py-2 overflow-x-auto line-clamp-2 mb-2">{{ item.sql_query }}</pre>
            <div class="flex items-center gap-4 text-[10px] text-[#5d6f71] uppercase tracking-wider">
              <span>{{ formatDate(item.created_at) }}</span>
              <span v-if="item.result_count !== null">{{ item.result_count }} rows</span>
              <span v-if="item.execution_time_ms !== null">{{ item.execution_time_ms }}ms</span>
            </div>
            <p v-if="item.error_message" class="text-xs text-red-400 mt-2 line-clamp-1">{{ item.error_message }}</p>
          </div>
        </div>

        <!-- Load More -->
        <div v-if="historyStore.hasMore && !searchTerm && displayedHistory.length > 0" class="p-4 text-center">
          <button
            @click="historyStore.loadHistory()"
            :disabled="historyStore.isLoading"
            class="px-4 py-2 bg-[#1d2526] border border-[#3d4f51] rounded-lg text-[#9fb4b7] hover:text-white text-sm font-medium transition-colors disabled:opacity-50"
          >
            Load More
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
