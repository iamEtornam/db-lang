<script setup lang="ts">
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const props = defineProps<{
  data: string;
  columns: string[];
}>();

const emit = defineEmits<{
  close: [];
}>();

interface ExportResult {
  data: string;
  is_binary: boolean;
  filename_extension: string;
}

const format = ref<'csv' | 'json' | 'tsv' | 'xlsx'>('csv');
const selectedColumns = ref<string[]>([...props.columns]);
const includeHeaders = ref(true);
const sheetName = ref('Query Results');
const isExporting = ref(false);
const error = ref<string | null>(null);

const allSelected = computed(() => selectedColumns.value.length === props.columns.length);

const formatInfo: Record<string, { label: string; icon: string; description: string }> = {
  csv: { label: 'CSV', icon: 'description', description: 'Comma-separated values' },
  json: { label: 'JSON', icon: 'data_object', description: 'JavaScript Object Notation' },
  tsv: { label: 'TSV', icon: 'table_rows', description: 'Tab-separated values' },
  xlsx: { label: 'Excel', icon: 'table_chart', description: 'Microsoft Excel workbook' },
};

function toggleAll() {
  if (allSelected.value) {
    selectedColumns.value = [];
  } else {
    selectedColumns.value = [...props.columns];
  }
}

function toggleColumn(column: string) {
  const index = selectedColumns.value.indexOf(column);
  if (index === -1) {
    selectedColumns.value.push(column);
  } else {
    selectedColumns.value.splice(index, 1);
  }
}

function base64ToArrayBuffer(base64: string): ArrayBuffer {
  const binaryString = window.atob(base64);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  return bytes.buffer;
}

