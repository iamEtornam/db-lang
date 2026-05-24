<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '~/components/ui/dialog'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Textarea } from '~/components/ui/textarea'
import { Label } from '~/components/ui/label'
import { Badge } from '~/components/ui/badge'

const props = defineProps<{
  /** Drives title, badge variant, button styling, and toast wording.
   *    insert/update     — row-level CRUD (Phase 1)
   *    delete            — row-level DELETE (Phase 1)
   *    ddl               — neutral DDL (CREATE, ALTER): "Statement executed" toast
   *    drop              — destructive DDL (DROP TABLE/COLUMN): destructive styling,
   *                        typically paired with requireTypedConfirmation */
  kind: 'insert' | 'update' | 'delete' | 'ddl' | 'drop'
  connectionId: string
  initialSql: string
  /** When set, the Execute button is disabled until the user types this
   *  exact string into the typed-confirmation input. Used to gate DROP
   *  operations behind a "type the name to confirm" pattern. */
  requireTypedConfirmation?: string
}>()

const emit = defineEmits<{
  executed: [affected: number]
}>()

const open = defineModel<boolean>('open', { default: false })

const editableSql = ref(props.initialSql)
const typedConfirmation = ref('')
const isRunning = ref(false)

// Re-seed the textarea every time the dialog opens with a fresh statement
// (the parent reuses the same component instance across edits). Clear the
// typed-confirmation field too — never carry it across opens.
watch(() => [props.initialSql, open.value] as const, ([sql, isOpen]) => {
  if (isOpen) {
    editableSql.value = sql
    typedConfirmation.value = ''
  }
})

const kindLabel = computed(() => {
  switch (props.kind) {
    case 'insert': return 'Insert row'
    case 'update': return 'Update row'
    case 'delete': return 'Delete row'
    case 'ddl': return 'Run statement'
    case 'drop': return 'Drop'
  }
})

const kindVariant = computed<'default' | 'destructive' | 'secondary'>(() => {
  if (props.kind === 'delete' || props.kind === 'drop') return 'destructive'
  if (props.kind === 'ddl') return 'secondary'
  return 'default'
})

const kindIcon = computed(() => {
  switch (props.kind) {
    case 'delete': return 'lucide:trash-2'
    case 'drop': return 'lucide:trash-2'
    case 'update': return 'lucide:pencil'
    case 'ddl': return 'lucide:wrench'
    case 'insert': return 'lucide:plus'
  }
})

const typedConfirmationOk = computed(() => {
  if (!props.requireTypedConfirmation) return true
  return typedConfirmation.value === props.requireTypedConfirmation
})

const canExecute = computed(() =>
  !isRunning.value && !!editableSql.value.trim() && typedConfirmationOk.value,
)

const helpText = computed(() => {
  switch (props.kind) {
    case 'delete':
    case 'drop':
      return 'This statement runs against your live database. Make sure the target is correct.'
    case 'ddl':
      return 'Schema-changing statement. Some DDL is not reversible — review the SQL before executing.'
    default:
      return 'This statement runs against your live database. Make sure the WHERE clause is correct.'
  }
})

async function execute() {
  if (!canExecute.value) return
  const sql = editableSql.value.trim()
  if (!sql) {
    toast.error('Statement is empty')
    return
  }
  isRunning.value = true
  try {
    const affected = await invoke<number>('execute_sql_statement', {
      connectionId: props.connectionId,
      sql,
    })
    emit('executed', affected)
    // DDL typically reports 0 affected — show neutral wording so the user
    // doesn't read "0 rows affected" as a failure for a successful CREATE.
    if (props.kind === 'ddl' || props.kind === 'drop') {
      toast.success('Statement executed')
    }
    else {
      toast.success(`${affected} ${affected === 1 ? 'row' : 'rows'} affected`)
    }
    open.value = false
  }
  catch (err) {
    toast.error('Execution failed', { description: err as string })
  }
  finally {
    isRunning.value = false
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-2xl">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Icon :name="kindIcon" class="size-5" />
          {{ kindLabel }}
          <Badge :variant="kindVariant" class="ml-1">{{ kind.toUpperCase() }}</Badge>
        </DialogTitle>
        <DialogDescription>
          Review the generated SQL. You can edit it before running.
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-2 py-2">
        <Textarea
          v-model="editableSql"
          rows="10"
          spellcheck="false"
          class="font-mono text-xs resize-none"
        />
        <p class="text-xs text-muted-foreground">{{ helpText }}</p>

        <!-- Typed-name confirm (DROP TABLE / DROP COLUMN). Same pattern as
             GitHub's "type the repository name to confirm". -->
        <div v-if="requireTypedConfirmation" class="space-y-1 pt-2 border-t border-border">
          <Label class="text-xs">
            Type
            <code class="font-mono bg-destructive/10 text-destructive px-1 rounded">{{ requireTypedConfirmation }}</code>
            to confirm
          </Label>
          <Input
            v-model="typedConfirmation"
            class="font-mono text-xs"
            autocomplete="off"
            spellcheck="false"
          />
          <p v-if="typedConfirmation && !typedConfirmationOk" class="text-xs text-destructive">
            Doesn't match — this is destructive and not reversible from this UI.
          </p>
        </div>
      </div>

      <DialogFooter>
        <Button variant="outline" :disabled="isRunning" @click="open = false">Cancel</Button>
        <Button
          :variant="kindVariant === 'destructive' ? 'destructive' : 'default'"
          :disabled="!canExecute"
          @click="execute"
        >
          <Icon v-if="isRunning" name="lucide:loader-2" class="size-4 animate-spin" />
          <Icon v-else name="lucide:play" class="size-4" />
          Execute
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
