<script setup lang="ts">
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '~/components/ui/dialog'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Label } from '~/components/ui/label'
import { Badge } from '~/components/ui/badge'
import {
  buildAddColumn,
  buildRenameColumn,
  buildAlterColumnType,
  type ColumnDef,
} from '~/lib/sql'

type Mode = 'add' | 'rename' | 'change-type'

const props = defineProps<{
  mode: Mode
  engine: string
  tableName: string
  schema: string | null
  /** For rename + change-type: the column being modified. */
  columnName?: string
  /** For change-type: the current data type, shown read-only for context. */
  currentType?: string
}>()

const emit = defineEmits<{
  confirm: [sql: string]
}>()

const open = defineModel<boolean>('open', { default: false })

// === Add-mode state ===
const addName = ref('')
const addType = ref('')
const addNullable = ref(true)
const addDefault = ref('')

// === Rename-mode state ===
const renameNew = ref('')

// === Change-type-mode state ===
const newType = ref('')
const usingExpr = ref('')

const formError = ref<string | null>(null)

watch(() => [open.value, props.mode, props.columnName, props.currentType] as const, ([isOpen, mode, colName, currType]) => {
  if (!isOpen) return
  formError.value = null
  if (mode === 'add') {
    addName.value = ''
    addType.value = ''
    addNullable.value = true
    addDefault.value = ''
  }
  else if (mode === 'rename') {
    renameNew.value = colName ?? ''
  }
  else if (mode === 'change-type') {
    newType.value = currType ?? ''
    usingExpr.value = ''
  }
})

const title = computed(() => {
  switch (props.mode) {
    case 'add': return 'Add column'
    case 'rename': return 'Rename column'
    case 'change-type': return 'Change column type'
  }
})

const sqliteChangeTypeBlocked = computed(
  () => props.mode === 'change-type' && props.engine === 'sqlite',
)

function onPreview() {
  formError.value = null
  let sql: string

  if (props.mode === 'add') {
    if (!addName.value.trim()) {
      formError.value = 'Column name is required.'
      return
    }
    if (!addType.value.trim()) {
      formError.value = 'Data type is required.'
      return
    }
    const col: ColumnDef = {
      name: addName.value.trim(),
      dataType: addType.value.trim(),
      nullable: addNullable.value,
      default: addDefault.value.trim() || undefined,
      isPrimaryKey: false,  // adding a PK as part of ADD COLUMN is engine-specific and rare; users hand-edit.
    }
    sql = buildAddColumn(props.engine, props.tableName, props.schema, col)
  }
  else if (props.mode === 'rename') {
    if (!props.columnName) {
      formError.value = 'Missing source column name.'
      return
    }
    const next = renameNew.value.trim()
    if (!next) {
      formError.value = 'New name is required.'
      return
    }
    if (next === props.columnName) {
      formError.value = 'New name is the same as the current name.'
      return
    }
    sql = buildRenameColumn(props.engine, props.tableName, props.schema, props.columnName, next)
  }
  else {
    // change-type
    if (sqliteChangeTypeBlocked.value) {
      formError.value = 'SQLite cannot change column types in place. To switch types, rebuild the table by hand or via your migration tool.'
      return
    }
    if (!props.columnName) {
      formError.value = 'Missing source column name.'
      return
    }
    if (!newType.value.trim()) {
      formError.value = 'New type is required.'
      return
    }
    sql = buildAlterColumnType(
      props.engine,
      props.tableName,
      props.schema,
      props.columnName,
      newType.value.trim(),
      usingExpr.value.trim() || undefined,
    )
  }

  emit('confirm', sql)
  open.value = false
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-xl">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Icon
            :name="mode === 'add' ? 'lucide:plus' : 'lucide:pencil'"
            class="size-5"
          />
          {{ title }}
          <Badge variant="outline">{{ tableName }}</Badge>
        </DialogTitle>
        <DialogDescription>
          You'll review and can hand-edit the generated DDL before it runs.
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-3 py-2">
        <!-- ADD COLUMN -->
        <template v-if="mode === 'add'">
          <div class="grid gap-2 sm:grid-cols-2">
            <div class="space-y-1">
              <Label class="text-xs">Column name <span class="text-destructive">*</span></Label>
              <Input v-model="addName" placeholder="created_at" class="font-mono text-xs" />
            </div>
            <div class="space-y-1">
              <Label class="text-xs">Data type <span class="text-destructive">*</span></Label>
              <Input v-model="addType" placeholder="timestamp" class="font-mono text-xs" />
            </div>
          </div>
          <div class="grid gap-2 sm:grid-cols-2">
            <div class="space-y-1">
              <Label class="text-xs">Default expression</Label>
              <Input v-model="addDefault" placeholder="now()" class="font-mono text-xs" />
              <p class="text-[11px] text-muted-foreground">Emitted as-is. Quote literals yourself: <code>'pending'</code></p>
            </div>
            <label class="flex items-end gap-2 cursor-pointer select-none pb-1">
              <input v-model="addNullable" type="checkbox" class="size-3.5 accent-primary mb-1.5">
              <span class="text-xs">Nullable</span>
            </label>
          </div>
        </template>

        <!-- RENAME COLUMN -->
        <template v-else-if="mode === 'rename'">
          <div class="space-y-1">
            <Label class="text-xs">Current name</Label>
            <Input :model-value="columnName" disabled class="font-mono text-xs" />
          </div>
          <div class="space-y-1">
            <Label class="text-xs">New name <span class="text-destructive">*</span></Label>
            <Input v-model="renameNew" class="font-mono text-xs" autofocus />
          </div>
        </template>

        <!-- CHANGE TYPE -->
        <template v-else>
          <div v-if="sqliteChangeTypeBlocked" class="text-xs text-destructive italic px-2 py-3 border border-dashed border-destructive/40 rounded">
            SQLite has no in-place column-type change. To switch types, rebuild the table — create a new one with the desired schema, copy data, drop the old. Not something this dialog automates yet.
          </div>
          <template v-else>
            <div class="grid gap-2 sm:grid-cols-2">
              <div class="space-y-1">
                <Label class="text-xs">Column</Label>
                <Input :model-value="columnName" disabled class="font-mono text-xs" />
              </div>
              <div class="space-y-1">
                <Label class="text-xs">Current type</Label>
                <Input :model-value="currentType" disabled class="font-mono text-xs" />
              </div>
            </div>
            <div class="space-y-1">
              <Label class="text-xs">New type <span class="text-destructive">*</span></Label>
              <Input v-model="newType" placeholder="varchar(255)" class="font-mono text-xs" />
            </div>
            <div v-if="engine === 'postgres'" class="space-y-1">
              <Label class="text-xs">
                USING expression
                <span class="text-muted-foreground font-normal ml-1">(postgres only — required if the cast isn't automatic)</span>
              </Label>
              <Input v-model="usingExpr" placeholder="column_name::varchar" class="font-mono text-xs" />
            </div>
          </template>
        </template>

        <p v-if="formError" class="text-xs text-destructive">{{ formError }}</p>
      </div>

      <DialogFooter>
        <Button variant="outline" @click="open = false">Cancel</Button>
        <Button :disabled="sqliteChangeTypeBlocked" @click="onPreview">
          <Icon name="lucide:arrow-right" class="size-4" />
          Preview SQL
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
