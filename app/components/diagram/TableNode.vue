<script setup lang="ts">
import { Handle, Position } from '@vue-flow/core'
import type { DiagramNodeData } from '~/lib/er-diagram'
import { columnHandle } from '~/lib/er-diagram'
defineProps<{ data: DiagramNodeData; selected?: boolean }>()
</script>

<template>
  <section class="er-table" :class="{ 'er-selected': selected }" :aria-label="data.label">
    <h2 class="px-4 py-3 font-semibold break-all">{{ data.label }}</h2>
    <p v-if="data.external" class="px-4 pb-3 text-xs text-muted-foreground">Outside the loaded schema</p>
    <p v-if="data.error" class="px-4 pb-3 text-xs text-destructive break-words">Columns unavailable: {{ data.error }}</p>
    <div v-for="column in data.columns" :key="column.name" class="relative flex items-start gap-3 px-4 py-2 text-xs">
      <Handle :id="columnHandle('target', column.name)" type="target" :position="Position.Left" :connectable="false" />
      <span class="min-w-0 flex-1 break-all font-mono">{{ column.name }}<span v-if="column.is_primary_key" class="ml-2 font-sans text-muted-foreground">PK</span><span v-if="column.is_foreign_key" class="ml-2 font-sans text-muted-foreground">FK</span></span>
      <span class="max-w-[130px] break-all text-muted-foreground">{{ column.data_type }}</span>
      <Handle :id="columnHandle('source', column.name)" type="source" :position="Position.Right" :connectable="false" />
    </div>
    <p v-if="!data.columns.length && !data.error" class="px-4 pb-3 text-xs text-muted-foreground">No columns</p>
  </section>
</template>

<style scoped>
.er-table { width: 320px; background: var(--card); border: 1px solid var(--border); border-radius: 6px; color: var(--foreground); }
.er-selected { outline: 2px solid var(--foreground); outline-offset: 3px; }
:deep(.vue-flow__handle) { width: 5px; height: 5px; background: var(--muted-foreground); border: 0; }
</style>
