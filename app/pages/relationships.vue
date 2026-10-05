<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { Button } from '~/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '~/components/ui/select'
import ErDiagram from '~/components/diagram/ErDiagram.vue'
import { useConnectionsStore } from '~/stores/connections'
import { isSqlEngine } from '~/lib/sql'
import type { ErSchema } from '~/types/database'

useHead({ title: 'ER diagrams' })
const store = useConnectionsStore()
const { connections, activeConnection } = storeToRefs(store)
const schema = ref<ErSchema | null>(null)
const loading = ref(false)
const error = ref('')
let request = 0
async function loadDiagram() {
  const revision = ++request
  schema.value = null
  error.value = ''
  const connection = activeConnection.value
  loading.value = false
  if (!connection || !isSqlEngine(connection.db_type)) return
  loading.value = true
  try {
    const result = await invoke<ErSchema>('get_er_schema', { connectionId: connection.id })
    if (revision === request) schema.value = result
  }
  catch (err) {
    if (revision === request) error.value = String(err)
  }
  finally {
    if (revision === request) loading.value = false
  }
}
watch(() => activeConnection.value?.id, loadDiagram, { immediate: true })
onMounted(() => { if (!connections.value.length) store.loadConnections() })
onBeforeUnmount(() => { request++ })
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-4">
    <div class="flex flex-wrap items-center gap-3">
      <h1 class="mr-auto text-xl font-semibold">ER diagrams</h1>
      <Select :model-value="activeConnection?.id" @update:model-value="store.setActiveConnection(String($event))">
        <SelectTrigger class="w-56" aria-label="Diagram connection"><SelectValue placeholder="Choose a connection" /></SelectTrigger>
        <SelectContent><SelectItem v-for="connection in connections" :key="connection.id" :value="connection.id">{{ connection.name }}</SelectItem></SelectContent>
      </Select>
      <Button variant="secondary" :disabled="loading || !activeConnection || !isSqlEngine(activeConnection.db_type)" @click="loadDiagram">Refresh</Button>
    </div>
    <p v-if="store.error" role="alert" class="text-sm text-destructive">{{ store.error }}</p>
    <p v-if="!activeConnection" class="text-sm text-muted-foreground">Choose a saved SQL connection to explore its tables and foreign keys.</p>
    <p v-else-if="!isSqlEngine(activeConnection.db_type)" class="text-sm text-muted-foreground">ER diagrams use declared SQL foreign keys. Choose PostgreSQL, MySQL, MariaDB, or SQLite.</p>
    <p v-else-if="loading" role="status" class="text-sm text-muted-foreground">Loading tables and foreign keys…</p>
    <p v-else-if="error" role="alert" class="text-sm text-destructive">Could not load the diagram: {{ error }}. Use Refresh to retry.</p>
    <p v-else-if="schema && !schema.tables.length" class="text-sm text-muted-foreground">No tables found in this connection.</p>
    <ErDiagram v-else-if="schema" :key="request" :schema="schema" />
  </div>
</template>
