<script setup lang="ts">
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from '~/components/ui/command'
import { useConnectionsStore } from '~/stores/connections'

const open = defineModel<boolean>('open', { default: false })

const router = useRouter()
const connectionsStore = useConnectionsStore()
const { connections } = storeToRefs(connectionsStore)

function navigate(path: string) {
  router.push(path)
  open.value = false
}

function selectConnection(id: string) {
  connectionsStore.setActiveConnection(id)
  open.value = false
}
</script>

<template>
  <CommandDialog v-model:open="open">
    <CommandInput placeholder="Search connections, navigate, run actions..." />
    <CommandList>
      <CommandEmpty>No results found.</CommandEmpty>

      <CommandGroup heading="Navigation">
        <CommandItem value="query" @select="navigate('/')">
          <Icon name="lucide:terminal-square" class="size-4 mr-2" />
          <span>Query Workspace</span>
        </CommandItem>
        <CommandItem value="schema" @select="navigate('/schema')">
          <Icon name="lucide:table-2" class="size-4 mr-2" />
          <span>Schema Explorer</span>
        </CommandItem>
        <CommandItem value="history" @select="navigate('/history')">
          <Icon name="lucide:clock" class="size-4 mr-2" />
          <span>Query History</span>
        </CommandItem>
        <CommandItem value="settings" @select="navigate('/settings')">
          <Icon name="lucide:settings" class="size-4 mr-2" />
          <span>Settings</span>
        </CommandItem>
      </CommandGroup>

      <CommandSeparator v-if="connections.length > 0" />

      <CommandGroup v-if="connections.length > 0" heading="Connections">
        <CommandItem
          v-for="conn in connections"
          :key="conn.id"
          :value="`connection-${conn.id}`"
          @select="selectConnection(conn.id)"
        >
          <Icon name="lucide:database" class="size-4 mr-2" />
          <span>{{ conn.name }}</span>
          <span class="ml-auto text-xs text-muted-foreground">{{ conn.db_type }}</span>
        </CommandItem>
      </CommandGroup>
    </CommandList>
  </CommandDialog>
</template>
