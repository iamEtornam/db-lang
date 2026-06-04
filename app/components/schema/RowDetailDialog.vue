<script setup lang="ts">
import { toast } from 'vue-sonner'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '~/components/ui/dialog'
import { Button } from '~/components/ui/button'
import { Badge } from '~/components/ui/badge'
import type { ColumnInfo } from '~/types/database'

const props = defineProps<{
  /** The row to display, or null when nothing is selected. */
  row: Record<string, unknown> | null
  /** Column metadata for the selected table — used to show each field's
   *  declared type. May be empty for engines that don't report it. */
  columns: ColumnInfo[]
  /** Table / collection / node name, shown in the header. */
  tableName: string
}>()

const open = defineModel<boolean>('open', { default: false })

/** name -> data_type lookup so we can annotate each field with its type. */
const typeByName = computed(() => {
  const map = new Map<string, string>()
  for (const c of props.columns) map.set(c.name, c.data_type)
  return map
})

/** Field rows in the row's own key order — this matches how the preview
 *  table renders columns and keeps synthetic NoSQL fields (_id, _key …)
 *  visible even when they aren't in `columns`. */
const fields = computed<{ name: string; value: unknown }[]>(() => {
  if (!props.row) return []
  return Object.keys(props.row).map(name => ({ name, value: props.row![name] }))
})

function isNullish(val: unknown): boolean {
  return val === null || val === undefined
}

/** Pretty-printed string for objects/arrays; plain string otherwise. */
function displayValue(val: unknown): string {
  if (typeof val === 'object' && val !== null) return JSON.stringify(val, null, 2)
  return String(val)
}

function copyValue(val: unknown) {
  if (isNullish(val)) {
    navigator.clipboard.writeText('')
  }
  else {
    navigator.clipboard.writeText(displayValue(val))
  }
  toast.success('Copied to clipboard')
}

function copyRowJson() {
  if (!props.row) return
  navigator.clipboard.writeText(JSON.stringify(props.row, null, 2))
  toast.success('Copied row as JSON')
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-2xl max-h-[85vh] overflow-hidden flex flex-col">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Icon name="lucide:eye" class="size-5" />
          Row details
          <Badge variant="outline">{{ tableName }}</Badge>
        </DialogTitle>
        <DialogDescription>
          {{ fields.length }} {{ fields.length === 1 ? 'field' : 'fields' }} · read-only view of the selected row
        </DialogDescription>
      </DialogHeader>

      <div class="overflow-y-auto flex-1 space-y-3 py-2 pr-1">
        <div
          v-for="field in fields"
          :key="field.name"
          class="space-y-1"
        >
          <div class="flex items-center justify-between gap-2">
            <div class="flex items-center gap-1.5 min-w-0">
              <span class="text-xs font-mono font-medium text-foreground truncate">{{ field.name }}</span>
              <span v-if="typeByName.get(field.name)" class="text-[11px] font-mono text-muted-foreground">
                {{ typeByName.get(field.name) }}
              </span>
            </div>
            <button
              type="button"
              class="shrink-0 rounded p-1 text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
              title="Copy value"
              @click="copyValue(field.value)"
            >
              <Icon name="lucide:copy" class="size-3.5" />
            </button>
          </div>

          <div
            v-if="isNullish(field.value)"
            class="text-xs text-muted-foreground italic px-2.5 py-1.5 rounded border border-border bg-muted/30"
          >
            null
          </div>
          <pre
            v-else
            class="text-xs font-mono whitespace-pre-wrap break-words px-2.5 py-1.5 rounded border border-border bg-muted/30 max-h-64 overflow-y-auto"
          >{{ displayValue(field.value) }}</pre>
        </div>

        <p v-if="fields.length === 0" class="text-xs text-muted-foreground italic text-center py-6">
          No fields to display
        </p>
      </div>

      <div class="flex items-center justify-end gap-2 pt-1">
        <Button variant="outline" size="sm" @click="copyRowJson">
          <Icon name="lucide:clipboard-copy" class="size-4" />
          Copy as JSON
        </Button>
        <Button size="sm" @click="open = false">Close</Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
