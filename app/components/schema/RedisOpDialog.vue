<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '~/components/ui/dialog'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Textarea } from '~/components/ui/textarea'
import { Label } from '~/components/ui/label'
import { Badge } from '~/components/ui/badge'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '~/components/ui/select'
import {
  REDIS_TYPES,
  defaultValueFor,
  prettyJson,
  validateForType,
  type RedisType,
} from '~/lib/redis'

type Op = 'insert' | 'replace' | 'delete'

const props = defineProps<{
  op: Op
  connectionId: string
  /** For replace + delete: the key being targeted. Insert defaults to ''. */
  initialKey?: string
  /** For replace + delete: the key's current type (Redis can't change a
   *  key's type without DEL'ing first, so this is readonly in those modes). */
  initialType?: RedisType
  /** For insert + replace: the value JSON (already shaped for the type). */
  initialValue?: string
  /** -1 means no expiry / persistent. */
  initialTtl?: number
}>()

const emit = defineEmits<{
  executed: [affected: number]
}>()

const open = defineModel<boolean>('open', { default: false })

const redisKey = ref(props.initialKey ?? '')
const redisType = ref<RedisType>(props.initialType ?? 'string')
const valueText = ref(props.initialValue ?? '')
const ttlInput = ref<string>(formatTtlForInput(props.initialTtl))
const isRunning = ref(false)
/** Replace-mode toggle for op === 'replace'. Off (default) = type-specific
 *  patch (HSET/HDEL for hash, SADD/SREM for set, ZADD/ZREM for zset, LSET
 *  for list when length unchanged). On (or string type) = DEL + recreate
 *  via redis_set_key. */
const replaceMode = ref(false)

function formatTtlForInput(ttl: number | undefined): string {
  if (ttl == null || ttl < 0) return ''
  return String(ttl)
}

watch(
  () => [open.value, props.initialKey, props.initialType, props.initialValue, props.initialTtl] as const,
  ([isOpen, k, t, v, ttl]) => {
    if (!isOpen) return
    redisKey.value = k ?? ''
    redisType.value = t ?? 'string'
    valueText.value = v ?? defaultValueFor(redisType.value)
    ttlInput.value = formatTtlForInput(ttl)
    replaceMode.value = false
  },
)

// When the user switches type during Insert, swap the value editor seed to
// match. Only fires when the user actively changes the dropdown — not on
// dialog open, which already seeded via the watcher above.
let typeSwitchSeeded = false
watch(redisType, (next, prev) => {
  if (props.op !== 'insert') return
  if (!typeSwitchSeeded) {
    typeSwitchSeeded = true
    return
  }
  if (next === prev) return
  valueText.value = defaultValueFor(next)
})

const valueValidation = computed(() => {
  if (props.op === 'delete') return { ok: true }
  return validateForType(valueText.value, redisType.value)
})

const keyMissing = computed(() => redisKey.value.trim() === '')

const parsedTtl = computed<number | null>(() => {
  const trimmed = ttlInput.value.trim()
  if (trimmed === '') return null
  const n = Number(trimmed)
  if (!Number.isInteger(n)) return null
  return n
})

const ttlInvalid = computed(() => {
  const t = ttlInput.value.trim()
  if (t === '') return false
  return parsedTtl.value === null
})

const canExecute = computed(() =>
  valueValidation.value.ok
  && !keyMissing.value
  && !ttlInvalid.value
  && !streamReplaceBlocked.value
  && !isRunning.value,
)

const titleLabel = computed(() => {
  switch (props.op) {
    case 'insert': return redisType.value === 'stream' ? 'Add stream entry' : 'Insert key'
    case 'replace':
      if (redisType.value === 'stream') return 'Replace stream entry'  // disallowed; shown via streamReplaceBlocked
      return (replaceMode.value || redisType.value === 'string') ? 'Replace key' : 'Update key'
    case 'delete': return 'Delete key'
  }
})

/** Whether patch-mode is available for the current type and op. Strings
 *  have no meaningful partial update (the value IS the whole thing).
 *  Streams have immutable entries — XADD adds, XDEL removes, no in-place
 *  modification. The toggle is hidden in both cases. */
const canPatch = computed(() =>
  props.op === 'replace' && redisType.value !== 'string' && redisType.value !== 'stream',
)

/** True when the dialog is open in replace mode for a stream — Redis
 *  doesn't support modifying stream entries in place, so we surface a
 *  blocker rather than silently DEL+recreate the whole stream (which
 *  would lose history). */
const streamReplaceBlocked = computed(() =>
  props.op === 'replace' && redisType.value === 'stream',
)

function ttlForBackend(): number | null {
  const ttl = parsedTtl.value
  return ttl != null && ttl > 0 ? ttl : null
}

/** Diff helpers — each returns the per-type payload the matching Tauri
 *  command expects. All operate on the parsed-JSON original (from the
 *  prop) vs. the parsed-JSON edited (from the live textarea). */
