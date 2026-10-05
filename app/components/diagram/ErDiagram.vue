<script setup lang="ts">
import { VueFlow, useVueFlow } from '@vue-flow/core'
import type { GraphNode } from '@vue-flow/core'
import '@vue-flow/core/dist/style.css'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import TableNode from './TableNode.vue'
import { buildErGraph, relationshipLabel } from '~/lib/er-diagram'
import type { ErSchema } from '~/types/database'

const props = defineProps<{ schema: ErSchema }>()
const { fitView, zoomIn, zoomOut, onNodesInitialized, getNodes } = useVueFlow()
const graph = buildErGraph(props.schema)
const nodes = shallowRef(graph.nodes)
const search = ref('')
const matching = computed(() => nodes.value.filter(n => n.data!.label.toLowerCase().includes(search.value.toLowerCase())))
const links = computed(() => [...new Set(props.schema.relationships.map(relationshipLabel))])
function resetLayout() {
  arrangeNodes(getNodes.value)
}
function arrangeNodes(measured: GraphNode[]) {
  const columns = Math.max(1, Math.ceil(Math.sqrt(measured.length)))
  const positions = new Map<string, { x: number; y: number }>()
  let y = 0
  for (let start = 0; start < measured.length; start += columns) {
    const row = measured.slice(start, start + columns)
    for (const [index, node] of row.entries()) positions.set(node.id, { x: index * 400, y })
    y += Math.max(...row.map(node => node.dimensions.height)) + 100
  }
  nodes.value = nodes.value.map(node => ({ ...node, position: positions.get(node.id) ?? node.position }))
  nextTick(() => fitView({ padding: 0.15, maxZoom: 1 }))
}
onNodesInitialized(arrangeNodes)
</script>

<template>
  <div class="flex flex-1 min-h-0 flex-col gap-3">
    <div class="flex flex-wrap items-center gap-2">
      <Button size="sm" variant="secondary" @click="zoomIn()">Zoom in</Button>
      <Button size="sm" variant="secondary" @click="zoomOut()">Zoom out</Button>
      <Button size="sm" variant="secondary" @click="fitView({ padding: 0.15 })">Fit diagram</Button>
      <Button size="sm" variant="ghost" @click="resetLayout">Reset layout</Button>
      <span class="ml-2 text-xs text-muted-foreground">Drag tables or the canvas. Arrows point to referenced columns.</span>
    </div>
    <div class="flex flex-1 min-h-0 gap-3 max-md:flex-col">
      <aside class="w-60 shrink-0 overflow-y-auto pr-1 max-md:w-full max-md:max-h-40" aria-label="Diagram navigation">
        <Input v-model="search" aria-label="Find a table" placeholder="Find a table…" class="mb-2" />
        <p v-if="!matching.length" class="p-2 text-sm text-muted-foreground">No matching tables</p>
        <Button v-for="node in matching" :key="node.id" variant="ghost" class="h-auto w-full justify-start whitespace-normal break-all text-left" @click="fitView({ nodes: [node.id], padding: 0.3, maxZoom: 1 })">{{ node.data!.label }}</Button>
        <details class="mt-4">
          <summary class="cursor-pointer text-sm font-medium">Foreign-key links ({{ links.length }})</summary>
          <ul class="mt-2 space-y-3 text-xs break-all">
            <li v-for="link in links" :key="link">{{ link }}</li>
          </ul>
          <p v-if="!links.length" class="mt-2 text-xs text-muted-foreground">No declared foreign keys. No relationships are inferred.</p>
        </details>
      </aside>
      <div class="er-canvas relative flex-1 min-h-64 rounded-md bg-muted/20" aria-label="Entity relationship diagram">
        <VueFlow v-model:nodes="nodes" :edges="graph.edges" :nodes-connectable="false" :edges-updatable="false" :delete-key-code="null" :min-zoom="0.05" :max-zoom="2" fit-view-on-init>
          <template #node-table="nodeProps"><TableNode v-bind="nodeProps" /></template>
        </VueFlow>
      </div>
    </div>
  </div>
</template>

<style scoped>
.er-canvas :deep(.vue-flow__edge-path) { stroke: var(--muted-foreground); stroke-width: 1.5; }
.er-canvas :deep(.vue-flow__arrowhead polyline) { fill: var(--muted-foreground); stroke: var(--muted-foreground); }
.er-canvas :deep(.vue-flow__node:focus-visible) { outline: 2px solid var(--foreground); outline-offset: 4px; }
</style>
