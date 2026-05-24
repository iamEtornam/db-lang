<script setup lang="ts">
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '~/components/ui/dialog'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Label } from '~/components/ui/label'
import { Badge } from '~/components/ui/badge'
import { buildCreateTable, type ColumnDef } from '~/lib/sql'

const props = defineProps<{
  engine: string
  /** Default schema. Editable in the form for postgres; hidden for
   *  mysql/mariadb (uses the connection's current database) and sqlite
   *  (no schema concept). */
  defaultSchema?: string | null
}>()

const emit = defineEmits<{
  /** Emitted with the generated DDL when the user clicks Preview. Parent
   *  hands it to SqlConfirmDialog. */
  confirm: [sql: string]
}>()

const open = defineModel<boolean>('open', { default: false })

const tableName = ref('')
const tableSchema = ref(props.defaultSchema ?? '')
const formError = ref<string | null>(null)

interface ColumnRow extends ColumnDef {
  id: number
}

let _nextId = 0
const nextId = () => ++_nextId

function freshColumn(overrides: Partial<ColumnRow> = {}): ColumnRow {
  return {
    id: nextId(),
    name: '',
    dataType: '',
    nullable: true,
    default: '',
    isPrimaryKey: false,
    ...overrides,
  }
}

// Seed a reasonable default: one `id` column marked PK on a sensible
// per-engine integer type. Users edit or remove it as needed.
function defaultColumns(engine: string): ColumnRow[] {
  const idType = engine === 'postgres' ? 'bigserial'
    : (engine === 'mysql' || engine === 'mariadb') ? 'BIGINT AUTO_INCREMENT'
    : 'INTEGER'  // sqlite — INTEGER PRIMARY KEY is the conventional auto-increment
  return [
    freshColumn({ name: 'id', dataType: idType, nullable: false, isPrimaryKey: true }),
  ]
}

const columns = ref<ColumnRow[]>(defaultColumns(props.engine))

watch(() => [open.value, props.engine, props.defaultSchema] as const, ([isOpen, engine, schema]) => {
  if (isOpen) {
    tableName.value = ''
    tableSchema.value = schema ?? ''
    columns.value = defaultColumns(engine)
    formError.value = null
  }
})

const showSchemaField = computed(() => props.engine === 'postgres')

function addColumn() {
  columns.value = [...columns.value, freshColumn()]
}

function removeColumn(id: number) {
  if (columns.value.length === 1) return
  columns.value = columns.value.filter(c => c.id !== id)
}

function togglePk(id: number) {
  // Only one PK at a time — flipping one on flips the others off. The DDL
  // builder enforces this too but enforcing in the UI prevents accidental
  // multi-PK preview confusion.
  columns.value = columns.value.map(c => ({
    ...c,
    isPrimaryKey: c.id === id ? !c.isPrimaryKey : false,
  }))
}

function onPreview() {
  formError.value = null
  if (!tableName.value.trim()) {
    formError.value = 'Table name is required.'
    return
  }
  const colDefs: ColumnDef[] = []
  for (const c of columns.value) {
    if (!c.name.trim()) {
      formError.value = 'Every column needs a name.'
      return
    }
    if (!c.dataType.trim()) {
      formError.value = `Column '${c.name}' needs a data type.`
      return
    }
    colDefs.push({
      name: c.name.trim(),
      dataType: c.dataType.trim(),
      nullable: c.nullable,
      default: c.default?.trim() || undefined,
      isPrimaryKey: c.isPrimaryKey,
    })
  }
  let sql: string
  try {
    sql = buildCreateTable(props.engine, tableName.value.trim(), tableSchema.value || null, colDefs)
  }
  catch (e) {
    formError.value = (e as Error).message
    return
  }
  emit('confirm', sql)
  open.value = false
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-3xl max-h-[85vh] flex flex-col overflow-hidden">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Icon name="lucide:plus-circle" class="size-5" />
          New table
          <Badge variant="outline">{{ engine }}</Badge>
        </DialogTitle>
        <DialogDescription>
          Fill in the columns. You can review and hand-edit the generated DDL on the next step.
        </DialogDescription>
      </DialogHeader>

      <div class="overflow-y-auto flex-1 space-y-3 py-2 pr-1">
        <!-- Table name + schema -->
        <div class="grid gap-2 sm:grid-cols-2">
          <div class="space-y-1">
            <Label class="text-xs">Table name <span class="text-destructive">*</span></Label>
            <Input v-model="tableName" placeholder="users" class="font-mono text-xs" />
          </div>
          <div v-if="showSchemaField" class="space-y-1">
            <Label class="text-xs">Schema</Label>
            <Input v-model="tableSchema" placeholder="public" class="font-mono text-xs" />
          </div>
        </div>

        <!-- Columns header -->
        <div class="flex items-center justify-between pt-1">
          <Label class="text-xs">Columns</Label>
          <Button variant="outline" size="sm" class="h-7 text-xs" @click="addColumn">
            <Icon name="lucide:plus" class="size-3.5" />
            Add column
          </Button>
        </div>

        <!-- Columns table -->
        <div class="border border-border rounded-md overflow-hidden">
          <div class="grid grid-cols-[1fr_1fr_auto_1fr_auto_auto] gap-2 px-2 py-1.5 bg-muted/50 text-[11px] font-medium text-muted-foreground border-b border-border">
            <div>Name</div>
            <div>Type</div>
            <div class="text-center">Null</div>
            <div>Default</div>
            <div class="text-center">PK</div>
            <div />
          </div>
          <div
            v-for="col in columns"
            :key="col.id"
            class="grid grid-cols-[1fr_1fr_auto_1fr_auto_auto] items-center gap-2 px-2 py-1.5 border-b border-border last:border-b-0"
          >
            <Input v-model="col.name" placeholder="column_name" class="h-7 font-mono text-xs" />
            <Input v-model="col.dataType" placeholder="text" class="h-7 font-mono text-xs" />
            <div class="text-center">
              <input v-model="col.nullable" type="checkbox" class="size-3.5 accent-primary cursor-pointer">
            </div>
            <Input v-model="col.default" placeholder="(none)" class="h-7 font-mono text-xs" />
            <div class="text-center">
              <input
                type="checkbox"
                class="size-3.5 accent-primary cursor-pointer"
                :checked="col.isPrimaryKey"
                @change="togglePk(col.id)"
              >
            </div>
            <button
              class="text-muted-foreground hover:text-destructive transition-colors p-1"
              :disabled="columns.length === 1"
              :class="columns.length === 1 ? 'opacity-30 cursor-not-allowed' : ''"
              title="Remove column"
              @click="removeColumn(col.id)"
            >
              <Icon name="lucide:trash-2" class="size-3.5" />
            </button>
          </div>
        </div>

        <p v-if="formError" class="text-xs text-destructive">{{ formError }}</p>
      </div>

      <DialogFooter>
        <Button variant="outline" @click="open = false">Cancel</Button>
        <Button @click="onPreview">
          <Icon name="lucide:arrow-right" class="size-4" />
          Preview SQL
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