function diffHash(orig: Record<string, unknown>, edited: Record<string, unknown>) {
  const setFields: Record<string, string> = {}
  const unsetFields: string[] = []
  for (const k of Object.keys(edited)) {
    const next = edited[k]
    const nextStr = next === null || next === undefined ? '' : String(next)
    if (!(k in orig) || String(orig[k] ?? '') !== nextStr) setFields[k] = nextStr
  }
  for (const k of Object.keys(orig)) {
    if (!(k in edited)) unsetFields.push(k)
  }
  return { setFields, unsetFields }
}
function diffSet(orig: string[], edited: string[]) {
  const origSet = new Set(orig)
  const editedSet = new Set(edited)
  const add: string[] = []
  const remove: string[] = []
  for (const m of editedSet) if (!origSet.has(m)) add.push(m)
  for (const m of origSet) if (!editedSet.has(m)) remove.push(m)
  return { add, remove }
}
function diffZset(orig: Record<string, number>, edited: Record<string, number>) {
  const setMembers: Record<string, number> = {}
  const removeMembers: string[] = []
  for (const m of Object.keys(edited)) {
    if (!(m in orig) || orig[m] !== edited[m]) setMembers[m] = edited[m]
  }
  for (const m of Object.keys(orig)) {
    if (!(m in edited)) removeMembers.push(m)
  }
  return { setMembers, removeMembers }
}

