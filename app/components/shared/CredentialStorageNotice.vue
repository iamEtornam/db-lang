<script setup lang="ts">
import { invoke, isTauri } from '@tauri-apps/api/core'
import { Button } from '~/components/ui/button'
import { useConnectionsStore } from '~/stores/connections'

const error = ref('')
const busy = ref(false)

onMounted(async () => {
  if (!isTauri()) return
  try {
    const status = await invoke<{ ready: boolean, error: string | null }>('credential_storage_status')
    if (!status.ready) error.value = status.error || 'Credential storage is unavailable.'
  }
  catch (err) { error.value = String(err) }
})

async function retry() {
  busy.value = true
  try {
    await invoke('retry_credential_storage')
    await useConnectionsStore().loadConnections()
    error.value = ''
    // Reload dependent settings and schema after initialization completes.
    window.location.reload()
  }
  catch (err) { error.value = String(err) }
  finally { busy.value = false }
}
</script>

<template>
  <section v-if="error" role="alert" class="shrink-0 space-y-2 rounded-md bg-secondary p-4 mb-4">
    <h2 class="text-sm font-semibold">Unlock credential storage</h2>
    <p class="text-sm break-words">{{ error }}</p>
    <p class="text-xs text-muted-foreground">Saved connections stay unavailable until the original OS credential key can be read.</p>
    <Button variant="secondary" :disabled="busy" @click="retry">{{ busy ? 'Retrying…' : 'Retry credential storage' }}</Button>
  </section>
</template>
