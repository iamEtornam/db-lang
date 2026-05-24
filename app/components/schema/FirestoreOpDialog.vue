<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '~/components/ui/dialog'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Textarea } from '~/components/ui/textarea'
import { Label } from '~/components/ui/label'
import { Badge } from '~/components/ui/badge'
import { prettyJson, validateJsonObject } from '~/lib/firestore'

type Op = 'insert' | 'replace' | 'delete'

const props = defineProps<{
  op: Op
  connectionId: string
  collection: string
  /** For replace + delete: existing doc ID (immutable in those modes).
   *  For insert: pre-fill (typically empty for auto-id). */
  initialDocId?: string
  /** For insert + replace: the document JSON, metadata already stripped. */
  initialDocument?: string
}>()

const emit = defineEmits<{
  executed: [affected: number]
}>()

const open = defineModel<boolean>('open', { default: false })

const docId = ref(props.initialDocId ?? '')
const docText = ref(props.initialDocument ?? '')
const isRunning = ref(false)
/** Replace-mode toggle for op === 'replace'. Off (default) = partial PATCH
 *  with updateMask listing only changed top-level fields. On = full
 *  replace (existing behaviour). */
const replaceMode = ref(false)

watch(() => [open.value, props.initialDocId, props.initialDocument] as const, ([isOpen, id, doc]) => {
  if (isOpen) {
    docId.value = id ?? ''
    docText.value = doc ?? prettyJson({})
    replaceMode.value = false
  }
})

const docValidation = computed(() => {
  if (props.op === 'delete') return { ok: true }
  return validateJsonObject(docText.value)
})

const docIdRequired = computed(() => props.op === 'replace' || props.op === 'delete')
const docIdMissing = computed(() => docIdRequired.value && docId.value.trim() === '')

const canExecute = computed(() => docValidation.value.ok && !docIdMissing.value && !isRunning.value)

const titleLabel = computed(() => {
  switch (props.op) {
    case 'insert': return 'Insert document'
    case 'replace': return replaceMode.value ? 'Replace document' : 'Update document'
    case 'delete': return 'Delete document'
  }
})

/** Diff two top-level JSON objects. Returns the changed top-level keys
 *  (the values to write) and the removed top-level keys (which need to
 *  be included in updateMask but absent from the body so Firestore deletes
 *  them). Deep field-path masks (e.g. `address.city`) are not generated —
 *  changing a nested object replaces that subtree wholesale. */
function diffTopLevel(orig: Record<string, unknown>, edited: Record<string, unknown>) {
  const set: Record<string, unknown> = {}
  const removed: string[] = []
  for (const k of Object.keys(edited)) {
    if (k === '_id' || k === '_createTime' || k === '_updateTime') continue
    if (!(k in orig) || JSON.stringify(orig[k]) !== JSON.stringify(edited[k])) {
      set[k] = edited[k]
    }
  }
  for (const k of Object.keys(orig)) {
    if (k === '_id' || k === '_createTime' || k === '_updateTime') continue
    if (!(k in edited)) removed.push(k)
  }
  return { set, removed }
}

async function execute() {
  if (!canExecute.value) return
  isRunning.value = true
  try {
    if (props.op === 'insert') {
      const newId = await invoke<string>('firestore_create_document', {
        connectionId: props.connectionId,
        collection: props.collection,
        docId: docId.value.trim() === '' ? null : docId.value.trim(),
        docJson: docText.value,
      })
      toast.success(`Document created (id: ${newId})`)
      emit('executed', 1)
    }
    else if (props.op === 'replace') {
      if (replaceMode.value) {
        await invoke<void>('firestore_patch_document', {
          connectionId: props.connectionId,
          collection: props.collection,
          docId: docId.value,
          docJson: docText.value,
        })
        toast.success('Document replaced')
      }
      else {
        const original = props.initialDocument ? JSON.parse(props.initialDocument) : {}
        const edited = JSON.parse(docText.value)
        const { set, removed } = diffTopLevel(original, edited)
        const allPaths = [...Object.keys(set), ...removed]
        if (allPaths.length === 0) {
          toast.info('No changes to save.')
          isRunning.value = false
          return
        }
        await invoke<void>('firestore_patch_document_fields', {
          connectionId: props.connectionId,
          collection: props.collection,
          docId: docId.value,
          fieldPaths: allPaths,
          fieldsSubsetJson: JSON.stringify(set),
        })
        toast.success('Document updated')
      }
      emit('executed', 1)
    }
    else {
      await invoke<void>('firestore_delete_document', {
        connectionId: props.connectionId,
        collection: props.collection,
        docId: docId.value,
      })
      toast.success('Document deleted')
      emit('executed', 1)
    }
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
          {{ collection }}/{{ docId || (op === 'insert' ? '<auto-id>' : '') }}
        </DialogDescription>
      </DialogHeader>

      <div class="overflow-y-auto flex-1 space-y-3 py-2 pr-1">
        <div class="space-y-1">
          <Label class="text-xs">
            Document ID
            <span v-if="!docIdRequired" class="text-muted-foreground font-normal ml-1">(optional — leave blank for auto-id)</span>
            <span v-else class="text-destructive ml-1">*</span>
          </Label>
          <Input
            v-model="docId"
            :disabled="op !== 'insert'"
            placeholder="document-id"
            class="font-mono text-xs"
          />
          <p v-if="docIdMissing" class="text-xs text-destructive">Document ID is required for {{ op }}.</p>
        </div>

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
              Replace mode: PATCH without updateMask rewrites the whole document body — fields not listed here are removed.
            </template>
            <template v-else>
              Update mode: only the top-level fields you changed are written via <code>updateMask</code>; other fields stay as-is.
            </template>
            <code>_id</code>, <code>_createTime</code>, <code>_updateTime</code> are stripped server-side.
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
          This removes the document at <code class="font-mono">{{ collection }}/{{ docId }}</code>. Subcollections under this document are <strong>not</strong> deleted automatically — Firestore requires them to be removed separately.
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
