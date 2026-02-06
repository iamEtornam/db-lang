<script setup lang="ts">
import { ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const props = defineProps<{
  sqlQuery: string;
  dbType?: string;
}>();

const emit = defineEmits<{
  close: [];
  applyImprovement: [query: string];
}>();

interface ClauseExplanation {
  clause_type: string;
  content: string;
  explanation: string;
}

interface QueryExplanation {
  summary: string;
  clauses: ClauseExplanation[];
  tables_involved: string[];
  potential_issues: string[];
  optimization_tips: string[];
}

const explanation = ref<QueryExplanation | null>(null);
const improvements = ref<string>('');
const loading = ref(false);
const improvementsLoading = ref(false);
const error = ref('');
const activeTab = ref<'explanation' | 'improvements'>('explanation');

async function loadExplanation() {
  if (!props.sqlQuery) return;
  
  loading.value = true;
  error.value = '';
  
  try {
    explanation.value = await invoke<QueryExplanation>('explain_query', {
      sqlQuery: props.sqlQuery,
    });
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
}

async function loadImprovements() {
  if (!props.sqlQuery) return;
  
  improvementsLoading.value = true;
  
  try {
    improvements.value = await invoke<string>('suggest_query_improvements', {
      sqlQuery: props.sqlQuery,
      dbType: props.dbType || 'SQL',
    });
  } catch (e) {
    improvements.value = 'Unable to generate improvements. AI features require a valid API key.';
  } finally {
    improvementsLoading.value = false;
  }
}

function getClauseColor(clauseType: string): string {
  const colors: Record<string, string> = {
    SELECT: 'text-blue-400 bg-blue-500/10',
    FROM: 'text-emerald-400 bg-emerald-500/10',
    WHERE: 'text-amber-400 bg-amber-500/10',
    JOIN: 'text-purple-400 bg-purple-500/10',
    'LEFT JOIN': 'text-purple-400 bg-purple-500/10',
    'RIGHT JOIN': 'text-purple-400 bg-purple-500/10',
    'INNER JOIN': 'text-purple-400 bg-purple-500/10',
    'GROUP BY': 'text-pink-400 bg-pink-500/10',
    'ORDER BY': 'text-cyan-400 bg-cyan-500/10',
    HAVING: 'text-orange-400 bg-orange-500/10',
    LIMIT: 'text-gray-400 bg-gray-500/10',
  };
  return colors[clauseType.toUpperCase()] || 'text-[#9fb4b7] bg-white/5';
}

watch(() => props.sqlQuery, () => {
  if (props.sqlQuery) {
    loadExplanation();
  }
}, { immediate: true });

watch(activeTab, (tab) => {
  if (tab === 'improvements' && !improvements.value && !improvementsLoading.value) {
    loadImprovements();
  }
});
</script>

<template>
  <div class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50" @click.self="emit('close')">
    <div class="bg-surface-dark border border-[#3d4f51] rounded-2xl w-full max-w-3xl max-h-[80vh] shadow-2xl overflow-hidden flex flex-col">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-4 border-b border-[#2a3637] shrink-0">
        <div class="flex items-center gap-3">
          <span class="material-symbols-outlined text-primary">auto_awesome</span>
          <h2 class="text-lg font-semibold text-white">Query Analysis</h2>
        </div>
        <button
          @click="emit('close')"
          class="p-1.5 hover:bg-white/10 rounded-lg transition-colors"
        >
          <span class="material-symbols-outlined text-[#9fb4b7]">close</span>
        </button>
      </div>

      <!-- Query preview -->
      <div class="px-6 py-4 bg-background-dark border-b border-[#2a3637] shrink-0">
        <pre class="text-sm text-[#9fb4b7] font-mono whitespace-pre-wrap max-h-24 overflow-y-auto">{{ sqlQuery }}</pre>
      </div>

      <!-- Tabs -->
      <div class="flex gap-1 px-6 pt-4 shrink-0">
        <button
          @click="activeTab = 'explanation'"
          class="px-4 py-2 text-sm font-medium rounded-lg transition-colors"
          :class="activeTab === 'explanation' ? 'bg-primary/20 text-primary' : 'text-[#9fb4b7] hover:text-white'"
        >
          Explanation
        </button>
        <button
          @click="activeTab = 'improvements'"
          class="px-4 py-2 text-sm font-medium rounded-lg transition-colors flex items-center gap-1"
          :class="activeTab === 'improvements' ? 'bg-primary/20 text-primary' : 'text-[#9fb4b7] hover:text-white'"
        >
          <span class="material-symbols-outlined text-[14px]">tips_and_updates</span>
          Improvements
        </button>
      </div>

      <!-- Content -->
      <div class="flex-1 overflow-y-auto p-6">
        <!-- Loading -->
        <div v-if="(activeTab === 'explanation' && loading) || (activeTab === 'improvements' && improvementsLoading)" class="flex items-center justify-center py-12">
          <div class="text-center">
            <div class="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin mx-auto mb-4"></div>
            <p class="text-[#9fb4b7] text-sm">Analyzing query with AI...</p>
          </div>
        </div>

        <!-- Error -->
        <div v-else-if="activeTab === 'explanation' && error" class="text-center py-12">
          <span class="material-symbols-outlined text-4xl text-red-400 mb-2">error</span>
          <p class="text-red-400">{{ error }}</p>
          <button
            @click="loadExplanation"
            class="mt-4 px-4 py-2 bg-primary/20 text-primary text-sm rounded-lg hover:bg-primary/30 transition-colors"
          >
            Try Again
          </button>
        </div>

        <!-- Explanation tab -->
        <div v-else-if="activeTab === 'explanation' && explanation" class="space-y-6">
          <!-- Summary -->
          <div class="p-4 bg-primary/10 border border-primary/20 rounded-xl">
            <div class="flex items-start gap-3">
              <span class="material-symbols-outlined text-primary text-[20px] mt-0.5">lightbulb</span>
              <div>
                <h3 class="text-white font-medium mb-1">Summary</h3>
                <p class="text-[#9fb4b7] text-sm">{{ explanation.summary }}</p>
              </div>
            </div>
          </div>

          <!-- Tables involved -->
          <div v-if="explanation.tables_involved.length > 0">
            <h3 class="text-white font-medium mb-2 flex items-center gap-2">
              <span class="material-symbols-outlined text-[18px] text-[#9fb4b7]">table_chart</span>
              Tables Involved
            </h3>
            <div class="flex flex-wrap gap-2">
              <span
                v-for="table in explanation.tables_involved"
                :key="table"
                class="px-3 py-1 bg-emerald-500/10 text-emerald-400 text-sm rounded-lg border border-emerald-500/20"
              >
                {{ table }}
              </span>
            </div>
          </div>

          <!-- Clauses breakdown -->
          <div v-if="explanation.clauses.length > 0">
            <h3 class="text-white font-medium mb-3 flex items-center gap-2">
              <span class="material-symbols-outlined text-[18px] text-[#9fb4b7]">segment</span>
              Query Breakdown
            </h3>
            <div class="space-y-3">
              <div
                v-for="(clause, idx) in explanation.clauses"
                :key="idx"
                class="p-4 bg-background-dark rounded-lg border border-[#2a3637]"
              >
                <div class="flex items-center gap-2 mb-2">
                  <span
                    class="px-2 py-0.5 text-xs font-bold rounded"
                    :class="getClauseColor(clause.clause_type)"
                  >
                    {{ clause.clause_type }}
                  </span>
                </div>
                <code class="text-xs text-[#9fb4b7] font-mono block mb-2 bg-black/20 p-2 rounded">{{ clause.content }}</code>
                <p class="text-sm text-white">{{ clause.explanation }}</p>
              </div>
            </div>
          </div>

          <!-- Potential issues -->
          <div v-if="explanation.potential_issues.length > 0">
            <h3 class="text-white font-medium mb-2 flex items-center gap-2">
              <span class="material-symbols-outlined text-[18px] text-amber-400">warning</span>
              Potential Issues
            </h3>
            <ul class="space-y-2">
              <li
                v-for="(issue, idx) in explanation.potential_issues"
                :key="idx"
                class="flex items-start gap-2 text-sm text-amber-300 bg-amber-500/10 p-3 rounded-lg border border-amber-500/20"
              >
                <span class="material-symbols-outlined text-[16px] mt-0.5">arrow_right</span>
                {{ issue }}
              </li>
            </ul>
          </div>

          <!-- Optimization tips -->
          <div v-if="explanation.optimization_tips.length > 0">
            <h3 class="text-white font-medium mb-2 flex items-center gap-2">
              <span class="material-symbols-outlined text-[18px] text-emerald-400">bolt</span>
              Optimization Tips
            </h3>
            <ul class="space-y-2">
              <li
                v-for="(tip, idx) in explanation.optimization_tips"
                :key="idx"
                class="flex items-start gap-2 text-sm text-emerald-300 bg-emerald-500/10 p-3 rounded-lg border border-emerald-500/20"
              >
                <span class="material-symbols-outlined text-[16px] mt-0.5">check</span>
                {{ tip }}
              </li>
            </ul>
          </div>
        </div>

        <!-- Improvements tab -->
        <div v-else-if="activeTab === 'improvements'" class="prose prose-invert max-w-none">
          <div v-if="improvements" class="whitespace-pre-wrap text-sm text-[#9fb4b7] leading-relaxed">
            {{ improvements }}
          </div>
          <div v-else class="text-center py-12 text-[#5d6f71]">
            <span class="material-symbols-outlined text-4xl mb-2">tips_and_updates</span>
            <p>Click to generate improvement suggestions</p>
            <button
              @click="loadImprovements"
              class="mt-4 px-4 py-2 bg-primary text-white text-sm rounded-lg hover:bg-primary/90 transition-colors"
            >
              Generate Suggestions
            </button>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="flex items-center justify-end px-6 py-4 border-t border-[#2a3637] gap-3 shrink-0">
        <button
          @click="emit('close')"
          class="px-4 py-2 text-sm font-medium text-[#9fb4b7] hover:text-white transition-colors"
        >
          Close
        </button>
      </div>
    </div>
  </div>
</template>
