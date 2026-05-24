<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '~/components/ui/dialog'
import { Button } from '~/components/ui/button'
import { Textarea } from '~/components/ui/textarea'
import { Badge } from '~/components/ui/badge'
import { Label } from '~/components/ui/label'
import { prettyJson, validateJsonObject } from '~/lib/mongo'

type Op = 'insert' | 'replace' | 'delete'

const props = defineProps<{
  op: Op
  connectionId: string
  collection: string
  /** For replace + delete: the filter doc as a JSON string. */
  initialFilter?: string
  /** For insert + replace: the document body as a JSON string. */
  initialDocument?: string
}>()

const emit = defineEmits<{
  executed: [affected: number]
}>()

const open = defineModel<boolean>('open', { default: false })

const filterText = ref(props.initialFilter ?? '')
const docText = ref(props.initialDocument ?? '')
const isRunning = ref(false)
/** Replace-mode toggle (op === 'replace' only). Off (default) = partial
 *  update via $set / $unset over only changed fields. On = full document
 *  replace via replaceOne. */
const replaceMode = ref(false)

// Re-seed every time the dialog opens. The parent reuses the same instance
// across rows, so without this the previous row's doc would persist.
watch(() => [open.value, props.initialFilter, props.initialDocument] as const, ([isOpen, f, d]) => {
  if (isOpen) {
    filterText.value = f ?? ''
    docText.value = d ?? prettyJson({})
    // Default to patch mode every time the dialog opens. Users who want
    // full replace re-enable it explicitly.
    replaceMode.value = false
  }
})

const filterValidation = computed(() => {
  if (props.op === 'insert') return { ok: true }
  return validateJsonObject(filterText.value)
})

const docValidation = computed(() => {
  if (props.op === 'delete') return { ok: true }
  return validateJsonObject(docText.value)
})

const canExecute = computed(() => filterValidation.value.ok && docValidation.value.ok && !isRunning.value)

const titleLabel = computed(() => {
  switch (props.op) {
    case 'insert': return 'Insert document'
    case 'replace': return replaceMode.value ? 'Replace document' : 'Update document'
    case 'delete': return 'Delete document'
  }
})

const operationLine = computed(() => {
  switch (props.op) {
    case 'insert': return `db.${props.collection}.insertOne(...)`
    case 'replace': return replaceMode.value
      ? `db.${props.collection}.replaceOne(filter, replacement)`
      : `db.${props.collection}.updateOne(filter, { $set, $unset })`
    case 'delete': return `db.${props.collection}.deleteOne(filter)`
  }
})

/** Diff two top-level JSON objects. Returns the {field: value} pairs that
 *  differ (deep-equal at the value level via JSON.stringify) and the field
 *  names that were removed in the edited version. `_id` is excluded from
 *  both — it can't be `$set`-changed in Mongo. */
function diffTopLevel(orig: Record<string, unknown>, edited: Record<string, unknown>) {
  const set: Record<string, unknown> = {}
  const unset: string[] = []
  for (const k of Object.keys(edited)) {
    if (k === '_id') continue
    if (!(k in orig) || JSON.stringify(orig[k]) !== JSON.stringify(edited[k])) {
      set[k] = edited[k]
    }
  }
  for (const k of Object.keys(orig)) {
    if (k === '_id') continue
    if (!(k in edited)) unset.push(k)
  }
  return { set, unset }
}

