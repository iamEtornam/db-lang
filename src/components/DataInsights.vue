<script setup lang="ts">
import { ref, computed, watch } from 'vue';

const props = defineProps<{
  data: string;
  columns: string[];
}>();

const emit = defineEmits<{
  close: [];
}>();

interface ColumnStats {
  name: string;
  type: 'string' | 'number' | 'boolean' | 'date' | 'null' | 'mixed';
  totalCount: number;
  nullCount: number;
  uniqueCount: number;
  min?: number | string;
  max?: number | string;
  avg?: number;
  sum?: number;
  topValues: { value: string; count: number }[];
}

interface DataQualityIssue {
  type: 'warning' | 'error' | 'info';
  column: string;
  message: string;
}

const parsedData = computed(() => {
  if (!props.data) return [];
  try {
    const data = JSON.parse(props.data);
    return Array.isArray(data) ? data : [];
  } catch {
    return [];
  }
});

const activeTab = ref<'overview' | 'columns' | 'quality'>('overview');
const selectedColumn = ref<string | null>(null);

const columnStats = computed<ColumnStats[]>(() => {
  if (parsedData.value.length === 0 || props.columns.length === 0) return [];
  
  return props.columns.map(col => {
    const values = parsedData.value.map(row => row[col]);
    const nonNullValues = values.filter(v => v !== null && v !== undefined && v !== '');
    
    // Determine type
    let type: ColumnStats['type'] = 'mixed';
    const typeChecks = nonNullValues.slice(0, 100).map(v => {
      if (v === null || v === undefined) return 'null';
      if (typeof v === 'boolean') return 'boolean';
      if (typeof v === 'number' || !isNaN(Number(v))) return 'number';
      if (isDateString(String(v))) return 'date';
      return 'string';
    });
    
    const typeCounts = typeChecks.reduce((acc, t) => {
      acc[t] = (acc[t] || 0) + 1;
      return acc;
    }, {} as Record<string, number>);
    
    const dominantType = Object.entries(typeCounts).sort((a, b) => b[1] - a[1])[0];
    if (dominantType && dominantType[1] > typeChecks.length * 0.8) {
      type = dominantType[0] as ColumnStats['type'];
    }
    
    // Calculate statistics
    const stats: ColumnStats = {
      name: col,
      type,
      totalCount: values.length,
      nullCount: values.filter(v => v === null || v === undefined || v === '').length,
      uniqueCount: new Set(values.map(v => JSON.stringify(v))).size,
      topValues: [],
    };
    
    // For numeric columns, calculate additional stats
    if (type === 'number') {
      const numericValues = nonNullValues.map(v => Number(v)).filter(n => !isNaN(n));
      if (numericValues.length > 0) {
        stats.min = Math.min(...numericValues);
        stats.max = Math.max(...numericValues);
        stats.sum = numericValues.reduce((a, b) => a + b, 0);
        stats.avg = stats.sum / numericValues.length;
      }
    } else if (type === 'string' || type === 'date') {
      const sortedValues = nonNullValues.map(String).sort();
      if (sortedValues.length > 0) {
        stats.min = sortedValues[0];
        stats.max = sortedValues[sortedValues.length - 1];
      }
    }
    
    // Top values
    const valueCounts = new Map<string, number>();
    values.forEach(v => {
      const key = v === null || v === undefined ? 'NULL' : String(v);
      valueCounts.set(key, (valueCounts.get(key) || 0) + 1);
    });
    
    stats.topValues = Array.from(valueCounts.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)
      .map(([value, count]) => ({ value, count }));
    
    return stats;
  });
});

const dataQualityIssues = computed<DataQualityIssue[]>(() => {
  const issues: DataQualityIssue[] = [];
  
  columnStats.value.forEach(stat => {
    // Check for high null percentage
    const nullPercentage = (stat.nullCount / stat.totalCount) * 100;
    if (nullPercentage > 50) {
      issues.push({
        type: 'warning',
        column: stat.name,
        message: `${nullPercentage.toFixed(1)}% of values are NULL/empty`,
      });
    } else if (nullPercentage > 10) {
      issues.push({
        type: 'info',
        column: stat.name,
        message: `${nullPercentage.toFixed(1)}% of values are NULL/empty`,
      });
    }
    
    // Check for potential unique identifiers
    if (stat.uniqueCount === stat.totalCount && stat.nullCount === 0) {
      issues.push({
        type: 'info',
        column: stat.name,
        message: 'All values are unique - possible primary key/identifier',
      });
    }
    
    // Check for low cardinality
    if (stat.uniqueCount <= 5 && stat.totalCount > 10) {
      issues.push({
        type: 'info',
        column: stat.name,
        message: `Low cardinality (${stat.uniqueCount} unique values) - consider as categorical`,
      });
    }
    
    // Check for potential outliers in numeric columns
    if (stat.type === 'number' && stat.avg !== undefined && stat.min !== undefined && stat.max !== undefined) {
      const range = (stat.max as number) - (stat.min as number);
      const deviation = range / stat.avg;
      if (deviation > 10) {
        issues.push({
          type: 'warning',
          column: stat.name,
          message: 'High variance detected - potential outliers',
        });
      }
    }
  });
  
  return issues;
});