async function handleExport() {
  if (selectedColumns.value.length === 0) {
    error.value = 'Please select at least one column';
    return;
  }

  isExporting.value = true;
  error.value = null;

  try {
    const result = await invoke<ExportResult>('export_data', {
      dataJson: props.data,
      format: format.value,
      columns: selectedColumns.value,
      includeHeaders: includeHeaders.value,
      delimiter: format.value === 'tsv' ? '\t' : ',',
      sheetName: format.value === 'xlsx' ? sheetName.value : null,
    });

    // Create download based on whether data is binary
    let blob: Blob;
    
    if (result.is_binary) {
      // Decode base64 for binary files (xlsx)
      const arrayBuffer = base64ToArrayBuffer(result.data);
      blob = new Blob([arrayBuffer], { 
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet' 
      });
    } else {
      // Text files (csv, json, tsv)
      const mimeTypes: Record<string, string> = {
        csv: 'text/csv',
        json: 'application/json',
        tsv: 'text/tab-separated-values',
      };
      blob = new Blob([result.data], { 
        type: mimeTypes[result.filename_extension] || 'text/plain'
      });
    }
    
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `export_${new Date().toISOString().slice(0, 10)}.${result.filename_extension}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    emit('close');
  } catch (err) {
    error.value = err as string;
  } finally {
    isExporting.value = false;
  }
}
</script>

<template>
  <div class="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4" @click.self="emit('close')">
    <div class="w-full max-w-md bg-surface-dark rounded-xl shadow-[0_20px_70px_rgba(0,0,0,0.55)] overflow-hidden border border-[#2a3637]">
      <!-- Header -->
      <header class="flex items-center justify-between px-6 py-4 border-b border-[#2a3637]">
        <div class="flex items-center gap-3">
          <span class="material-symbols-outlined text-primary text-[20px]">download</span>
          <h2 class="text-lg font-bold text-white">Export Data</h2>
        </div>
        <button @click="emit('close')" class="p-1 hover:bg-white/5 rounded-lg transition-colors">
          <span class="material-symbols-outlined text-[#9fb4b7] text-[20px]">close</span>
        </button>
      </header>

      <div class="p-6 space-y-5">
        <!-- Error -->
        <div v-if="error" class="bg-red-500/10 border border-red-500/30 rounded-lg px-4 py-3 flex items-center gap-3">
          <span class="material-symbols-outlined text-red-400 text-lg">error</span>
          <p class="text-red-400 text-sm">{{ error }}</p>
        </div>

        <!-- Format Selection -->
        <div>
          <label class="block text-sm font-medium text-white mb-2">Export Format</label>
          <div class="grid grid-cols-2 gap-2">
            <button
              v-for="(info, key) in formatInfo"
              :key="key"
              @click="format = key as any"
              class="flex items-center gap-3 px-4 py-3 rounded-lg text-sm font-medium transition-colors text-left"
              :class="format === key ? 'bg-primary text-white' : 'bg-[#1d2526] text-[#9fb4b7] hover:text-white border border-[#3d4f51] hover:border-primary/50'"
            >
              <span class="material-symbols-outlined text-[20px]">{{ info.icon }}</span>
              <div>
                <p class="font-medium">{{ info.label }}</p>
                <p class="text-[10px] opacity-70">{{ info.description }}</p>
              </div>
            </button>
          </div>
        </div>

        <!-- Sheet Name (xlsx only) -->
        <div v-if="format === 'xlsx'">
          <label class="block text-sm font-medium text-white mb-2">Sheet Name</label>
          <input
            v-model="sheetName"
            type="text"
            class="w-full bg-[#1d2526] border border-[#3d4f51] rounded-lg px-4 py-2 text-white text-sm focus:outline-none focus:border-primary"
            placeholder="Query Results"
          />
        </div>

        <!-- Column Selection -->
        <div>
          <div class="flex items-center justify-between mb-2">
            <label class="text-sm font-medium text-white">Columns</label>
            <button
              @click="toggleAll"
              class="text-xs text-primary hover:text-primary/80 font-medium"
            >
              {{ allSelected ? 'Deselect All' : 'Select All' }}
            </button>
          </div>
          <div class="max-h-40 overflow-y-auto bg-[#1d2526] rounded-lg border border-[#3d4f51] p-2 space-y-1">
            <label
              v-for="column in columns"
              :key="column"
              class="flex items-center gap-2 px-2 py-1.5 rounded hover:bg-white/5 cursor-pointer"
            >
              <input
                type="checkbox"
                :checked="selectedColumns.includes(column)"
                @change="toggleColumn(column)"
                class="rounded border-[#3d4f51] bg-surface-dark text-primary focus:ring-primary focus:ring-offset-0"
              />
              <span class="text-sm text-white font-mono">{{ column }}</span>
            </label>
          </div>
        </div>

        <!-- Options -->
        <div v-if="format === 'csv' || format === 'tsv'" class="flex items-center justify-between">
          <span class="text-sm text-[#9fb4b7]">Include headers</span>
          <button
            @click="includeHeaders = !includeHeaders"
            class="relative inline-block w-11 h-6 transition duration-200 ease-in"
          >
            <span
              class="absolute block w-6 h-6 rounded-full bg-white transition-all"
              :class="includeHeaders ? 'right-0 border-primary border-4' : 'left-0 border-[#3d4f51] border-4'"
            ></span>
            <span
              class="block h-6 rounded-full transition-colors"
              :class="includeHeaders ? 'bg-primary' : 'bg-[#1d2526]'"
            ></span>
          </button>
        </div>
      </div>

      <!-- Footer -->
      <footer class="px-6 py-4 bg-[#1d2526]/50 border-t border-[#2a3637] flex justify-end gap-3">
        <button
          @click="emit('close')"
          class="px-4 py-2 text-[#9fb4b7] hover:text-white text-sm font-medium transition-colors"
        >
          Cancel
        </button>
        <button
          @click="handleExport"
          :disabled="isExporting || selectedColumns.length === 0"
          class="flex items-center gap-2 px-4 py-2 bg-primary hover:bg-primary/90 rounded-lg text-white text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <span v-if="isExporting" class="material-symbols-outlined animate-spin text-[16px]">progress_activity</span>
          <span class="material-symbols-outlined text-[16px]" v-else>download</span>
          {{ isExporting ? 'Exporting...' : 'Export' }}
        </button>
      </footer>
    </div>
  </div>
</template>
