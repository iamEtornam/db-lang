<script setup lang="ts">
import type { PlanNode } from '~/lib/query-plan'
defineProps<{ node: PlanNode }>()
</script>
<template>
  <li class="min-w-0">
    <details open class="rounded-md bg-muted/30 p-3">
      <summary class="cursor-pointer break-words font-medium text-sm">{{ node.operation }}<span v-if="node.relation" class="ml-2 font-mono font-normal">{{ ' ' + node.relation }}</span></summary>
      <dl v-if="node.metrics.length" class="mt-3 flex flex-wrap gap-x-6 gap-y-2 text-xs">
        <div v-for="item in node.metrics" :key="item.label"><dt class="text-muted-foreground">{{ item.label }}</dt><dd class="mt-1 font-mono">{{ item.value }}</dd></div>
      </dl>
      <details class="mt-3 text-xs"><summary class="cursor-pointer text-muted-foreground">Step details</summary><pre class="mt-2 whitespace-pre-wrap break-all font-mono">{{ JSON.stringify(node.details, null, 2) }}</pre></details>
      <ul v-if="node.children.length" class="mt-4 space-y-3 pl-4" aria-label="Child plan steps"><PlanTreeNode v-for="child in node.children" :key="child.id" :node="child" /></ul>
    </details>
  </li>
</template>
