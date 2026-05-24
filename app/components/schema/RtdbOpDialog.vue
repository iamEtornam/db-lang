<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '~/components/ui/dialog'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Textarea } from '~/components/ui/textarea'
import { Label } from '~/components/ui/label'
import { Badge } from '~/components/ui/badge'
import { prettyJson, validateJsonAny } from '~/lib/rtdb'

type Op = 'insert' | 'replace' | 'delete'

const props = defineProps<{
  op: Op
  connectionId: string
  /** Top-level node name shown in the schema sidebar (e.g. `users`). */
  node: string
  /** For replace + delete: the child key under the node.
   *  For insert: pre-fill (empty = use push to auto-generate). */
  initialKey?: string
  /** For insert + replace: JSON value being written. Any JSON shape allowed. */
  initialValue?: string
}>()

const emit = defineEmits<{
  executed: [affected: number]
}>()

const open = defineModel<boolean>('open', { default: false })

const childKey = ref(props.initialKey ?? '')
const valueText = ref(props.initialValue ?? '')
const isRunning = ref(false)
/** Replace-mode toggle for op === 'replace'. Off (default) = PATCH merge
 *  with `{changed: value, removed: null}`. On = PUT full replace. PATCH
 *  only applies when both the original and edited values are JSON objects;
 *  primitive-to-primitive edits always go through PUT. */
const replaceMode = ref(false)

watch(() => [open.value, props.initialKey, props.initialValue] as const, ([isOpen, k, v]) => {
  if (isOpen) {
    childKey.value = k ?? ''
    valueText.value = v ?? prettyJson(null)
    replaceMode.value = false
  }
})

const valueValidation = computed(() => {
  if (props.op === 'delete') return { ok: true }
  return validateJsonAny(valueText.value)
})

const keyRequired = computed(() => props.op === 'replace' || props.op === 'delete')
const keyMissing = computed(() => keyRequired.value && childKey.value.trim() === '')

const canExecute = computed(() => valueValidation.value.ok && !keyMissing.value && !isRunning.value)

const titleLabel = computed(() => {
  switch (props.op) {
    case 'insert': return 'Insert child'
    case 'replace': return replaceMode.value ? 'Replace child' : 'Update child'
    case 'delete': return 'Delete child'
  }
})

/** Whether the patch path is available for the current edit. RTDB PATCH
 *  requires an object body; primitive/array edits fall back to PUT. */
const canPatch = computed(() => {
  if (props.op !== 'replace') return false
  try {
    const orig = props.initialValue ? JSON.parse(props.initialValue) : null
    const edited = JSON.parse(valueText.value)
    return (
      typeof orig === 'object' && orig !== null && !Array.isArray(orig)
      && typeof edited === 'object' && edited !== null && !Array.isArray(edited)
    )
  }
  catch {
    return false
  }
})

const pathPreview = computed(() => {
  if (props.op === 'insert' && childKey.value.trim() === '') return `${props.node}/<auto-push>`
  return `${props.node}/${childKey.value}`
})

