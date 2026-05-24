<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '~/components/ui/dialog'
import { Button } from '~/components/ui/button'
import { Badge } from '~/components/ui/badge'

const props = defineProps<{
  connectionId: string
  /** Each statement runs in a single atomic transaction on the backend.
   *  Order is preserved. Statements are shown read-only — users wanting to
   *  hand-edit should discard the batch and use the row-level edit flow. */
  statements: string[]
}>()

const emit = defineEmits<{
  executed: [count: number]
}>()

const open = defineModel<boolean>('open', { default: false })

const isRunning = ref(false)

async function execute() {
  if (props.statements.length === 0 || isRunning.value) return
  isRunning.value = true
  try {
    const count = await invoke<number>('execute_sql_batch', {
      connectionId: props.connectionId,
      statements: props.statements,
    })
    toast.success(`${count} ${count === 1 ? 'statement' : 'statements'} executed atomically`)
    emit('executed', count)
    open.value = false
  }
  catch (err) {
    toast.error('Batch execution failed', { description: err as string })
  }
  finally {
    isRunning.value = false
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-3xl max-h-[85vh] flex flex-col overflow-hidden">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Icon name="lucide:layers" class="size-5" />
          Save changes
          <Badge variant="default" class="ml-1">{{ statements.length }} {{ statements.length === 1 ? 'statement' : 'statements' }}</Badge>
        </DialogTitle>
        <DialogDescription>
          All statements run in a single atomic transaction. If any fails, the whole batch rolls back.
        </DialogDescription>
      </DialogHeader>

      <div class="overflow-y-auto flex-1 space-y-2 py-2 pr-1">
        <div
          v-for="(stmt, i) in statements"
          :key="i"
          class="border border-border rounded-md bg-muted/30 font-mono text-xs whitespace-pre-wrap p-2"
        >
          <div class="text-[10px] uppercase tracking-wide text-muted-foreground mb-1">Statement {{ i + 1 }}</div>
          {{ stmt }}
        </div>
        <p class="text-xs text-muted-foreground pt-1">
          Statements are shown read-only. To hand-edit one, cancel and use the row-level edit flow.
        </p>
      </div>

      <DialogFooter>
        <Button variant="outline" :disabled="isRunning" @click="open = false">Cancel</Button>
        <Button :disabled="isRunning || statements.length === 0" @click="execute">
          <Icon v-if="isRunning" name="lucide:loader-2" class="size-4 animate-spin" />
          <Icon v-else name="lucide:play" class="size-4" />
          Execute {{ statements.length }} {{ statements.length === 1 ? 'statement' : 'statements' }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
