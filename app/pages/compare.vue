<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Button } from '~/components/ui/button'
import { Label } from '~/components/ui/label'
import { Textarea } from '~/components/ui/textarea'
import { useConnectionsStore } from '~/stores/connections'
import { compareSchemas, migrationDraft } from '~/lib/schemaDiff'
import type { ComparisonSchema } from '~/lib/schemaDiff'

useHead({ title: 'Schema comparison' })
const store = useConnectionsStore()
const connections = computed(() => store.connections.filter(connection => ['postgres', 'mysql', 'mariadb', 'sqlite'].includes(connection.db_type)))
const sourceId = ref('')
const targetId = ref('')
const source = ref<ComparisonSchema | null>(null)
const target = ref<ComparisonSchema | null>(null)
const sourceSchema = ref('')
const targetSchema = ref('')
const loading = ref(false)
const error = ref('')
let revision = 0
const schemas = (snapshot: ComparisonSchema | null) => [...new Set(snapshot?.tables.map(row => row.table.schema) ?? [])]
const sourceSchemas = computed(() => schemas(source.value))
const targetSchemas = computed(() => schemas(target.value))
const changes = computed(() => source.value && target.value ? compareSchemas(source.value, target.value, sourceSchema.value || null, targetSchema.value || null) : [])
const draft = computed(() => migrationDraft(changes.value))
const ready = computed(() => source.value !== null && target.value !== null)
watch([sourceId, targetId], () => { ++revision; source.value = null; target.value = null; error.value = ''; loading.value = false })
onBeforeUnmount(() => { ++revision })
onMounted(() => { if (!store.connections.length) store.loadConnections() })
async function loadComparison() {
  if (!sourceId.value || !targetId.value || loading.value) return
  const sourceConnection = connections.value.find(connection => connection.id === sourceId.value)
  const targetConnection = connections.value.find(connection => connection.id === targetId.value)
  const family = (engine?: string) => engine === 'mariadb' ? 'mysql' : engine
  if (!sourceConnection || !targetConnection || family(sourceConnection.db_type) !== family(targetConnection.db_type)) { error.value = 'Choose connections using the same SQL engine family'; return }
  const requestRevision = ++revision
  loading.value = true; error.value = ''; source.value = null; target.value = null
  try {
    const [from, to] = await Promise.all([
      invoke<ComparisonSchema>('get_comparison_schema', { connectionId: sourceId.value }),
      invoke<ComparisonSchema>('get_comparison_schema', { connectionId: targetId.value }),
    ])
    if (requestRevision !== revision) return
    // Validate family before assigning snapshots so a stale server response cannot break rendering.
    compareSchemas(from, to, null, null)
    sourceSchema.value = schemas(from)[0] ?? ''; targetSchema.value = schemas(to)[0] ?? ''
    source.value = from; target.value = to
  }
  catch (cause) { if (requestRevision === revision) error.value = String(cause) }
  finally { if (requestRevision === revision) loading.value = false }
}
async function copyDraft() {
  try { await navigator.clipboard.writeText(draft.value); toast.success('Migration draft copied') }
  catch (cause) { toast.error('Could not copy draft', { description: String(cause) }) }
}
</script>
<template>
  <div class="flex flex-col gap-6 p-6">
    <div><h1 class="text-xl font-semibold">Schema comparison</h1><p class="mt-1 text-sm text-muted-foreground">Compare a desired source with the current target. No database changes are executed.</p></div>
    <div class="grid max-w-4xl gap-4 sm:grid-cols-2">
      <div class="space-y-2"><Label for="compare-source">Desired source connection</Label><select id="compare-source" v-model="sourceId" :disabled="loading" class="h-9 w-full rounded-md bg-background px-3 text-sm ring-1 ring-border"><option value="">Choose a SQL connection</option><option v-for="connection in connections" :key="connection.id" :value="connection.id">{{ connection.name }} ({{ connection.db_type }})</option></select></div>
      <div class="space-y-2"><Label for="compare-target">Current target connection</Label><select id="compare-target" v-model="targetId" :disabled="loading" class="h-9 w-full rounded-md bg-background px-3 text-sm ring-1 ring-border"><option value="">Choose a SQL connection</option><option v-for="connection in connections" :key="connection.id" :value="connection.id">{{ connection.name }} ({{ connection.db_type }})</option></select></div>
    </div>
    <div><Button :disabled="loading || !sourceId || !targetId" @click="loadComparison">{{ loading ? 'Reading schemas…' : ready ? 'Refresh comparison' : 'Compare schemas' }}</Button></div>
    <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
    <template v-if="ready">
      <div class="grid max-w-4xl gap-4 sm:grid-cols-2">
        <div class="space-y-2"><Label for="compare-source-schema">Source schema</Label><select id="compare-source-schema" v-model="sourceSchema" class="h-9 w-full rounded-md bg-background px-3 text-sm ring-1 ring-border"><option v-if="!sourceSchemas.length" value="">Empty database</option><option v-for="schema in sourceSchemas" :key="schema ?? ''" :value="schema ?? ''">{{ schema ?? 'SQLite main' }}</option></select></div>
        <div class="space-y-2"><Label for="compare-target-schema">Target schema</Label><select id="compare-target-schema" v-model="targetSchema" class="h-9 w-full rounded-md bg-background px-3 text-sm ring-1 ring-border"><option v-if="!targetSchemas.length" value="">Empty database</option><option v-for="schema in targetSchemas" :key="schema ?? ''" :value="schema ?? ''">{{ schema ?? 'SQLite main' }}</option></select></div>
      </div>
      <p class="text-sm text-muted-foreground">Matches tables and columns by name within the selected schemas. Compares native types, nullability, defaults, and available key flags. Indexes, complete constraints and foreign-key destinations, generated expressions, views, triggers, permissions, and column order are outside this comparison. A matching result does not establish full schema parity.</p>
      <p role="status" class="text-sm">{{ changes.length ? `${changes.length} column/table differences` : 'No differences in the compared metadata' }}</p>
      <div v-if="changes.length" class="max-h-[50dvh] overflow-auto"><table class="w-full text-sm"><thead class="sticky top-0 bg-background"><tr><th scope="col" class="p-3 text-left">Object</th><th scope="col" class="p-3 text-left">Change to target</th><th scope="col" class="p-3 text-left">Desired source</th><th scope="col" class="p-3 text-left">Current target</th></tr></thead><tbody><tr v-for="(change, index) in changes" :key="index" class="odd:bg-muted/30"><td class="p-3 align-top whitespace-pre-wrap break-words">{{ change.table }}{{ change.column === null ? '' : ` / ${change.column}` }}</td><td class="p-3 align-top">{{ change.kind }}{{ change.sql ? '' : ' (manual review)' }}</td><td class="p-3 align-top whitespace-pre-wrap break-words">{{ change.source }}</td><td class="p-3 align-top whitespace-pre-wrap break-words">{{ change.target }}</td></tr></tbody></table></div>
      <div v-if="changes.length" class="flex flex-col gap-3"><Label for="migration-draft">Migration draft for the target</Label><p class="text-sm text-muted-foreground">Only ordinary nullable columns without defaults or key flags generate ADD COLUMN statements. Review every statement and missing constraint in your migration workflow. Drops, new tables, and other changes are manual review items.</p><Textarea id="migration-draft" :model-value="draft" readonly rows="10" spellcheck="false" class="font-mono"/><div><Button variant="secondary" @click="copyDraft">Copy draft</Button></div></div>
    </template>
  </div>
</template>
