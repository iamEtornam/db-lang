<script setup lang="ts">
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '~/components/ui/dialog'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Textarea } from '~/components/ui/textarea'
import { Label } from '~/components/ui/label'
import { Badge } from '~/components/ui/badge'
import { buildInsert, buildUpdate, coerceCellInput, type PkBinding, type ValueBinding } from '~/lib/sql'
import type { ColumnInfo } from '~/types/database'

const props = defineProps<{
  mode: 'insert' | 'edit'
  engine: string
  tableName: string
  schema: string | null
  columns: ColumnInfo[]
  /** All PK columns. Single-element array for the common case; longer for
   *  composite PKs. Empty array means no PK and the dialog refuses to edit. */
  pkColumns: string[]
  initialRow: Record<string, unknown> | null
}>()

const emit = defineEmits<{
  /** Emitted with the generated SQL when the user clicks Save. Parent should
   *  hand it off to SqlConfirmDialog for final review + execution. */
  confirm: [sql: string]
}>()

const open = defineModel<boolean>('open', { default: false })

interface FieldState {
  /** String form bound to the input (always a string while editing). */
  text: string
  /** When true, value will be sent as NULL regardless of `text`. */
  isNull: boolean
}

const fields = ref<Record<string, FieldState>>({})
const formError = ref<string | null>(null)

/** Columns we still refuse to render an editor for. JSON/JSONB are
 *  Phase-4 editable; everything else here (binary blobs, arrays, geometry,
 *  XML) needs engine-specific cast syntax we don't model yet. */
function isReadOnlyType(dataType: string): boolean {
  const t = dataType.toLowerCase()
  return /\b(blob|bytea|geometry|xml)\b/.test(t) || /\barray\b/.test(t)
}

/** JSON / JSONB columns get a dedicated JSON-validating Textarea and the
 *  emitted ValueBinding carries the dataType so the SQL builder wraps with
 *  the right cast (::jsonb / CAST(... AS JSON) / plain text for sqlite). */
function isJsonType(dataType: string): boolean {
  return /\b(json|jsonb)\b/i.test(dataType)
}

/** Returns null on valid JSON, an error string otherwise. Used for the
 *  inline JSON column hint and the onSave gate. */
function jsonParseError(text: string): string | null {
  try {
    JSON.parse(text)
    return null
  }
  catch (e) {
    return (e as Error).message
  }
}

/** Compare a user-edited JSON text against the row's original JSON value
 *  (which may already be a parsed object from the read path, or a string).
 *  Returns true when the two are semantically equal — handles reformatted
 *  whitespace and key reordering so we don't trigger spurious UPDATE
 *  emissions when nothing meaningful changed. */
function sameJsonAsOriginal(edited: unknown, original: unknown): boolean {
  if (edited === null || edited === undefined) {
    return original === null || original === undefined
  }
  try {
    const editedParsed = typeof edited === 'string' ? JSON.parse(edited) : edited
    const originalParsed = typeof original === 'string'
      ? (() => { try { return JSON.parse(original) } catch { return original } })()
      : original
    return JSON.stringify(editedParsed) === JSON.stringify(originalParsed)
  }
  catch {
    return String(edited) === String(original)
  }
}

/** Auto-managed columns we should not present on Insert. The user can still
 *  add them by hand-editing the SQL in the confirm dialog. */