async function execute() {
  if (!canExecute.value) return
  isRunning.value = true
  try {
    let affected = 0
    if (props.op === 'insert') {
      // insert_one returns the new _id as a JSON string. Treat any successful
      // call as exactly 1 affected document.
      await invoke<string>('mongo_insert_one', {
        connectionId: props.connectionId,
        collection: props.collection,
        docJson: docText.value,
      })
      affected = 1
    }
    else if (props.op === 'replace') {
      if (replaceMode.value) {
        affected = await invoke<number>('mongo_replace_one', {
          connectionId: props.connectionId,
          collection: props.collection,
          filterJson: filterText.value,
          replacementJson: docText.value,
        })
      }
      else {
        // Partial update: diff initial vs edited and emit only the changes.
        const original = props.initialDocument ? JSON.parse(props.initialDocument) : {}
        const edited = JSON.parse(docText.value)
        const { set, unset } = diffTopLevel(original, edited)
        if (Object.keys(set).length === 0 && unset.length === 0) {
          toast.info('No changes to save.')
          isRunning.value = false
          return
        }
        affected = await invoke<number>('mongo_update_one', {
          connectionId: props.connectionId,
          collection: props.collection,
          filterJson: filterText.value,
          setJson: JSON.stringify(set),
          unsetFields: unset,
        })
      }
    }
    else {
      affected = await invoke<number>('mongo_delete_one', {
        connectionId: props.connectionId,
        collection: props.collection,
        filterJson: filterText.value,
      })
    }
    emit('executed', affected)
    const verb = props.op === 'insert'
      ? 'inserted'
      : props.op === 'replace'
        ? (replaceMode.value ? 'replaced' : 'updated')
        : 'deleted'
    toast.success(`${affected} ${affected === 1 ? 'document' : 'documents'} ${verb}`)
    open.value = false
  }
  catch (err) {
    toast.error('Operation failed', { description: err as string })
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
          <Icon
            :name="op === 'delete' ? 'lucide:trash-2' : op === 'replace' ? 'lucide:pencil' : 'lucide:plus'"
            class="size-5"
          />
          {{ titleLabel }}
          <Badge :variant="op === 'delete' ? 'destructive' : 'default'" class="ml-1">{{ op.toUpperCase() }}</Badge>
        </DialogTitle>
        <DialogDescription class="font-mono text-xs">
          {{ operationLine }}
        </DialogDescription>
      </DialogHeader>

      <div class="overflow-y-auto flex-1 space-y-3 py-2 pr-1">
        <!-- Filter editor (replace + delete only) -->
        <div v-if="op !== 'insert'" class="space-y-1">
          <Label class="text-xs">Filter</Label>
          <Textarea
            v-model="filterText"
            rows="4"
            spellcheck="false"
            class="font-mono text-xs resize-none"
          />
          <p v-if="!filterValidation.ok" class="text-xs text-destructive">{{ filterValidation.error }}</p>
        </div>

        <!-- Document editor (insert + replace) -->
        <div v-if="op !== 'delete'" class="space-y-1">
          <Label class="text-xs">{{ op === 'insert' ? 'Document' : (replaceMode ? 'Replacement' : 'Edited document') }}</Label>
          <Textarea
            v-model="docText"
            rows="14"
            spellcheck="false"
            class="font-mono text-xs resize-none"
          />
          <p v-if="!docValidation.ok" class="text-xs text-destructive">{{ docValidation.error }}</p>
          <p v-else-if="op === 'replace'" class="text-xs text-muted-foreground">
            <template v-if="replaceMode">
              Replace mode: the entire document is rewritten. Fields not listed here are lost. <code>_id</code> is preserved server-side.
            </template>
            <template v-else>
              Update mode: only the top-level fields you changed are written via <code>$set</code>; fields you removed are <code>$unset</code>. Safer under concurrent writes.
            </template>
          </p>
        </div>

        <!-- Replace-mode toggle (replace op only). Patch is the default. -->
        <label v-if="op === 'replace'" class="flex items-center gap-2 text-xs cursor-pointer select-none">
          <input
            v-model="replaceMode"
            type="checkbox"
            class="size-3.5 accent-primary"
          >
          Replace entire document
          <span class="text-muted-foreground">(rewrites every field, removes anything you didn't list)</span>
        </label>

        <p v-if="op === 'delete'" class="text-xs text-muted-foreground">
          This removes exactly one document matching the filter. Be sure the filter targets the right row.
        </p>
      </div>

      <DialogFooter>
        <Button variant="outline" :disabled="isRunning" @click="open = false">Cancel</Button>
        <Button :variant="op === 'delete' ? 'destructive' : 'default'" :disabled="!canExecute" @click="execute">
          <Icon v-if="isRunning" name="lucide:loader-2" class="size-4 animate-spin" />
          <Icon v-else name="lucide:play" class="size-4" />
          Execute
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
