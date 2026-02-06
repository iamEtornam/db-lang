import { defineStore } from 'pinia';
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

export interface QueryHistory {
  id: string;
  connection_id: string;
  natural_query: string;
  sql_query: string;
  result_count: number | null;
  execution_time_ms: number | null;
  status: string;
  error_message: string | null;
  created_at: string;
}

export interface AddHistoryRequest {
  connection_id: string;
  natural_query: string;
  sql_query: string;
  result_count?: number | null;
  execution_time_ms?: number | null;
  status: string;
  error_message?: string | null;
}

export const useHistoryStore = defineStore('history', () => {
  const history = ref<QueryHistory[]>([]);
  const isLoading = ref(false);
  const error = ref<string | null>(null);
  const hasMore = ref(true);
  const currentOffset = ref(0);
  const pageSize = 50;

  async function loadHistory(reset = false) {
    if (reset) {
      currentOffset.value = 0;
      history.value = [];
      hasMore.value = true;
    }

    if (!hasMore.value) return;

    isLoading.value = true;
    error.value = null;

    try {
      const result = await invoke<QueryHistory[]>('get_history', {
        limit: pageSize,
        offset: currentOffset.value,
      });

      if (result.length < pageSize) {
        hasMore.value = false;
      }

      if (reset) {
        history.value = result;
      } else {
        history.value.push(...result);
      }
      currentOffset.value += result.length;
    } catch (err) {
      error.value = err as string;
    } finally {
      isLoading.value = false;
    }
  }

  async function addToHistory(historyItem: AddHistoryRequest): Promise<QueryHistory | null> {
    try {
      const result = await invoke<QueryHistory>('add_to_history', {
        history: historyItem,
      });
      history.value.unshift(result);
      return result;
    } catch (err) {
      console.error('Failed to add to history:', err);
      return null;
    }
  }

  async function searchHistory(searchTerm: string): Promise<QueryHistory[]> {
    isLoading.value = true;
    error.value = null;

    try {
      const result = await invoke<QueryHistory[]>('search_history', {
        searchTerm,
        limit: 50,
      });
      return result;
    } catch (err) {
      error.value = err as string;
      return [];
    } finally {
      isLoading.value = false;
    }
  }

  async function clearOldHistory(daysToKeep = 30): Promise<number> {
    try {
      const count = await invoke<number>('clear_old_history', { daysToKeep });
      await loadHistory(true);
      return count;
    } catch (err) {
      error.value = err as string;
      return 0;
    }
  }

  function clearError() {
    error.value = null;
  }

  return {
    history,
    isLoading,
    error,
    hasMore,
    loadHistory,
    addToHistory,
    searchHistory,
    clearOldHistory,
    clearError,
  };
});