async function execute() {
  if (!canExecute.value) return
  isRunning.value = true
  try {
    if (props.op === 'delete') {
      const removed = await invoke<number>('redis_delete_key', {
        connectionId: props.connectionId,
        key: redisKey.value,
      })
      toast.success(`Deleted ${removed} key${removed === 1 ? '' : 's'}`)
      emit('executed', removed)
      open.value = false
      return
    }

    // Stream insert: XADD a single entry. (Streams use XADD/XDEL rather
    // than SET; they're append-only and entries are immutable.)
    if (props.op === 'insert' && redisType.value === 'stream') {
      const fields = JSON.parse(valueText.value) as Record<string, unknown>
      const stringified: Record<string, string> = {}
      for (const [k, v] of Object.entries(fields)) {
        stringified[k] = v === null || v === undefined ? '' : String(v)
      }
      const newId = await invoke<string>('redis_stream_add', {
        connectionId: props.connectionId,
        key: redisKey.value,
        entryId: '',  // server-generated
        fields: stringified,
        ttlSeconds: ttlForBackend(),
      })
      toast.success(`Entry added (id: ${newId})`)
      emit('executed', 1)
      open.value = false
      return
    }

    // Insert always goes through full-set (there's no patch concept for
    // a brand-new key). Replace branches on type + replaceMode.
    if (props.op === 'insert' || replaceMode.value || redisType.value === 'string') {
      await invoke<void>('redis_set_key', {
        connectionId: props.connectionId,
        key: redisKey.value,
        keyType: redisType.value,
        valueJson: valueText.value,
        ttlSeconds: ttlForBackend(),
      })
      toast.success(`Key '${redisKey.value}' written`)
      emit('executed', 1)
      open.value = false
      return
    }

    // Patch path — diff and dispatch by type.
    const orig = props.initialValue ? JSON.parse(props.initialValue) : null
    const edited = JSON.parse(valueText.value)
    const ttl = ttlForBackend()

    if (redisType.value === 'hash') {
      const { setFields, unsetFields } = diffHash(orig ?? {}, edited ?? {})
      if (Object.keys(setFields).length === 0 && unsetFields.length === 0) {
        toast.info('No changes to save.')
        isRunning.value = false
        return
      }
      await invoke<void>('redis_hash_patch', {
        connectionId: props.connectionId,
        key: redisKey.value,
        setFields,
        unsetFields,
        ttlSeconds: ttl,
      })
    }
    else if (redisType.value === 'set') {
      const origArr = Array.isArray(orig) ? orig.map(String) : []
      const editArr = Array.isArray(edited) ? edited.map(String) : []
      const { add, remove } = diffSet(origArr, editArr)
      if (add.length === 0 && remove.length === 0) {
        toast.info('No changes to save.')
        isRunning.value = false
        return
      }
      await invoke<void>('redis_set_patch', {
        connectionId: props.connectionId,
        key: redisKey.value,
        add,
        remove,
        ttlSeconds: ttl,
      })
    }
    else if (redisType.value === 'zset') {
      const { setMembers, removeMembers } = diffZset(orig ?? {}, edited ?? {})
      if (Object.keys(setMembers).length === 0 && removeMembers.length === 0) {
        toast.info('No changes to save.')
        isRunning.value = false
        return
      }
      await invoke<void>('redis_zset_patch', {
        connectionId: props.connectionId,
        key: redisKey.value,
        setMembers,
        removeMembers,
        ttlSeconds: ttl,
      })
    }
    else if (redisType.value === 'list') {
      const origArr = Array.isArray(orig) ? orig.map(String) : []
      const editArr = Array.isArray(edited) ? edited.map(String) : []
      // LSET only works when length matches; otherwise we can't grow or
      // shrink the list. Fall back to full DEL+RPUSH via redis_set_key.
      if (origArr.length !== editArr.length) {
        toast.info('List length changed — falling back to full replace.')
        await invoke<void>('redis_set_key', {
          connectionId: props.connectionId,
          key: redisKey.value,
          keyType: 'list',
          valueJson: valueText.value,
          ttlSeconds: ttl,
        })
      }
      else {
        const changes: [number, string][] = []
        for (let i = 0; i < origArr.length; i++) {
          if (origArr[i] !== editArr[i]) changes.push([i, editArr[i]!])
        }
        if (changes.length === 0) {
          toast.info('No changes to save.')
          isRunning.value = false
          return
        }
        await invoke<void>('redis_list_set_indices', {
          connectionId: props.connectionId,
          key: redisKey.value,
          changes,
          ttlSeconds: ttl,
        })
      }
    }

    toast.success(`Key '${redisKey.value}' updated`)
    emit('executed', 1)
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
          {{ redisKey || (op === 'insert' ? '<new-key>' : '') }}
        </DialogDescription>
      </DialogHeader>

      <div class="overflow-y-auto flex-1 space-y-3 py-2 pr-1">
        <div class="space-y-1">
          <Label class="text-xs">
            Key
            <span class="text-destructive ml-1">*</span>
          </Label>
          <Input
            v-model="redisKey"
            :disabled="op !== 'insert'"
            placeholder="users:42"
            class="font-mono text-xs"
          />
          <p v-if="keyMissing" class="text-xs text-destructive">Key is required.</p>
        </div>

        <div v-if="op !== 'delete'" class="space-y-1">
          <Label class="text-xs">Type</Label>
          <Select v-model="redisType" :disabled="op !== 'insert'">
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="t in REDIS_TYPES" :key="t.value" :value="t.value">
                {{ t.label }}
              </SelectItem>
            </SelectContent>
          </Select>
          <p v-if="op === 'replace'" class="text-xs text-muted-foreground">
            Redis cannot change a key's type in place. To switch types, delete first and re-insert.
          </p>
        </div>

        <div v-if="op !== 'delete'" class="space-y-1">
          <Label class="text-xs">Value</Label>
          <Textarea
            v-model="valueText"
            rows="12"
            spellcheck="false"
            class="font-mono text-xs resize-none"
          />
          <p v-if="!valueValidation.ok" class="text-xs text-destructive">{{ valueValidation.error }}</p>
          <p v-else-if="streamReplaceBlocked" class="text-xs text-destructive">
            Stream entries are immutable in Redis. To modify, XDEL the old entry and XADD a new one.
          </p>
          <p v-else-if="op === 'replace'" class="text-xs text-muted-foreground">
            <template v-if="replaceMode || redisType === 'string'">
              Replace mode: the existing value is removed and recreated from this JSON exactly. Any old fields / list items / set members not listed here are gone.
            </template>
            <template v-else-if="redisType === 'hash'">
              Update mode: HSET on the changed fields, HDEL on the ones you removed. Other fields stay as-is.
            </template>
            <template v-else-if="redisType === 'set'">
              Update mode: SADD on new members, SREM on removed members. Existing members not touched here stay in the set.
            </template>
            <template v-else-if="redisType === 'zset'">
              Update mode: ZADD on changed (member, score) pairs, ZREM on removed members. Other members stay at their existing scores.
            </template>
            <template v-else-if="redisType === 'list'">
              Update mode: LSET each changed index in a single transaction. If the list length changed, falls back to full replace automatically.
            </template>
          </p>
          <p v-else-if="op === 'insert' && redisType === 'stream'" class="text-xs text-muted-foreground">
            XADD with a server-generated entry ID. One entry per click — to backfill many entries, save individually.
          </p>
        </div>

        <!-- Replace-mode toggle. Hidden for strings (no meaningful patch). -->
        <label v-if="canPatch" class="flex items-center gap-2 text-xs cursor-pointer select-none">
          <input
            v-model="replaceMode"
            type="checkbox"
            class="size-3.5 accent-primary"
          >
          Replace entire value
          <span class="text-muted-foreground">(DEL + recreate; removes anything you didn't list)</span>
        </label>

        <div v-if="op !== 'delete'" class="space-y-1">
          <Label class="text-xs">
            TTL (seconds)
            <span class="text-muted-foreground font-normal ml-1">— leave blank for no expiry</span>
          </Label>
          <Input
            v-model="ttlInput"
            type="text"
            inputmode="numeric"
            placeholder="3600"
            class="font-mono text-xs"
          />
          <p v-if="ttlInvalid" class="text-xs text-destructive">TTL must be a whole number of seconds.</p>
        </div>

        <p v-if="op === 'delete'" class="text-xs text-muted-foreground">
          Deletes the key <code class="font-mono">{{ redisKey }}</code> entirely. Not reversible from this UI.
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