async function execute() {
  if (!canExecute.value) return
  isRunning.value = true
  try {
    if (props.op === 'insert') {
      const trimmedKey = childKey.value.trim()
      if (trimmedKey === '') {
        const newKey = await invoke<string>('rtdb_push', {
          connectionId: props.connectionId,
          path: props.node,
          valueJson: valueText.value,
        })
        toast.success(`Child pushed (key: ${newKey})`)
      }
      else {
        await invoke<void>('rtdb_set', {
          connectionId: props.connectionId,
          path: `${props.node}/${trimmedKey}`,
          valueJson: valueText.value,
        })
        toast.success(`Child written at ${props.node}/${trimmedKey}`)
      }
      emit('executed', 1)
    }
    else if (props.op === 'replace') {
      // Patch when both original and edited are objects AND replaceMode is off.
      // Otherwise PUT (full replace) — covers primitive ↔ primitive, array
      // edits, and the explicit "Replace entire" opt-in.
      if (!replaceMode.value && canPatch.value) {
        const orig = JSON.parse(props.initialValue ?? 'null') as Record<string, unknown>
        const edited = JSON.parse(valueText.value) as Record<string, unknown>
        const partial: Record<string, unknown> = {}
        for (const k of Object.keys(edited)) {
          if (!(k in orig) || JSON.stringify(orig[k]) !== JSON.stringify(edited[k])) {
            partial[k] = edited[k]
          }
        }
        for (const k of Object.keys(orig)) {
          if (!(k in edited)) partial[k] = null
        }
        if (Object.keys(partial).length === 0) {
          toast.info('No changes to save.')
          isRunning.value = false
          return
        }
        await invoke<void>('rtdb_patch', {
          connectionId: props.connectionId,
          path: `${props.node}/${childKey.value}`,
          partialJson: JSON.stringify(partial),
        })
        toast.success('Child updated')
      }
      else {
        await invoke<void>('rtdb_set', {
          connectionId: props.connectionId,
          path: `${props.node}/${childKey.value}`,
          valueJson: valueText.value,
        })
        toast.success('Child replaced')
      }
      emit('executed', 1)
    }
    else {
      await invoke<void>('rtdb_delete', {
        connectionId: props.connectionId,
        path: `${props.node}/${childKey.value}`,
      })
      toast.success('Child deleted')
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
          {{ pathPreview }}
        </DialogDescription>
      </DialogHeader>

      <div class="overflow-y-auto flex-1 space-y-3 py-2 pr-1">
        <div class="space-y-1">
          <Label class="text-xs">
            Child key
            <span v-if="!keyRequired" class="text-muted-foreground font-normal ml-1">(leave blank to push with a server-generated key)</span>
            <span v-else class="text-destructive ml-1">*</span>
          </Label>
          <Input
            v-model="childKey"
            :disabled="op !== 'insert'"
            placeholder="child-key"
            class="font-mono text-xs"
          />
          <p v-if="keyMissing" class="text-xs text-destructive">Child key is required for {{ op }}.</p>
        </div>

        <div v-if="op !== 'delete'" class="space-y-1">
          <Label class="text-xs">Value (any JSON)</Label>
          <Textarea
            v-model="valueText"
            rows="14"
            spellcheck="false"
            class="font-mono text-xs resize-none"
          />
          <p v-if="!valueValidation.ok" class="text-xs text-destructive">{{ valueValidation.error }}</p>
          <p v-else-if="op === 'replace'" class="text-xs text-muted-foreground">
            <template v-if="replaceMode || !canPatch">
              PUT mode: the entire value at this path is rewritten. Sending <code>null</code> deletes the node.
              <template v-if="!canPatch && !replaceMode">
                (PATCH unavailable — both sides need to be JSON objects.)
              </template>
            </template>
            <template v-else>
              PATCH mode: only the top-level keys you changed are written; keys you removed are set to <code>null</code>. Other keys at this path stay as-is.
            </template>
          </p>
          <p v-else class="text-xs text-muted-foreground">
            Strings, numbers, booleans, objects, arrays — any valid JSON is accepted.
          </p>
        </div>

        <!-- Replace-mode toggle (replace op only, and only when PATCH is
             applicable; otherwise PUT is the only option and the toggle
             would be confusing). -->
        <label v-if="op === 'replace' && canPatch" class="flex items-center gap-2 text-xs cursor-pointer select-none">
          <input
            v-model="replaceMode"
            type="checkbox"
            class="size-3.5 accent-primary"
          >
          Replace entire value
          <span class="text-muted-foreground">(rewrites every key, removes anything you didn't list)</span>
        </label>

        <p v-if="op === 'delete'" class="text-xs text-muted-foreground">
          This removes everything at <code class="font-mono">{{ pathPreview }}</code>, including any descendant nodes. Not reversible from this UI.
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