function isAutoColumn(col: ColumnInfo): boolean {
  const def = (col.column_default ?? '').toLowerCase()
  return /nextval\(|autoincrement|auto_increment/.test(def)
    || /\bserial\b/.test(col.data_type.toLowerCase())
}

const editableColumns = computed(() => {
  return props.columns.filter((col) => {
    if (props.mode === 'insert' && isAutoColumn(col)) return false
    return true
  })
})

/** Decide once (at open time) whether a column gets a multi-line textarea
 *  or a single-line input. Swapping mid-typing based on live text length
 *  causes the focused element to unmount and the cursor to jump. */
function shouldUseTextarea(col: ColumnInfo): boolean {
  if (isReadOnlyType(col.data_type)) return false
  const t = col.data_type.toLowerCase()
  if (/text|varchar\(\s*[5-9]\d{2,}|character\s+varying/.test(t)) return true
  const initial = props.initialRow?.[col.name]
  if (typeof initial === 'string' && initial.length > 80) return true
  return false
}

function seedFields() {
  const next: Record<string, FieldState> = {}
  for (const col of props.columns) {
    if (props.mode === 'edit' && props.initialRow) {
      const v = props.initialRow[col.name]
      // JSON columns: pretty-print so the editor is readable. Existing
      // strings that happen to be JSON also get parsed-and-pretty-printed
      // for consistency (best-effort — non-JSON strings pass through).
      let text: string
      if (v === null || v === undefined) {
        text = ''
      }
      else if (isJsonType(col.data_type)) {
        try {
          const parsed = typeof v === 'string' ? JSON.parse(v) : v
          text = JSON.stringify(parsed, null, 2)
        }
        catch {
          text = String(v)
        }
      }
      else if (typeof v === 'object') {
        text = JSON.stringify(v)
      }
      else {
        text = String(v)
      }
      next[col.name] = {
        text,
        isNull: v === null || v === undefined,
      }
    }
    else {
      // Insert mode: start every field empty and editable. NULL is opt-in
      // via the toggle — defaulting it on would force the user to click
      // through every nullable column before they could type.
      next[col.name] = { text: '', isNull: false }
    }
  }
  fields.value = next
  formError.value = null
}

// Re-seed every time the dialog opens or the row being edited changes.
watch(() => [open.value, props.initialRow, props.mode] as const, ([isOpen]) => {
  if (isOpen) seedFields()
})

function toggleNull(name: string) {
  const f = fields.value[name]
  if (!f) return
  f.isNull = !f.isNull
}

const pkColumnSet = computed(() => new Set(props.pkColumns))
const isPkColumn = (name: string) => pkColumnSet.value.has(name)

function onSave() {
  formError.value = null

  let pkBindings: PkBinding[] = []
  if (props.mode === 'edit') {
    if (props.pkColumns.length === 0) {
      formError.value = 'Cannot update — this table has no primary key.'
      return
    }
    if (!props.initialRow) {
      formError.value = 'Internal error: missing row state.'
      return
    }
    // Every PK column must have a non-null value to target a unique row;
    // a null PK value would silently match arbitrary rows.
    for (const col of props.pkColumns) {
      const v = props.initialRow[col]
      if (v === null || v === undefined) {
        formError.value = `Primary key '${col}' is null on this row — cannot target it safely.`
        return
      }
      pkBindings.push({ column: col, value: v })
    }
  }

  const bindings: ValueBinding[] = []
  const missingRequired: string[] = []
  const jsonErrors: string[] = []
  for (const col of editableColumns.value) {
    if (isReadOnlyType(col.data_type)) continue
    if (props.mode === 'edit' && isPkColumn(col.name)) continue
    const f = fields.value[col.name]
    if (!f) continue

    const colIsJson = isJsonType(col.data_type)

    if (props.mode === 'insert') {
      // Insert mode: only include the column if the user opted into NULL or
      // typed something. Blank-and-not-NULL means "let the DB use its
      // default (or NULL for nullable cols without one)". This preserves
      // DEFAULT now() etc. instead of clobbering them with NULL.
      if (!f.isNull && f.text === '') {
        if (!col.is_nullable && col.column_default == null) {
          missingRequired.push(col.name)
        }
        continue
      }
      if (colIsJson && !f.isNull) {
        const err = jsonParseError(f.text)
        if (err) {
          jsonErrors.push(`${col.name}: ${err}`)
          continue
        }
      }
      // JSON columns: keep the raw user text. quoteValue will single-quote
      // and cast it appropriately for the engine. Non-JSON columns go via
      // the usual coercion (numbers / bools / nulls / strings).
      const value = colIsJson && !f.isNull ? f.text : coerceCellInput(f.text, f.isNull, col.data_type)
      bindings.push({ column: col.name, value, dataType: col.data_type })
    }
    else {
      // Edit mode: only emit columns the user actually changed — keeps the
      // UPDATE statement minimal and avoids overwriting concurrent edits
      // on untouched columns.
      if (colIsJson && !f.isNull) {
        const err = jsonParseError(f.text)
        if (err) {
          jsonErrors.push(`${col.name}: ${err}`)
          continue
        }
      }
      const value = colIsJson && !f.isNull ? f.text : coerceCellInput(f.text, f.isNull, col.data_type)
      const original = props.initialRow?.[col.name] ?? null
      // For JSON columns we compare the canonicalised forms — typing a
      // semantically-equivalent JSON (e.g. different key order) should
      // still count as no change to keep the UPDATE minimal.
      const sameAsOriginal = colIsJson
        ? sameJsonAsOriginal(value, original)
        : (value === original
          || (value === null && (original === null || original === undefined))
          || String(value) === String(original))
      if (sameAsOriginal) continue
      bindings.push({ column: col.name, value, dataType: col.data_type })
    }
  }

  if (jsonErrors.length > 0) {
    formError.value = `Invalid JSON in ${jsonErrors.length === 1 ? 'column' : 'columns'}: ${jsonErrors.join('; ')}`
    return
  }

  if (missingRequired.length > 0) {
    formError.value = `Required field${missingRequired.length === 1 ? '' : 's'} missing: ${missingRequired.join(', ')}`
    return
  }

  if (bindings.length === 0) {
    formError.value = props.mode === 'edit'
      ? 'No changes to save.'
      : 'At least one column is required.'
    return
  }

  let sql: string
  if (props.mode === 'insert') {
    sql = buildInsert(props.engine, props.tableName, props.schema, bindings)
  }
  else {
    sql = buildUpdate(
      props.engine,
      props.tableName,
      props.schema,
      bindings,
      pkBindings,
    )
  }

  emit('confirm', sql)
  open.value = false
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-xl max-h-[85vh] overflow-hidden flex flex-col">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Icon :name="mode === 'insert' ? 'lucide:plus' : 'lucide:pencil'" class="size-5" />
          {{ mode === 'insert' ? 'Insert row' : 'Edit row' }}
          <Badge variant="outline">{{ tableName }}</Badge>
        </DialogTitle>
        <DialogDescription>
          {{ mode === 'insert'
            ? 'Fill in the columns. Auto-generated columns (e.g. serial PKs) are skipped — add them by hand-editing the SQL in the next step if needed.'
            : 'Edit cell values. Only changed columns are included in the UPDATE.' }}
        </DialogDescription>
      </DialogHeader>

      <div class="overflow-y-auto flex-1 space-y-3 py-2 pr-1">
        <div
          v-for="col in editableColumns"
          :key="col.name"
          class="space-y-1"
        >
          <div class="flex items-center justify-between gap-2">
            <Label class="text-xs font-mono">
              {{ col.name }}
              <span class="text-muted-foreground font-normal ml-1.5">{{ col.data_type }}</span>
              <Badge v-if="col.is_primary_key" class="text-[10px] py-0 h-4 ml-1.5">PK</Badge>
              <span v-if="!col.is_nullable" class="text-destructive ml-1">*</span>
            </Label>
            <button
              v-if="col.is_nullable && !(mode === 'edit' && isPkColumn(col.name))"
              type="button"
              class="text-[10px] uppercase tracking-wide px-1.5 py-0.5 rounded border transition-colors"
              :class="fields[col.name]?.isNull
                ? 'border-primary bg-primary/10 text-primary'
                : 'border-border text-muted-foreground hover:text-foreground'"
              @click="toggleNull(col.name)"
            >
              NULL
            </button>
          </div>

          <div v-if="isReadOnlyType(col.data_type)" class="text-xs text-muted-foreground italic px-2 py-1.5 border border-dashed rounded">
            {{ col.data_type }} values aren't editable yet — edit the SQL directly in the next step.
          </div>
          <template v-else-if="isJsonType(col.data_type)">
            <Textarea
              v-model="fields[col.name].text"
              :disabled="fields[col.name]?.isNull"
              rows="6"
              spellcheck="false"
              placeholder='{"key": "value"}'
              class="font-mono text-xs"
            />
            <p
              v-if="!fields[col.name]?.isNull && fields[col.name]?.text.trim() !== '' && jsonParseError(fields[col.name].text)"
              class="text-[11px] text-destructive"
            >
              {{ jsonParseError(fields[col.name].text) }}
            </p>
          </template>
          <Textarea
            v-else-if="shouldUseTextarea(col)"
            v-model="fields[col.name].text"
            :disabled="fields[col.name]?.isNull || (mode === 'edit' && isPkColumn(col.name))"
            rows="3"
            class="font-mono text-xs"
          />
          <Input
            v-else
            v-model="fields[col.name].text"
            :disabled="fields[col.name]?.isNull || (mode === 'edit' && isPkColumn(col.name))"
            :placeholder="fields[col.name]?.isNull ? 'NULL' : col.column_default ?? ''"
            class="font-mono text-xs"
          />
        </div>

        <p v-if="formError" class="text-xs text-destructive">{{ formError }}</p>
      </div>

      <DialogFooter>
        <Button variant="outline" @click="open = false">Cancel</Button>
        <Button @click="onSave">
          <Icon name="lucide:arrow-right" class="size-4" />
          Preview SQL
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
