<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import ResultsTable from "./ResultsTable.vue";

const query = ref("");
const sql = ref("");
const results = ref("");
const error = ref("");
const showConfirmation = ref(false);
const isLoading = ref(false);

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
    sql.value = (translatedSql as string).replace(/`/g, '').replace('sql', '');
    showConfirmation.value = true;
  } catch (err) {
    error.value = err as string;
    sql.value = "";
  } finally {
    isLoading.value = false;
  }
}

async function executeQuery() {
  isLoading.value = true;
  results.value = "";
  error.value = "";
  
  try {
    const response = await invoke("query_db", {
      engine: "postgres", // This should be dynamic based on DbConnector
      connStr: "postgresql://user:password@localhost:5432/db", // This should be dynamic
      query: sql.value,
    });
    results.value = response as string;
  } catch (err) {
    error.value = err as string;
  } finally {
    isLoading.value = false;
  }
}
</script>

<template>
  <div class="query-interface">
    <div class="panel query-panel">
      <textarea v-model="query" placeholder="e.g., Show me all users from California" @keyup.enter="translateQuery"></textarea>
      <button @click="translateQuery" :disabled="isLoading || !query.trim()">Translate to SQL</button>
    </div>

    <div class="panel sql-panel">
      <div class="panel-header">
        <h3>Generated SQL</h3>
        <button v-if="showConfirmation" @click="executeQuery" :disabled="isLoading">
          {{ isLoading ? 'Executing...' : 'Execute Query' }}
        </button>
      </div>
      <pre class="code-block">{{ sql || 'SQL will appear here...' }}</pre>
    </div>

    <div class="panel results-panel">
      <div class="panel-header">
        <h3>Results</h3>
      </div>
      <div class="results-content">
        <div v-if="isLoading" class="loading-indicator">
          <div class="spinner"></div>
          <p>Processing...</p>
        </div>
        <ResultsTable v-else-if="results" :results="results" />
        <p v-else-if="error" class="error-message">{{ error }}</p>
        <p v-else class="placeholder">Query results will appear here.</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.query-interface {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background-color: var(--bg-light);
  color: var(--text-primary-light);
}

.panel {
  display: flex;
  flex-direction: column;
  border-bottom: 1px solid var(--border-light);
}

.query-panel {
  padding: 20px;
  gap: 10px;
}

textarea {
  width: 100%;
  min-height: 80px;
  padding: 12px;
  border: 1px solid var(--border-light);
  border-radius: 8px;
  background-color: var(--bg-sidebar-light);
  color: var(--text-primary-light);
  font-family: inherit;
  font-size: 1rem;
  resize: vertical;
  box-sizing: border-box;
}

textarea:focus {
  outline: none;
  border-color: var(--accent-light);
  box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.3);
}

button {
  padding: 10px 20px;
  border: none;
  border-radius: 8px;
  background-color: var(--accent-light);
  color: white;
  cursor: pointer;
  transition: background-color 0.2s;
  font-size: 0.9rem;
  font-weight: 500;
  align-self: flex-end;
}

button:hover {
  background-color: var(--accent-hover-light);
}
button:disabled {
  background-color: #a0a0a0;
  cursor: not-allowed;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 20px;
  border-bottom: 1px solid var(--border-light);
}

h3 {
  margin: 0;
  font-size: 1rem;
  font-weight: 600;
  color: var(--text-secondary-light);
}

.code-block {
  padding: 20px;
  margin: 0;
  font-family: "SF Mono", "Fira Code", monospace;
  font-size: 0.9rem;
  white-space: pre-wrap;
  word-wrap: break-word;
  color: var(--text-secondary-light);
  background-color: var(--bg-sidebar-light);
  flex-grow: 1;
}

.results-panel {
  flex: 1;
  min-height: 0;
}
.results-content {
  padding: 20px;
  overflow: auto;
  flex-grow: 1;
}
.placeholder, .error-message {
  color: var(--text-secondary-light);
}

.error-message {
  color: #ff5555;
  white-space: pre-wrap;
}

.loading-indicator {
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  gap: 10px;
  padding: 20px;
  color: var(--text-secondary-light);
}

.spinner {
  width: 32px;
  height: 32px;
  border: 4px solid var(--border-light);
  border-top-color: var(--accent-light);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-color-scheme: dark) {
  .query-interface {
    background-color: var(--bg-dark);
    color: var(--text-primary-dark);
  }
  .panel, .panel-header {
    border-color: var(--border-dark);
  }
  textarea {
    border-color: var(--border-dark);
    background-color: var(--bg-sidebar-dark);
    color: var(--text-primary-dark);
  }
  textarea:focus {
    border-color: var(--accent-dark);
    box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.3);
  }
  button {
    background-color: var(--accent-dark);
  }
  button:hover {
    background-color: var(--accent-hover-dark);
  }
  button:disabled {
    background-color: #555;
  }
  h3, .code-block, .placeholder {
    color: var(--text-secondary-dark);
  }
  .code-block {
    background-color: var(--bg-sidebar-dark);
  }
  .spinner {
    border-color: var(--border-dark);
    border-top-color: var(--accent-dark);
  }
}
</style>
