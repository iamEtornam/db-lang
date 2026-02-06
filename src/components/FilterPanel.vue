<script setup lang="ts">
import { ref, computed, watch } from 'vue';

export interface Filter {
  id: string;
  column: string;
  operator: FilterOperator;
  value: string | number | [string | number, string | number] | null;
  enabled: boolean;
}

export type FilterOperator = 
  | 'eq' | 'ne' | 'gt' | 'lt' | 'gte' | 'lte' 
  | 'contains' | 'not_contains' | 'starts_with' | 'ends_with'
  | 'between' | 'is_null' | 'is_not_null'
  | 'in' | 'not_in';

export interface ColumnInfo {
  name: string;
  type: 'string' | 'number' | 'boolean' | 'date' | 'mixed';
}

const props = defineProps<{
  columns: ColumnInfo[];
  data: any[];
}>();

const emit = defineEmits<{
  filtersChange: [filters: Filter[]];
  close: [];
}>();

const filters = ref<Filter[]>([]);
const showAddFilter = ref(false);
const newFilterColumn = ref('');
const newFilterOperator = ref<FilterOperator>('eq');
const newFilterValue = ref<string | number>('');
const newFilterValueEnd = ref<string | number>(''); // For 'between' operator

// Filter presets stored in localStorage
const filterPresets = ref<{ name: string; filters: Filter[] }[]>([]);
const presetName = ref('');
const showSavePreset = ref(false);

// Load presets from localStorage
const PRESETS_KEY = 'query_studio_filter_presets';
try {
  const saved = localStorage.getItem(PRESETS_KEY);
  if (saved) {
    filterPresets.value = JSON.parse(saved);
  }
} catch {}

// Operator options based on column type
const operatorsByType: Record<string, { value: FilterOperator; label: string }[]> = {
  string: [
    { value: 'eq', label: 'Equals' },
    { value: 'ne', label: 'Not equals' },
    { value: 'contains', label: 'Contains' },
    { value: 'not_contains', label: 'Does not contain' },
    { value: 'starts_with', label: 'Starts with' },
    { value: 'ends_with', label: 'Ends with' },
    { value: 'is_null', label: 'Is empty' },
    { value: 'is_not_null', label: 'Is not empty' },
  ],
  number: [
    { value: 'eq', label: 'Equals' },
    { value: 'ne', label: 'Not equals' },
    { value: 'gt', label: 'Greater than' },
    { value: 'lt', label: 'Less than' },
    { value: 'gte', label: 'Greater or equal' },
    { value: 'lte', label: 'Less or equal' },
    { value: 'between', label: 'Between' },
    { value: 'is_null', label: 'Is empty' },
    { value: 'is_not_null', label: 'Is not empty' },
  ],
  date: [
    { value: 'eq', label: 'Equals' },
    { value: 'ne', label: 'Not equals' },
    { value: 'gt', label: 'After' },
    { value: 'lt', label: 'Before' },
    { value: 'gte', label: 'On or after' },
    { value: 'lte', label: 'On or before' },
    { value: 'between', label: 'Between' },
    { value: 'is_null', label: 'Is empty' },
    { value: 'is_not_null', label: 'Is not empty' },
  ],
  boolean: [
    { value: 'eq', label: 'Equals' },
    { value: 'is_null', label: 'Is empty' },
    { value: 'is_not_null', label: 'Is not empty' },
  ],
  mixed: [
    { value: 'eq', label: 'Equals' },
    { value: 'ne', label: 'Not equals' },
    { value: 'contains', label: 'Contains' },
    { value: 'is_null', label: 'Is empty' },
    { value: 'is_not_null', label: 'Is not empty' },
  ],
};

const selectedColumnInfo = computed(() => {
  return props.columns.find(c => c.name === newFilterColumn.value);
});

const availableOperators = computed(() => {
  const type = selectedColumnInfo.value?.type || 'mixed';
  return operatorsByType[type] || operatorsByType.mixed;
});

const needsValue = computed(() => {
  return !['is_null', 'is_not_null'].includes(newFilterOperator.value);
});

const needsSecondValue = computed(() => {
  return newFilterOperator.value === 'between';
});

const activeFiltersCount = computed(() => {
  return filters.value.filter(f => f.enabled).length;
});

