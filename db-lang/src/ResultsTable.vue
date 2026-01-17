<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  results: string;
}>();

const parsedResults = computed(() => {
  if (!props.results) return [];
  try {
    const data = JSON.parse(props.results);
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
</script>

<template>
  <div class="results-table-container">
    <table v-if="parsedResults.length > 0">
      <thead>
        <tr>
          <th v-for="column in columns" :key="column">{{ column }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(row, index) in parsedResults" :key="index">
          <td v-for="column in columns" :key="column">{{ row[column] }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.results-table-container {
  width: 100%;
  overflow-x: auto;
  border: 1px solid var(--border-light);
  border-radius: 8px;
  background-color: var(--bg-sidebar-light);
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.9rem;
}

th,
td {
  padding: 12px 15px;
  text-align: left;
  border-bottom: 1px solid var(--border-light);
  white-space: nowrap;
}

thead tr {
  background-color: var(--bg-inset-light);
}

th {
  font-weight: 600;
  color: var(--text-secondary-light);
}

tbody tr:last-child td {
  border-bottom: none;
}

@media (prefers-color-scheme: dark) {
  .results-table-container {
    border-color: var(--border-dark);
    background-color: var(--bg-sidebar-dark);
  }

  th,
  td {
    border-bottom-color: var(--border-dark);
  }

  thead tr {
    background-color: var(--bg-inset-dark);
  }

  th {
    color: var(--text-secondary-dark);
  }
}
</style>
