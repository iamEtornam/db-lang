<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '~/components/ui/dialog'
import { Button } from '~/components/ui/button'
import { isSqlEngine } from '~/lib/sql'
import { planTree } from '~/lib/query-plan'
import type { PlanNode, QueryPlan } from '~/lib/query-plan'
import PlanTreeNode from './PlanTreeNode.vue'
const props = defineProps<{ connectionId: string; engine: string; query: string }>()
const open = ref(false)
const busy = ref(false)
const error = ref('')
const treeError = ref('')
const plan = shallowRef<QueryPlan | null>(null)
const tree = shallowRef<PlanNode[]>([])
let revision = 0
function clear() { revision++; busy.value = false; plan.value = null; tree.value = []; error.value = ''; treeError.value = '' }
async function inspect() {
  clear()
  const request = revision
  busy.value = true
  try {
    const result = await invoke<QueryPlan>('inspect_query_plan', { connectionId: props.connectionId, query: props.query })
    if (request !== revision) return
    plan.value = result
    try { tree.value = planTree(result) }
    catch (err) { treeError.value = `Could not display the plan tree. Read the raw output below. ${String(err)}` }
  }
  catch (err) { if (request === revision) error.value = String(err) }
  finally { if (request === revision) busy.value = false }
}
watch(open, value => { if (value) inspect(); else clear() })
watch(() => [props.connectionId, props.engine, props.query], () => { clear(); if (open.value) error.value = 'The connection or query changed. Inspect again for a current plan.' })
onBeforeUnmount(clear)
</script>
<template>
  <Button v-if="isSqlEngine(engine)" variant="ghost" :disabled="!query.trim()" @click="open = true">Query plan</Button>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-4xl max-h-[90svh] overflow-y-auto">
      <DialogHeader><DialogTitle>Query plan</DialogTitle><DialogDescription>Estimated plan from {{ engine }}. ANALYZE is disabled. Costs are planner units, not elapsed milliseconds.</DialogDescription></DialogHeader>
      <Button variant="secondary" class="w-fit" :disabled="busy || !query.trim()" @click="inspect">{{ busy ? 'Inspecting…' : 'Inspect again' }}</Button>
      <p v-if="busy" role="status" class="text-sm text-muted-foreground">Waiting for the database planner…</p>
      <p v-if="error" role="alert" class="text-sm text-destructive break-words">{{ error }}</p>
      <p v-if="treeError" role="alert" class="text-sm text-destructive break-words">{{ treeError }}</p>
      <p v-if="plan && !tree.length && !treeError" class="text-sm text-muted-foreground">The database returned no plan steps.</p>
      <ul v-if="tree.length" class="space-y-3" aria-label="Query plan steps"><PlanTreeNode v-for="node in tree" :key="node.id" :node="node" /></ul>
      <details v-if="plan" class="text-sm"><summary class="cursor-pointer font-medium">Raw plan output</summary><pre class="mt-3 text-xs font-mono whitespace-pre-wrap break-all">{{ JSON.stringify(plan.rows, null, 2) }}</pre></details>
    </DialogContent>
  </Dialog>
</template>