const overviewStats = computed(() => {
  return {
    totalRows: parsedData.value.length,
    totalColumns: props.columns.length,
    numericColumns: columnStats.value.filter(c => c.type === 'number').length,
    textColumns: columnStats.value.filter(c => c.type === 'string').length,
    dateColumns: columnStats.value.filter(c => c.type === 'date').length,
    completeness: columnStats.value.length > 0 
      ? ((1 - columnStats.value.reduce((acc, c) => acc + c.nullCount, 0) / 
          (columnStats.value.reduce((acc, c) => acc + c.totalCount, 0) || 1)) * 100).toFixed(1)
      : '100',
  };
});

function isDateString(str: string): boolean {
  // Simple date detection
  const datePatterns = [
    /^\d{4}-\d{2}-\d{2}/, // ISO date
    /^\d{2}\/\d{2}\/\d{4}/, // US date
    /^\d{2}-\d{2}-\d{4}/, // EU date
  ];
  return datePatterns.some(p => p.test(str));
}

function getTypeIcon(type: string): string {
  switch (type) {
    case 'number': return 'tag';
    case 'string': return 'text_fields';
    case 'boolean': return 'toggle_on';
    case 'date': return 'schedule';
    default: return 'category';
  }
}

function getTypeColor(type: string): string {
  switch (type) {
    case 'number': return 'text-blue-400 bg-blue-500/10';
    case 'string': return 'text-emerald-400 bg-emerald-500/10';
    case 'boolean': return 'text-purple-400 bg-purple-500/10';
    case 'date': return 'text-amber-400 bg-amber-500/10';
    default: return 'text-gray-400 bg-gray-500/10';
  }
}

function formatNumber(num: number | undefined): string {
  if (num === undefined) return '-';
  if (Number.isInteger(num)) return num.toLocaleString();
  return num.toLocaleString(undefined, { maximumFractionDigits: 2 });
}

function selectColumn(name: string) {
  selectedColumn.value = selectedColumn.value === name ? null : name;
}

const selectedColumnStats = computed(() => {
  if (!selectedColumn.value) return null;
  return columnStats.value.find(c => c.name === selectedColumn.value);
});

watch(() => props.data, () => {
  selectedColumn.value = null;
});
</script>

