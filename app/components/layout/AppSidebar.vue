<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarSeparator,
} from '~/components/ui/sidebar'
import { navigationMenus, bottomMenuItems } from '~/constants/menus'
import { getEngine } from '~/constants/engines'
import { useConnectionsStore } from '~/stores/connections'
import type { Connection } from '~/types/database'
import ConnectionDialog from '~/components/connection/ConnectionDialog.vue'

const connectionsStore = useConnectionsStore()
const { connections, activeConnection } = storeToRefs(connectionsStore)
const route = useRoute()
const router = useRouter()

const connectionStatuses = ref<Record<string, 'connected' | 'connecting' | 'error' | 'idle'>>({})

const showNewConnectionDialog = ref(false)
const editingConnection = ref<Connection | null>(null)
const showEditDialog = ref(false)

function startEdit(e: Event, conn: Connection) {
  e.stopPropagation()
  editingConnection.value = conn
  showEditDialog.value = true
}

function isActive(href: string) {
  if (href === '/') return route.path === '/'
  return route.path.startsWith(href)
}

function getEngineIcon(dbType: string): string {
  const engine = getEngine(dbType)
  return engine?.icon ?? 'lucide:database'
}

function getStatusDot(connectionId: string): { color: string; visible: boolean; label: string } {
  const status = connectionStatuses.value[connectionId]
  if (!status || status === 'idle') {
    return { color: 'bg-muted-foreground/20', visible: false, label: 'Not connected' }
  }
  switch (status) {
    case 'connected': return { color: 'bg-green-500', visible: true, label: 'Connected' }
    case 'connecting': return { color: 'bg-yellow-500 animate-pulse', visible: true, label: 'Connecting...' }
    case 'error': return { color: 'bg-red-500', visible: true, label: 'Connection failed' }
    default: return { color: 'bg-muted-foreground/20', visible: false, label: 'Not connected' }
  }
}

async function selectConnection(conn: Connection) {
  // Clicking the active connected connection = disconnect
  if (activeConnection.value?.id === conn.id && connectionStatuses.value[conn.id] === 'connected') {
    disconnectConnection(conn.id)
    return
  }

  connectionsStore.setActiveConnection(conn.id)
  connectionStatuses.value[conn.id] = 'connecting'

  try {
    await invoke<boolean>('test_connection_by_id', {
      connectionId: conn.id,
    })
    connectionStatuses.value[conn.id] = 'connected'
    toast.success(`Connected to ${conn.name}`)

    // Load all tables and columns for this connection
    const loaded = await connectionsStore.loadSchema()
    if (loaded) {
      toast.success(`Schema loaded: ${connectionsStore.tables.length} tables`)
    }

    if (route.path !== '/') {
      router.push('/')
    }
  }
  catch (err) {
    connectionStatuses.value[conn.id] = 'error'
    toast.error(`Failed to connect to ${conn.name}`, { description: err as string })
  }
}

function disconnectConnection(connId: string) {
  connectionStatuses.value[connId] = 'idle'
  connectionsStore.clearSchema()
  if (activeConnection.value?.id === connId) {
    connectionsStore.setActiveConnection('')
  }
  toast.info('Disconnected')
}

async function deleteConnection(e: Event, connId: string) {
  e.stopPropagation()
  const ok = await connectionsStore.deleteConnection(connId)
  if (ok) {
    delete connectionStatuses.value[connId]
    toast.success('Connection removed')
  }
}
</script>

