<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { toast } from 'vue-sonner'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Badge } from '~/components/ui/badge'

const props = defineProps<{
  connectionId: string
}>()

interface RtdbEvent {
  kind: string
  path: string
  data: unknown
}

const watchPath = ref('/')
const isSubscribed = ref(false)
const isSubscribing = ref(false)
const subscriptionId = ref<string | null>(null)
const events = ref<RtdbEvent[]>([])
const snapshot = ref<Record<string, unknown>>({})

let unlisten: UnlistenFn | null = null

async function startWatch() {
  if (!watchPath.value.trim()) return
  isSubscribing.value = true

  try {
    const id = await invoke<string>('rtdb_subscribe', {
      connectionId: props.connectionId,
      path: watchPath.value,
    })

    subscriptionId.value = id
    isSubscribed.value = true
    events.value = []
    snapshot.value = {}

    unlisten = await listen<RtdbEvent>(`rtdb:event:${id}`, (event) => {
      const evt = event.payload
      events.value.unshift(evt)

      if (events.value.length > 200) {
        events.value = events.value.slice(0, 200)
      }

      if (evt.kind === 'put') {
        if (evt.path === '/') {
          snapshot.value = (evt.data as Record<string, unknown>) ?? {}
        }
        else {
          const key = evt.path.replace(/^\//, '')
          if (evt.data === null) {
            delete snapshot.value[key]
          }
          else {
            snapshot.value[key] = evt.data
          }
        }
      }
      else if (evt.kind === 'patch' && typeof evt.data === 'object' && evt.data !== null) {
        Object.assign(snapshot.value, evt.data)
      }
      else if (evt.kind === 'error') {
        toast.error('RTDB stream error', { description: String(evt.data) })
      }
    })
  }
  catch (err) {
    toast.error('Failed to subscribe', { description: err as string })
  }
  finally {
    isSubscribing.value = false
  }
}

async function stopWatch() {
  if (subscriptionId.value) {
    try {
      await invoke('rtdb_unsubscribe', { subId: subscriptionId.value })
    }
    catch {
      // Subscription may already be cleaned up
    }
  }
  if (unlisten) {
    unlisten()
    unlisten = null
  }
  isSubscribed.value = false
  subscriptionId.value = null
}

function formatValue(val: unknown): string {
  if (val === null || val === undefined) return 'null'
  if (typeof val === 'object') return JSON.stringify(val, null, 2)
  return String(val)
}

onUnmounted(() => {
  stopWatch()
})
</script>

<template>
  <div class="flex flex-col h-full overflow-hidden">
    <div class="shrink-0 flex items-center gap-2 pb-3">
      <Input
        v-model="watchPath"
        placeholder="/ or /users or /messages"
        class="flex-1 font-mono text-sm"
        :disabled="isSubscribed"
      />
      <Button
        v-if="!isSubscribed"
        size="sm"
        :disabled="isSubscribing || !watchPath.trim()"
        @click="startWatch"
      >
        <Icon v-if="isSubscribing" name="lucide:loader-2" class="size-3.5 animate-spin" />
        <Icon v-else name="lucide:radio" class="size-3.5" />
        Watch
      </Button>
      <Button
        v-else
        size="sm"
        variant="destructive"
        @click="stopWatch"
      >
        <Icon name="lucide:square" class="size-3.5" />
        Stop
      </Button>
    </div>

    <div v-if="!isSubscribed && events.length === 0" class="flex-1 flex flex-col items-center justify-center text-muted-foreground gap-2">
      <Icon name="lucide:radio" class="size-8" />
      <p class="text-sm">Enter a path and click Watch to stream live changes</p>
    </div>

    <div v-else class="flex-1 overflow-hidden flex flex-col min-h-0 gap-3">
      <!-- Current snapshot -->
      <div v-if="Object.keys(snapshot).length > 0" class="shrink-0 max-h-[40%] overflow-auto rounded-md border border-border">
        <div class="flex items-center gap-2 px-3 py-1.5 border-b border-border bg-muted/40">
          <Icon name="lucide:database" class="size-3.5 text-muted-foreground" />
          <span class="text-xs font-medium text-muted-foreground">Current Snapshot</span>
          <Badge v-if="isSubscribed" variant="outline" class="text-[10px] gap-1">
            <span class="size-1.5 rounded-full bg-green-500 inline-block animate-pulse" />
            Live
          </Badge>
        </div>
        <pre class="p-3 text-xs font-mono overflow-auto">{{ formatValue(snapshot) }}</pre>
      </div>

      <!-- Event log -->
      <div class="flex-1 overflow-auto rounded-md border border-border min-h-0">
        <div class="flex items-center gap-2 px-3 py-1.5 border-b border-border bg-muted/40 sticky top-0">
          <Icon name="lucide:list" class="size-3.5 text-muted-foreground" />
          <span class="text-xs font-medium text-muted-foreground">Events</span>
          <Badge variant="secondary" class="text-[10px]">{{ events.length }}</Badge>
        </div>
        <div class="divide-y divide-border">
          <div
            v-for="(evt, i) in events"
            :key="i"
            class="px-3 py-2 text-xs hover:bg-muted/20 transition-colors"
          >
            <div class="flex items-center gap-2 mb-1">
              <Badge
                :variant="evt.kind === 'put' ? 'default' : evt.kind === 'error' ? 'destructive' : 'secondary'"
                class="text-[10px]"
              >
                {{ evt.kind }}
              </Badge>
              <span class="font-mono text-muted-foreground">{{ evt.path }}</span>
            </div>
            <pre class="text-[11px] font-mono text-muted-foreground truncate max-w-full">{{ typeof evt.data === 'object' ? JSON.stringify(evt.data) : evt.data }}</pre>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
