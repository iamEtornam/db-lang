<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '~/components/ui/dialog'
import { Button } from '~/components/ui/button'
import { Badge } from '~/components/ui/badge'

type Engine = 'mongo' | 'firestore' | 'rtdb' | 'redis'

const props = defineProps<{
  engine: Engine
  connectionId: string
  /** Collection / node / prefix the selected items live under. Mongo +
   *  Firestore: the collection name. RTDB: the parent path. Redis: ignored
   *  (keys are global). */
  container: string
  /** The stringified identifiers to delete. Mongo: _id values (hex or
   *  whatever the doc used). Firestore: doc IDs. RTDB: child keys under
   *  `container`. Redis: full key names. */
  ids: string[]
}>()

const emit = defineEmits<{
  executed: [affected: number]
}>()

const open = defineModel<boolean>('open', { default: false })

const isRunning = ref(false)

const engineLabel = computed(() => {
  switch (props.engine) {
    case 'mongo': return 'MongoDB documents'
    case 'firestore': return 'Firestore documents'
    case 'rtdb': return 'Realtime Database children'
    case 'redis': return 'Redis keys'
  }
})

const operationLine = computed(() => {
  switch (props.engine) {
    case 'mongo': return `db.${props.container}.deleteMany({ _id: { $in: [...] } })`
    case 'firestore': return `Batch commit ${props.ids.length} deletes under ${props.container}/`
    case 'rtdb': return `PATCH ${props.container}.json  ←  { each_key: null }`
    case 'redis': return `DEL key1 key2 ... (${props.ids.length})`
  }
})

// Cap the visible list so a 5000-row bulk doesn't blow out the dialog.
const PREVIEW_LIMIT = 100
const visibleIds = computed(() => props.ids.slice(0, PREVIEW_LIMIT))
const overflowCount = computed(() => Math.max(0, props.ids.length - PREVIEW_LIMIT))

async function execute() {
  if (props.ids.length === 0 || isRunning.value) return
  isRunning.value = true
  try {
    let affected = 0
    switch (props.engine) {
      case 'mongo': {
        // Filter targets _id $in: backend coerces 24-hex strings to ObjectId
        // before dispatching to delete_many.
        const filter = JSON.stringify({ _id: { $in: props.ids } })
        affected = await invoke<number>('mongo_delete_many', {
          connectionId: props.connectionId,
          collection: props.container,
          filterJson: filter,
        })
        break
      }
      case 'firestore': {
        affected = await invoke<number>('firestore_delete_many_documents', {
          connectionId: props.connectionId,
          collection: props.container,
          docIds: props.ids,
        })
        break
      }
      case 'rtdb': {
        affected = await invoke<number>('rtdb_delete_many', {
          connectionId: props.connectionId,
          parentPath: props.container,
          childKeys: props.ids,
        })
        break
      }
      case 'redis': {
        affected = await invoke<number>('redis_delete_keys', {
          connectionId: props.connectionId,
          keys: props.ids,
        })
        break
      }
    }
    toast.success(`${affected} ${affected === 1 ? 'item' : 'items'} deleted`)
    emit('executed', affected)
    open.value = false
  }
  catch (err) {
    toast.error('Bulk delete failed', { description: err as string })
  }
  finally {
    isRunning.value = false
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-2xl max-h-[85vh] flex flex-col overflow-hidden">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Icon name="lucide:trash-2" class="size-5" />
          Bulk delete
          <Badge variant="destructive" class="ml-1">{{ ids.length }} {{ ids.length === 1 ? 'item' : 'items' }}</Badge>
        </DialogTitle>
        <DialogDescription class="font-mono text-xs">
          {{ operationLine }}
        </DialogDescription>
      </DialogHeader>

      <div class="overflow-y-auto flex-1 space-y-2 py-2 pr-1">
        <p class="text-xs text-muted-foreground">
          Deleting {{ ids.length }} {{ engineLabel }}. This is not reversible from this UI.
        </p>
        <div class="border border-border rounded-md max-h-[40vh] overflow-y-auto font-mono text-xs bg-muted/30 px-3 py-2">
          <div v-for="id in visibleIds" :key="id" class="truncate py-0.5">
            {{ id }}
          </div>
          <div v-if="overflowCount > 0" class="text-muted-foreground italic pt-1">
            ... and {{ overflowCount }} more
          </div>
        </div>
      </div>

      <DialogFooter>
        <Button variant="outline" :disabled="isRunning" @click="open = false">Cancel</Button>
        <Button variant="destructive" :disabled="ids.length === 0 || isRunning" @click="execute">
          <Icon v-if="isRunning" name="lucide:loader-2" class="size-4 animate-spin" />
          <Icon v-else name="lucide:trash-2" class="size-4" />
          Delete {{ ids.length }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