<template>
  <Sidebar collapsible="icon" class="border-r border-border">
    <!-- Header: Logo -->
    <SidebarHeader>
      <SidebarMenu>
        <SidebarMenuItem>
          <SidebarMenuButton size="lg" as-child>
            <NuxtLink to="/" class="flex items-center gap-2">
              <div class="flex aspect-square size-8 items-center justify-center rounded-lg bg-primary text-primary-foreground">
                <Icon name="lucide:database-zap" class="size-4" />
              </div>
              <div class="flex flex-col gap-0.5 leading-none">
                <span class="font-semibold">QueryStudio</span>
                <span class="text-xs text-muted-foreground">AI Database Tool</span>
              </div>
            </NuxtLink>
          </SidebarMenuButton>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarHeader>

    <SidebarContent>
      <!-- Main Navigation -->
      <SidebarGroup>
        <SidebarGroupLabel>Navigation</SidebarGroupLabel>
        <SidebarGroupContent>
          <SidebarMenu>
            <SidebarMenuItem
              v-for="item in navigationMenus[0]?.items"
              :key="item.href"
            >
              <SidebarMenuButton
                as-child
                :is-active="isActive(item.href ?? '/')"
                :tooltip="item.title"
              >
                <NuxtLink :to="item.href ?? '/'">
                  <Icon :name="item.icon ?? 'lucide:circle'" />
                  <span>{{ item.title }}</span>
                </NuxtLink>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroupContent>
      </SidebarGroup>

      <SidebarSeparator />

      <!-- Connections -->
      <SidebarGroup>
        <SidebarGroupLabel class="flex items-center justify-between">
          <span>Connections</span>
          <button
            class="rounded p-0.5 hover:bg-sidebar-accent text-muted-foreground hover:text-foreground transition-colors"
            @click="showNewConnectionDialog = true"
          >
            <Icon name="lucide:plus" class="size-3.5" />
          </button>
        </SidebarGroupLabel>
        <SidebarGroupContent>
          <SidebarMenu>
            <SidebarMenuItem
              v-for="conn in connections"
              :key="conn.id"
            >
              <SidebarMenuButton
                :is-active="activeConnection?.id === conn.id"
                :tooltip="`${conn.name} (${conn.db_type})`"
                class="group"
                @click="selectConnection(conn)"
              >
                <Icon :name="getEngineIcon(conn.db_type)" class="size-4 shrink-0" />
                <span class="truncate">{{ conn.name }}</span>
                <div class="ml-auto flex items-center gap-1.5 shrink-0">
                  <button
                    v-if="connectionStatuses[conn.id] === 'connected'"
                    class="opacity-0 group-hover:opacity-100 rounded p-0.5 hover:bg-yellow-500/20 text-muted-foreground hover:text-yellow-500 transition-all"
                    title="Disconnect"
                    @click.stop="disconnectConnection(conn.id)"
                  >
                    <Icon name="lucide:unplug" class="size-3" />
                  </button>
                  <button
                    class="opacity-0 group-hover:opacity-100 rounded p-0.5 hover:bg-accent text-muted-foreground hover:text-foreground transition-all"
                    title="Edit connection"
                    @click="startEdit($event, conn)"
                  >
                    <Icon name="lucide:pencil" class="size-3" />
                  </button>
                  <button
                    class="opacity-0 group-hover:opacity-100 rounded p-0.5 hover:bg-destructive/20 text-muted-foreground hover:text-destructive transition-all"
                    title="Remove connection"
                    @click="deleteConnection($event, conn.id)"
                  >
                    <Icon name="lucide:trash-2" class="size-3" />
                  </button>
                  <div
                    v-if="getStatusDot(conn.id).visible"
                    class="size-2 rounded-full"
                    :class="getStatusDot(conn.id).color"
                    :title="getStatusDot(conn.id).label"
                  />
                </div>
              </SidebarMenuButton>
            </SidebarMenuItem>

            <SidebarMenuItem v-if="connections.length === 0">
              <SidebarMenuButton
                class="text-muted-foreground"
                @click="showNewConnectionDialog = true"
              >
                <Icon name="lucide:plus-circle" class="size-4" />
                <span>Add connection</span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroupContent>
      </SidebarGroup>
    </SidebarContent>

    <!-- Footer: Settings + Dark mode -->
    <SidebarFooter>
      <SidebarMenu>
        <SidebarMenuItem
          v-for="item in bottomMenuItems"
          :key="item.href"
        >
          <SidebarMenuButton as-child :tooltip="item.title">
            <NuxtLink :to="item.href">
              <Icon :name="item.icon" />
              <span>{{ item.title }}</span>
            </NuxtLink>
          </SidebarMenuButton>
        </SidebarMenuItem>
        <SidebarMenuItem>
          <ColorModeToggle />
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarFooter>

  </Sidebar>

  <!-- New connection dialog -->
  <ConnectionDialog v-model:open="showNewConnectionDialog" />

  <!-- Edit connection dialog -->
  <ConnectionDialog
    v-model:open="showEditDialog"
    :edit-connection="editingConnection"
    @update:open="(v) => { if (!v) editingConnection = null }"
  />
</template>