<template>
  <div class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50" @click.self="emit('close')">
    <div class="bg-surface-dark border border-[#3d4f51] rounded-2xl w-full max-w-5xl h-[80vh] shadow-2xl overflow-hidden flex flex-col">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-4 border-b border-[#2a3637] shrink-0">
        <div class="flex items-center gap-3">
          <span class="material-symbols-outlined text-primary">analytics</span>
          <h2 class="text-lg font-semibold text-white">Data Insights</h2>
          <span class="text-xs text-[#5d6f71] bg-[#2a3637] px-2 py-0.5 rounded">
            {{ overviewStats.totalRows.toLocaleString() }} rows · {{ overviewStats.totalColumns }} columns
          </span>
        </div>
        <button
          @click="emit('close')"
          class="p-1.5 hover:bg-white/10 rounded-lg transition-colors"
        >
          <span class="material-symbols-outlined text-[#9fb4b7]">close</span>
        </button>
      </div>

      <!-- Tabs -->
      <div class="flex gap-1 px-6 pt-4 border-b border-[#2a3637] shrink-0">
        <button
          @click="activeTab = 'overview'"
          class="px-4 py-2 text-sm font-medium rounded-t-lg transition-colors border-b-2 -mb-[2px]"
          :class="activeTab === 'overview' ? 'text-primary border-primary bg-primary/10' : 'text-[#9fb4b7] border-transparent hover:text-white'"
        >
          Overview
        </button>
        <button
          @click="activeTab = 'columns'"
          class="px-4 py-2 text-sm font-medium rounded-t-lg transition-colors border-b-2 -mb-[2px]"
          :class="activeTab === 'columns' ? 'text-primary border-primary bg-primary/10' : 'text-[#9fb4b7] border-transparent hover:text-white'"
        >
          Column Analysis
        </button>
        <button
          @click="activeTab = 'quality'"
          class="px-4 py-2 text-sm font-medium rounded-t-lg transition-colors border-b-2 -mb-[2px] flex items-center gap-2"
          :class="activeTab === 'quality' ? 'text-primary border-primary bg-primary/10' : 'text-[#9fb4b7] border-transparent hover:text-white'"
        >
          Data Quality
          <span v-if="dataQualityIssues.length > 0" class="text-[10px] bg-amber-500/20 text-amber-400 px-1.5 py-0.5 rounded-full">
            {{ dataQualityIssues.length }}
          </span>
        </button>
      </div>

      <!-- Content -->
      <div class="flex-1 overflow-y-auto p-6">
        <!-- Overview Tab -->
        <div v-if="activeTab === 'overview'" class="space-y-6">
          <!-- Stats cards -->
          <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div class="bg-background-dark rounded-xl p-4 border border-[#2a3637]">
              <div class="flex items-center gap-2 text-[#9fb4b7] mb-2">
                <span class="material-symbols-outlined text-[18px]">table_rows</span>
                <span class="text-xs uppercase tracking-wider">Total Rows</span>
              </div>
              <p class="text-2xl font-bold text-white">{{ overviewStats.totalRows.toLocaleString() }}</p>
            </div>
            
            <div class="bg-background-dark rounded-xl p-4 border border-[#2a3637]">
              <div class="flex items-center gap-2 text-[#9fb4b7] mb-2">
                <span class="material-symbols-outlined text-[18px]">view_column</span>
                <span class="text-xs uppercase tracking-wider">Columns</span>
              </div>
              <p class="text-2xl font-bold text-white">{{ overviewStats.totalColumns }}</p>
            </div>
            
            <div class="bg-background-dark rounded-xl p-4 border border-[#2a3637]">
              <div class="flex items-center gap-2 text-[#9fb4b7] mb-2">
                <span class="material-symbols-outlined text-[18px]">check_circle</span>
                <span class="text-xs uppercase tracking-wider">Data Completeness</span>
              </div>
              <p class="text-2xl font-bold text-white">{{ overviewStats.completeness }}%</p>
            </div>
            
            <div class="bg-background-dark rounded-xl p-4 border border-[#2a3637]">
              <div class="flex items-center gap-2 text-[#9fb4b7] mb-2">
                <span class="material-symbols-outlined text-[18px]">warning</span>
                <span class="text-xs uppercase tracking-wider">Quality Issues</span>
              </div>
              <p class="text-2xl font-bold" :class="dataQualityIssues.length > 0 ? 'text-amber-400' : 'text-emerald-400'">
                {{ dataQualityIssues.length }}
              </p>
            </div>
          </div>

          <!-- Column type distribution -->
          <div class="bg-background-dark rounded-xl p-5 border border-[#2a3637]">
            <h3 class="text-white font-medium mb-4">Column Types Distribution</h3>
            <div class="flex gap-6">
              <div class="flex items-center gap-2">
                <span class="w-3 h-3 rounded bg-blue-500"></span>
                <span class="text-sm text-[#9fb4b7]">Numeric: {{ overviewStats.numericColumns }}</span>
              </div>
              <div class="flex items-center gap-2">
                <span class="w-3 h-3 rounded bg-emerald-500"></span>
                <span class="text-sm text-[#9fb4b7]">Text: {{ overviewStats.textColumns }}</span>
              </div>
              <div class="flex items-center gap-2">
                <span class="w-3 h-3 rounded bg-amber-500"></span>
                <span class="text-sm text-[#9fb4b7]">Date/Time: {{ overviewStats.dateColumns }}</span>
              </div>
            </div>
            
            <!-- Simple bar visualization -->
            <div class="mt-4 h-4 rounded-full overflow-hidden bg-[#2a3637] flex">
              <div 
                v-if="overviewStats.numericColumns > 0"
                class="bg-blue-500 h-full" 
                :style="{ width: `${(overviewStats.numericColumns / overviewStats.totalColumns) * 100}%` }"
              ></div>
              <div 
                v-if="overviewStats.textColumns > 0"
                class="bg-emerald-500 h-full" 
                :style="{ width: `${(overviewStats.textColumns / overviewStats.totalColumns) * 100}%` }"
              ></div>
              <div 
                v-if="overviewStats.dateColumns > 0"
                class="bg-amber-500 h-full" 
                :style="{ width: `${(overviewStats.dateColumns / overviewStats.totalColumns) * 100}%` }"
              ></div>
            </div>
          </div>

          <!-- Quick stats per column -->
          <div class="bg-background-dark rounded-xl border border-[#2a3637] overflow-hidden">
            <div class="px-5 py-3 border-b border-[#2a3637]">
              <h3 class="text-white font-medium">Column Summary</h3>
            </div>
            <div class="divide-y divide-[#2a3637]">
              <div 
                v-for="stat in columnStats" 
                :key="stat.name"
                class="px-5 py-3 flex items-center gap-4 hover:bg-white/5 cursor-pointer transition-colors"
                @click="activeTab = 'columns'; selectColumn(stat.name)"
              >
                <span 
                  class="material-symbols-outlined text-[18px] p-1.5 rounded"
                  :class="getTypeColor(stat.type)"
                >
                  {{ getTypeIcon(stat.type) }}
                </span>
                <div class="flex-1 min-w-0">
                  <p class="text-white font-medium text-sm truncate">{{ stat.name }}</p>
                  <p class="text-xs text-[#5d6f71]">{{ stat.type }} · {{ stat.uniqueCount }} unique</p>
                </div>
                <div class="text-right">
                  <p class="text-sm text-white">{{ ((1 - stat.nullCount / stat.totalCount) * 100).toFixed(0) }}%</p>
                  <p class="text-xs text-[#5d6f71]">complete</p>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Columns Tab -->
        <div v-else-if="activeTab === 'columns'" class="flex gap-6 h-full">
          <!-- Column list -->
          <div class="w-64 shrink-0 space-y-2">
            <div
              v-for="stat in columnStats"
              :key="stat.name"
              @click="selectColumn(stat.name)"
              class="p-3 rounded-lg cursor-pointer transition-colors border"
              :class="selectedColumn === stat.name ? 'bg-primary/20 border-primary/30' : 'bg-background-dark border-[#2a3637] hover:bg-white/5'"
            >
              <div class="flex items-center gap-2">
                <span 
                  class="material-symbols-outlined text-[16px]"
                  :class="selectedColumn === stat.name ? 'text-primary' : getTypeColor(stat.type).split(' ')[0]"
                >
                  {{ getTypeIcon(stat.type) }}
                </span>
                <span class="text-sm font-medium truncate" :class="selectedColumn === stat.name ? 'text-white' : 'text-[#9fb4b7]'">
                  {{ stat.name }}
                </span>
              </div>
            </div>
          </div>

          <!-- Column details -->
          <div class="flex-1">
            <div v-if="!selectedColumnStats" class="h-full flex items-center justify-center text-[#5d6f71]">
              <div class="text-center">
                <span class="material-symbols-outlined text-4xl mb-2">touch_app</span>
                <p>Select a column to view details</p>
              </div>
            </div>

            <div v-else class="space-y-4">
              <!-- Column header -->
              <div class="flex items-center gap-3 mb-6">
                <span 
                  class="material-symbols-outlined text-[24px] p-2 rounded-lg"
                  :class="getTypeColor(selectedColumnStats.type)"
                >
                  {{ getTypeIcon(selectedColumnStats.type) }}
                </span>
                <div>
                  <h3 class="text-xl font-semibold text-white">{{ selectedColumnStats.name }}</h3>
                  <p class="text-sm text-[#9fb4b7] capitalize">{{ selectedColumnStats.type }} column</p>
                </div>
              </div>

              <!-- Stats grid -->
              <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
                <div class="bg-background-dark rounded-lg p-3 border border-[#2a3637]">
                  <p class="text-xs text-[#5d6f71] uppercase mb-1">Total Values</p>
                  <p class="text-lg font-semibold text-white">{{ selectedColumnStats.totalCount.toLocaleString() }}</p>
                </div>
                <div class="bg-background-dark rounded-lg p-3 border border-[#2a3637]">
                  <p class="text-xs text-[#5d6f71] uppercase mb-1">Unique Values</p>
                  <p class="text-lg font-semibold text-white">{{ selectedColumnStats.uniqueCount.toLocaleString() }}</p>
                </div>
                <div class="bg-background-dark rounded-lg p-3 border border-[#2a3637]">
                  <p class="text-xs text-[#5d6f71] uppercase mb-1">NULL Count</p>
                  <p class="text-lg font-semibold" :class="selectedColumnStats.nullCount > 0 ? 'text-amber-400' : 'text-white'">
                    {{ selectedColumnStats.nullCount.toLocaleString() }}
                  </p>
                </div>
                <div class="bg-background-dark rounded-lg p-3 border border-[#2a3637]">
                  <p class="text-xs text-[#5d6f71] uppercase mb-1">Completeness</p>
                  <p class="text-lg font-semibold text-white">
                    {{ ((1 - selectedColumnStats.nullCount / selectedColumnStats.totalCount) * 100).toFixed(1) }}%
                  </p>
                </div>
              </div>

              <!-- Numeric stats -->
              <div v-if="selectedColumnStats.type === 'number'" class="grid grid-cols-2 md:grid-cols-4 gap-3">
                <div class="bg-background-dark rounded-lg p-3 border border-[#2a3637]">
                  <p class="text-xs text-[#5d6f71] uppercase mb-1">Min</p>
                  <p class="text-lg font-semibold text-white font-mono">{{ formatNumber(selectedColumnStats.min as number) }}</p>
                </div>
                <div class="bg-background-dark rounded-lg p-3 border border-[#2a3637]">
                  <p class="text-xs text-[#5d6f71] uppercase mb-1">Max</p>
                  <p class="text-lg font-semibold text-white font-mono">{{ formatNumber(selectedColumnStats.max as number) }}</p>
                </div>
                <div class="bg-background-dark rounded-lg p-3 border border-[#2a3637]">
                  <p class="text-xs text-[#5d6f71] uppercase mb-1">Average</p>
                  <p class="text-lg font-semibold text-white font-mono">{{ formatNumber(selectedColumnStats.avg) }}</p>
                </div>
                <div class="bg-background-dark rounded-lg p-3 border border-[#2a3637]">
                  <p class="text-xs text-[#5d6f71] uppercase mb-1">Sum</p>
                  <p class="text-lg font-semibold text-white font-mono">{{ formatNumber(selectedColumnStats.sum) }}</p>
                </div>
              </div>

              <!-- Top values -->
              <div class="bg-background-dark rounded-lg border border-[#2a3637] overflow-hidden">
                <div class="px-4 py-3 border-b border-[#2a3637]">
                  <h4 class="text-sm font-medium text-white">Top Values</h4>
                </div>
                <div class="divide-y divide-[#2a3637]">
                  <div 
                    v-for="(item, idx) in selectedColumnStats.topValues" 
                    :key="idx"
                    class="px-4 py-2 flex items-center justify-between"
                  >
                    <span class="text-sm text-white truncate max-w-[300px]" :class="item.value === 'NULL' ? 'italic text-[#5d6f71]' : ''">
                      {{ item.value }}
                    </span>
                    <div class="flex items-center gap-3">
                      <div class="w-24 h-1.5 rounded-full bg-[#2a3637] overflow-hidden">
                        <div 
                          class="h-full bg-primary rounded-full"
                          :style="{ width: `${(item.count / selectedColumnStats.totalCount) * 100}%` }"
                        ></div>
                      </div>
                      <span class="text-xs text-[#9fb4b7] w-16 text-right">
                        {{ item.count.toLocaleString() }} ({{ ((item.count / selectedColumnStats.totalCount) * 100).toFixed(1) }}%)
                      </span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Quality Tab -->
        <div v-else-if="activeTab === 'quality'" class="space-y-4">
          <div v-if="dataQualityIssues.length === 0" class="text-center py-12">
            <span class="material-symbols-outlined text-4xl text-emerald-400 mb-2">check_circle</span>
            <p class="text-white font-medium">No data quality issues detected</p>
            <p class="text-sm text-[#5d6f71]">Your data looks healthy</p>
          </div>

          <div v-else class="space-y-3">
            <div
              v-for="(issue, idx) in dataQualityIssues"
              :key="idx"
              class="p-4 rounded-lg border flex items-start gap-3"
              :class="{
                'bg-red-500/10 border-red-500/30': issue.type === 'error',
                'bg-amber-500/10 border-amber-500/30': issue.type === 'warning',
                'bg-blue-500/10 border-blue-500/30': issue.type === 'info',
              }"
            >
              <span 
                class="material-symbols-outlined text-[20px] mt-0.5"
                :class="{
                  'text-red-400': issue.type === 'error',
                  'text-amber-400': issue.type === 'warning',
                  'text-blue-400': issue.type === 'info',
                }"
              >
                {{ issue.type === 'error' ? 'error' : issue.type === 'warning' ? 'warning' : 'info' }}
              </span>
              <div>
                <p class="text-white font-medium text-sm">{{ issue.column }}</p>
                <p class="text-sm" :class="{
                  'text-red-300': issue.type === 'error',
                  'text-amber-300': issue.type === 'warning',
                  'text-blue-300': issue.type === 'info',
                }">{{ issue.message }}</p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