function generateFilterId(): string {
  return `filter_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
}

function addFilter() {
  if (!newFilterColumn.value) return;
  
  let value: Filter['value'] = null;
  
  if (needsValue.value) {
    if (needsSecondValue.value) {
      value = [newFilterValue.value, newFilterValueEnd.value];
    } else {
      value = newFilterValue.value;
    }
  }
  
  const filter: Filter = {
    id: generateFilterId(),
    column: newFilterColumn.value,
    operator: newFilterOperator.value,
    value,
    enabled: true,
  };
  
  filters.value.push(filter);
  emitFilters();
  resetNewFilter();
}

function removeFilter(id: string) {
  filters.value = filters.value.filter(f => f.id !== id);
  emitFilters();
}

function toggleFilter(id: string) {
  const filter = filters.value.find(f => f.id === id);
  if (filter) {
    filter.enabled = !filter.enabled;
    emitFilters();
  }
}

function clearAllFilters() {
  filters.value = [];
  emitFilters();
}

function resetNewFilter() {
  newFilterColumn.value = '';
  newFilterOperator.value = 'eq';
  newFilterValue.value = '';
  newFilterValueEnd.value = '';
  showAddFilter.value = false;
}

function emitFilters() {
  emit('filtersChange', filters.value.filter(f => f.enabled));
}

function savePreset() {
  if (!presetName.value.trim() || filters.value.length === 0) return;
  
  filterPresets.value.push({
    name: presetName.value.trim(),
    filters: JSON.parse(JSON.stringify(filters.value)),
  });
  
  localStorage.setItem(PRESETS_KEY, JSON.stringify(filterPresets.value));
  presetName.value = '';
  showSavePreset.value = false;
}

function loadPreset(preset: { name: string; filters: Filter[] }) {
  filters.value = JSON.parse(JSON.stringify(preset.filters));
  // Generate new IDs to avoid conflicts
  filters.value.forEach(f => f.id = generateFilterId());
  emitFilters();
}

function deletePreset(index: number) {
  filterPresets.value.splice(index, 1);
  localStorage.setItem(PRESETS_KEY, JSON.stringify(filterPresets.value));
}

function getOperatorLabel(operator: FilterOperator): string {
  for (const ops of Object.values(operatorsByType)) {
    const found = ops.find(o => o.value === operator);
    if (found) return found.label;
  }
  return operator;
}

function formatFilterValue(filter: Filter): string {
  if (filter.value === null) return '';
  if (Array.isArray(filter.value)) {
    return `${filter.value[0]} - ${filter.value[1]}`;
  }
  return String(filter.value);
}

// Watch column changes to reset operator
watch(newFilterColumn, () => {
  newFilterOperator.value = 'eq';
  newFilterValue.value = '';
  newFilterValueEnd.value = '';
});

// Emit initial empty filters
emitFilters();
</script>

<template>
  <div class="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4" @click.self="emit('close')">
    <div class="w-full max-w-2xl bg-surface-dark rounded-xl shadow-[0_20px_70px_rgba(0,0,0,0.55)] overflow-hidden border border-[#2a3637] flex flex-col max-h-[80vh]">
      <!-- Header -->
      <header class="flex items-center justify-between px-6 py-4 border-b border-[#2a3637] shrink-0">
        <div class="flex items-center gap-3">
          <span class="material-symbols-outlined text-primary text-[20px]">filter_list</span>
          <h2 class="text-lg font-bold text-white">Filter Results</h2>
          <span v-if="activeFiltersCount > 0" class="text-xs bg-primary/20 text-primary px-2 py-0.5 rounded-full font-medium">
            {{ activeFiltersCount }} active
          </span>
        </div>
        <button @click="emit('close')" class="p-1 hover:bg-white/5 rounded-lg transition-colors">
          <span class="material-symbols-outlined text-[#9fb4b7] text-[20px]">close</span>
        </button>
      </header>

      <div class="flex-1 overflow-y-auto p-6 space-y-6">
        <!-- Active Filters -->
        <div v-if="filters.length > 0">
          <div class="flex items-center justify-between mb-3">
            <h3 class="text-sm font-medium text-white">Active Filters</h3>
            <button
              @click="clearAllFilters"
              class="text-xs text-red-400 hover:text-red-300 font-medium transition-colors"
            >
              Clear all
            </button>
          </div>
          
          <div class="space-y-2">
            <div
              v-for="filter in filters"
              :key="filter.id"
              class="flex items-center gap-3 p-3 bg-background-dark rounded-lg border border-[#2a3637] group"
              :class="{ 'opacity-50': !filter.enabled }"
            >
              <button
                @click="toggleFilter(filter.id)"
                class="p-1 rounded transition-colors"
                :class="filter.enabled ? 'text-primary hover:bg-primary/20' : 'text-[#5d6f71] hover:bg-white/10'"
              >
                <span class="material-symbols-outlined text-[18px]">
                  {{ filter.enabled ? 'check_box' : 'check_box_outline_blank' }}
                </span>
              </button>
              
              <div class="flex-1 flex flex-wrap items-center gap-2 text-sm">
                <span class="text-white font-medium">{{ filter.column }}</span>
                <span class="text-primary font-mono text-xs bg-primary/10 px-2 py-0.5 rounded">
                  {{ getOperatorLabel(filter.operator) }}
                </span>
                <span v-if="filter.value !== null" class="text-[#9fb4b7]">
                  "{{ formatFilterValue(filter) }}"
                </span>
              </div>
              
              <button
                @click="removeFilter(filter.id)"
                class="p-1 hover:bg-red-500/20 rounded text-[#5d6f71] hover:text-red-400 transition-colors opacity-0 group-hover:opacity-100"
              >
                <span class="material-symbols-outlined text-[16px]">close</span>
              </button>
            </div>
          </div>
        </div>

        <!-- Add Filter Section -->
        <div class="bg-background-dark rounded-xl border border-[#2a3637] p-4">
          <div v-if="!showAddFilter" class="text-center">
            <button
              @click="showAddFilter = true"
              class="flex items-center gap-2 mx-auto px-4 py-2 text-primary hover:bg-primary/10 rounded-lg transition-colors text-sm font-medium"
            >
              <span class="material-symbols-outlined text-[18px]">add</span>
              Add Filter
            </button>
          </div>
          
          <div v-else class="space-y-4">
            <div class="flex items-center justify-between">
              <h3 class="text-sm font-medium text-white">New Filter</h3>
              <button
                @click="resetNewFilter"
                class="text-xs text-[#5d6f71] hover:text-white transition-colors"
              >
                Cancel
              </button>
            </div>
            
            <!-- Column Selection -->
            <div>
              <label class="block text-xs font-medium text-[#9fb4b7] mb-1.5">Column</label>
              <select
                v-model="newFilterColumn"
                class="w-full bg-surface-dark border border-[#3d4f51] rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-primary"
              >
                <option value="" disabled>Select a column</option>
                <option v-for="col in columns" :key="col.name" :value="col.name">
                  {{ col.name }} ({{ col.type }})
                </option>
              </select>
            </div>
            
            <!-- Operator Selection -->
            <div v-if="newFilterColumn">
              <label class="block text-xs font-medium text-[#9fb4b7] mb-1.5">Condition</label>
              <select
                v-model="newFilterOperator"
                class="w-full bg-surface-dark border border-[#3d4f51] rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-primary"
              >
                <option v-for="op in availableOperators" :key="op.value" :value="op.value">
                  {{ op.label }}
                </option>
              </select>
            </div>
            
            <!-- Value Input -->
            <div v-if="newFilterColumn && needsValue" class="flex gap-3">
              <div class="flex-1">
                <label class="block text-xs font-medium text-[#9fb4b7] mb-1.5">
                  {{ needsSecondValue ? 'From' : 'Value' }}
                </label>
                <input
                  v-model="newFilterValue"
                  :type="selectedColumnInfo?.type === 'number' ? 'number' : selectedColumnInfo?.type === 'date' ? 'date' : 'text'"
                  class="w-full bg-surface-dark border border-[#3d4f51] rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-primary"
                  placeholder="Enter value..."
                />
              </div>
              
              <div v-if="needsSecondValue" class="flex-1">
                <label class="block text-xs font-medium text-[#9fb4b7] mb-1.5">To</label>
                <input
                  v-model="newFilterValueEnd"
                  :type="selectedColumnInfo?.type === 'number' ? 'number' : selectedColumnInfo?.type === 'date' ? 'date' : 'text'"
                  class="w-full bg-surface-dark border border-[#3d4f51] rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-primary"
                  placeholder="Enter value..."
                />
              </div>
            </div>
            
            <!-- Boolean Value Selection -->
            <div v-if="newFilterColumn && selectedColumnInfo?.type === 'boolean' && needsValue">
              <label class="block text-xs font-medium text-[#9fb4b7] mb-1.5">Value</label>
              <div class="flex gap-3">
                <button
                  @click="newFilterValue = 'true'"
                  class="flex-1 px-4 py-2 rounded-lg text-sm font-medium transition-colors"
                  :class="newFilterValue === 'true' ? 'bg-primary text-white' : 'bg-surface-dark border border-[#3d4f51] text-[#9fb4b7] hover:text-white'"
                >
                  True
                </button>
                <button
                  @click="newFilterValue = 'false'"
                  class="flex-1 px-4 py-2 rounded-lg text-sm font-medium transition-colors"
                  :class="newFilterValue === 'false' ? 'bg-primary text-white' : 'bg-surface-dark border border-[#3d4f51] text-[#9fb4b7] hover:text-white'"
                >
                  False
                </button>
              </div>
            </div>
            
            <!-- Add Button -->
            <button
              @click="addFilter"
              :disabled="!newFilterColumn || (needsValue && !newFilterValue)"
              class="w-full flex items-center justify-center gap-2 px-4 py-2 bg-primary hover:bg-primary/90 rounded-lg text-white text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <span class="material-symbols-outlined text-[18px]">add</span>
              Add Filter
            </button>
          </div>
        </div>

        <!-- Filter Presets -->
        <div v-if="filterPresets.length > 0 || filters.length > 0" class="border-t border-[#2a3637] pt-6">
          <div class="flex items-center justify-between mb-3">
            <h3 class="text-sm font-medium text-white">Saved Presets</h3>
            <button
              v-if="filters.length > 0 && !showSavePreset"
              @click="showSavePreset = true"
              class="text-xs text-primary hover:text-primary/80 font-medium transition-colors"
            >
              Save current as preset
            </button>
          </div>
          
          <!-- Save Preset Form -->
          <div v-if="showSavePreset" class="mb-4 flex gap-2">
            <input
              v-model="presetName"
              class="flex-1 bg-background-dark border border-[#3d4f51] rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-primary"
              placeholder="Preset name..."
              @keyup.enter="savePreset"
            />
            <button
              @click="savePreset"
              :disabled="!presetName.trim()"
              class="px-4 py-2 bg-primary hover:bg-primary/90 rounded-lg text-white text-sm font-medium transition-colors disabled:opacity-50"
            >
              Save
            </button>
            <button
              @click="showSavePreset = false; presetName = ''"
              class="px-4 py-2 text-[#9fb4b7] hover:text-white text-sm font-medium transition-colors"
            >
              Cancel
            </button>
          </div>
          
          <!-- Preset List -->
          <div v-if="filterPresets.length > 0" class="space-y-2">
            <div
              v-for="(preset, idx) in filterPresets"
              :key="idx"
              class="flex items-center gap-3 p-3 bg-background-dark rounded-lg border border-[#2a3637] hover:border-primary/30 transition-colors group"
            >
              <span class="material-symbols-outlined text-[18px] text-[#5d6f71]">bookmark</span>
              <div class="flex-1">
                <p class="text-white text-sm font-medium">{{ preset.name }}</p>
                <p class="text-[10px] text-[#5d6f71]">{{ preset.filters.length }} filter(s)</p>
              </div>
              <button
                @click="loadPreset(preset)"
                class="px-3 py-1 text-xs text-primary hover:bg-primary/10 rounded transition-colors"
              >
                Apply
              </button>
              <button
                @click="deletePreset(idx)"
                class="p-1 hover:bg-red-500/20 rounded text-[#5d6f71] hover:text-red-400 transition-colors opacity-0 group-hover:opacity-100"
              >
                <span class="material-symbols-outlined text-[14px]">delete</span>
              </button>
            </div>
          </div>
          
          <p v-else class="text-xs text-[#5d6f71] italic">No saved presets yet</p>
        </div>
      </div>

      <!-- Footer -->
      <footer class="px-6 py-4 bg-[#1d2526]/50 border-t border-[#2a3637] flex justify-between items-center shrink-0">
        <p class="text-xs text-[#5d6f71]">
          Filters are applied client-side to current results
        </p>
        <button
          @click="emit('close')"
          class="px-4 py-2 bg-primary hover:bg-primary/90 rounded-lg text-white text-sm font-medium transition-colors"
        >
          Done
        </button>
      </footer>
    </div>
  </div>
</template>
