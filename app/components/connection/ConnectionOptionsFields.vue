<script setup lang="ts">
import { Input } from '~/components/ui/input'
import { Label } from '~/components/ui/label'
import type { ConnectionOptions } from '~/types/database'
const options = defineModel<ConnectionOptions>({ required: true })
const props = defineProps<{ engine: string }>()
const supportsSsh = computed(() => ['postgres', 'mysql', 'mariadb', 'mongodb', 'redis'].includes(props.engine))
function toggleSsh(event: Event) {
  options.value.ssh = (event.target as HTMLInputElement).checked ? { host: '', port: 22, username: '', identity_file: null } : null
}
</script>
<template>
  <fieldset class="space-y-3">
    <legend class="font-medium text-sm mb-2">Organization and access</legend>
    <div class="space-y-1"><Label for="conn-group">Group</Label><Input id="conn-group" v-model="options.group" maxlength="80" placeholder="e.g. Analytics" /></div>
    <div class="space-y-1"><Label for="conn-environment">Environment</Label><select id="conn-environment" v-model="options.environment" class="h-9 w-full rounded-md border border-input bg-background px-3"><option value="">Unspecified</option><option value="development">Development</option><option value="staging">Staging</option><option value="production">Production</option></select></div>
    <Label class="flex items-center gap-2"><input v-model="options.read_only" type="checkbox">Read-only connection</Label>
    <p class="text-xs text-muted-foreground">Blocks writes made through Query Studio. Use a database account with read-only permissions as well.</p>
    <Label class="flex items-center gap-2"><input type="checkbox" :checked="!!options.ssh" :disabled="!supportsSsh" @change="toggleSsh">Connect through SSH</Label>
    <p v-if="!supportsSsh" class="text-xs text-muted-foreground">SSH forwarding is available for network SQL, MongoDB, and Redis connections.</p>
    <template v-if="options.ssh">
      <p class="text-xs text-muted-foreground">Uses OpenSSH with your agent or identity file. The server must already be trusted in known_hosts. Use a single database hostname, not a MongoDB URI.</p>
      <div class="space-y-1"><Label for="ssh-host">SSH host</Label><Input id="ssh-host" v-model="options.ssh.host" /></div>
      <div class="space-y-1"><Label for="ssh-port">SSH port</Label><Input id="ssh-port" v-model.number="options.ssh.port" type="number" min="1" max="65535" /></div>
      <div class="space-y-1"><Label for="ssh-user">SSH username</Label><Input id="ssh-user" v-model="options.ssh.username" /></div>
      <div class="space-y-1"><Label for="ssh-identity">Identity file (optional absolute path)</Label><Input id="ssh-identity" :model-value="options.ssh.identity_file ?? ''" @update:model-value="options.ssh.identity_file = String($event) || null" /></div>
    </template>
  </fieldset>
</template>
