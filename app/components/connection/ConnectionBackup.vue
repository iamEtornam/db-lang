<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Label } from '~/components/ui/label'
import { useConnectionsStore } from '~/stores/connections'

const store = useConnectionsStore()
const selected = ref<string[]>([])
const passphrase = ref('')
const fileInput = ref<HTMLInputElement | null>(null)
const busy = ref(false)
const message = ref('')
const error = ref('')
const canExport = computed(() => selected.value.length > 0 && Array.from(passphrase.value).length >= 12 && !busy.value)

onMounted(() => store.loadConnections())

async function exportBackup() {
  if (!canExport.value) return
  busy.value = true
  message.value = ''
  error.value = ''
  let url: string | undefined
  try {
    const contents = await invoke<string>('export_connection_profiles', {
      connectionIds: selected.value,
      passphrase: passphrase.value,
    })
    url = URL.createObjectURL(new Blob([contents], { type: 'application/json' }))
    const link = document.createElement('a')
    link.href = url
    link.download = `query-studio-connections-${new Date().toISOString().slice(0, 10)}.json`
    document.body.append(link)
    link.click()
    link.remove()
    message.value = 'Encrypted backup download started. Keep its passphrase separately.'
  }
  catch (err) {
    error.value = String(err)
  }
  finally {
    if (url) setTimeout(() => URL.revokeObjectURL(url!), 1000)
    passphrase.value = ''
    busy.value = false
  }
}

async function importBackup() {
  const file = fileInput.value?.files?.[0]
  if (!file) { error.value = 'Choose an encrypted connection backup first.'; return }
  if (Array.from(passphrase.value).length < 12) { error.value = 'Enter the backup passphrase (at least 12 characters).'; return }
  if (file.size > 10 * 1024 * 1024) { error.value = 'Backup files must be 10 MB or smaller.'; return }
  busy.value = true
  message.value = ''
  error.value = ''
  try {
    const count = await invoke<number>('import_connection_profiles', { backup: await file.text(), passphrase: passphrase.value })
    await store.loadConnections()
    if (store.error) throw new Error(`${count} connections imported, but the connection list could not refresh. Reload the app.`)
    selected.value = []
    if (fileInput.value) fileInput.value.value = ''
    message.value = `Imported ${count} connection${count === 1 ? '' : 's'} as new profiles.`
  }
  catch (err) {
    error.value = String(err)
  }
  finally {
    passphrase.value = ''
    busy.value = false
  }
}
</script>

<template>
  <section aria-labelledby="connection-backup-title" class="space-y-4">
    <h2 id="connection-backup-title" class="text-sm font-semibold">Connection backup</h2>
    <p class="text-sm text-muted-foreground">Move connections between devices using an encrypted file. Imports create new profiles and keep your existing connections.</p>
    <fieldset :disabled="busy" class="space-y-4">
      <legend class="text-sm mb-2">Connections to export</legend>
      <div v-if="store.connections.length" class="max-h-48 overflow-y-auto space-y-2 pr-2">
        <label v-for="connection in store.connections" :key="connection.id" class="flex items-center gap-2 text-sm">
          <input v-model="selected" type="checkbox" :value="connection.id" />
          <span class="break-words">{{ connection.name }} <span class="text-muted-foreground">({{ connection.db_type }})</span></span>
        </label>
      </div>
      <p v-else class="text-sm text-muted-foreground">{{ store.isLoading ? 'Loading connections…' : 'No saved connections to export.' }}</p>
      <p v-if="store.error" role="alert" class="text-sm text-destructive">{{ store.error }}</p>
      <div class="space-y-2">
        <Label for="backup-passphrase">Backup passphrase</Label>
        <Input id="backup-passphrase" v-model="passphrase" type="password" autocomplete="new-password" aria-describedby="backup-passphrase-help" />
        <p id="backup-passphrase-help" class="text-xs text-muted-foreground">Use at least 12 characters. You will need the same passphrase to import this file. AI keys, query history, and the device encryption key are excluded.</p>
      </div>
      <Button :disabled="!canExport" @click="exportBackup">Export selected connections</Button>
      <div class="space-y-2 pt-2">
        <Label for="connection-backup-file">Encrypted backup file</Label>
        <input id="connection-backup-file" ref="fileInput" type="file" accept=".json,application/json" class="block w-full text-sm file:mr-3 file:px-3 file:py-2 file:rounded-md file:border-0 file:bg-secondary file:text-secondary-foreground" />
      </div>
      <Button variant="secondary" :disabled="busy" @click="importBackup">Import connections</Button>
    </fieldset>
    <p v-if="busy" role="status" class="text-sm">Protecting connection data…</p>
    <p v-if="message" role="status" class="text-sm">{{ message }}</p>
    <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
  </section>
</template>
