<script setup lang="ts">
import { basicSetup } from 'codemirror'
import { Compartment, EditorState } from '@codemirror/state'
import { EditorView, keymap } from '@codemirror/view'
import { sql, PostgreSQL, MySQL, SQLite, type SQLNamespace } from '@codemirror/lang-sql'
import { javascript } from '@codemirror/lang-javascript'
import { json } from '@codemirror/lang-json'
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language'
import { tags } from '@lezer/highlight'
import { format } from 'sql-formatter'
import type { ColumnInfo, TableInfo } from '~/types/database'
import { Button } from '~/components/ui/button'

const props = defineProps<{
  modelValue: string
  engine: string
  tables: TableInfo[]
  tableColumns: Record<string, ColumnInfo[]>
  disabled?: boolean
}>()
const emit = defineEmits<{
  'update:modelValue': [value: string]
  execute: [query: string]
}>()
const host = ref<HTMLElement>()
const selection = ref('')
const formatError = ref('')
let view: EditorView | undefined
const language = new Compartment()
const editable = new Compartment()
const isSql = computed(() => ['postgres', 'mysql', 'mariadb', 'sqlite'].includes(props.engine))

function languageExtension() {
  if (isSql.value) {
    const schema: { [name: string]: SQLNamespace } = {}
    for (const table of props.tables) {
      const columns = (props.tableColumns[table.schema ? `${table.schema}.${table.name}` : table.name] ?? props.tableColumns[table.name] ?? []).map(c => c.name)
      if (table.schema) {
        const namespace = (schema[table.schema] ??= {}) as { [name: string]: SQLNamespace }
        namespace[table.name] = columns
      }
      else schema[table.name] = columns
    }
    return sql({
      dialect: props.engine === 'postgres' ? PostgreSQL : props.engine === 'sqlite' ? SQLite : MySQL,
      schema,
      defaultSchema: props.engine === 'postgres' ? 'public' : undefined,
      upperCaseKeywords: true,
    })
  }
  if (props.engine === 'mongodb') return javascript()
  if (['firestore', 'firebase_rtdb'].includes(props.engine)) return json()
  return []
}
function runSelection() {
  if (!view || props.disabled) return
  const selected = view.state.sliceDoc(view.state.selection.main.from, view.state.selection.main.to)
  const query = selected.trim() ? selected : view.state.doc.toString()
  if (query.trim()) emit('execute', query)
}
function formatQuery() {
  if (!view || !isSql.value) return
  try {
    const language = props.engine === 'postgres' ? 'postgresql' : props.engine === 'sqlite' ? 'sqlite' : 'mysql'
    const formatted = format(view.state.doc.toString(), { language })
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: formatted } })
    formatError.value = ''
  }
  catch (err) {
    formatError.value = `Could not format this query: ${err instanceof Error ? err.message : String(err)}`
  }
}
onMounted(() => {
  view = new EditorView({
    parent: host.value,
    state: EditorState.create({
      doc: props.modelValue,
      extensions: [
        keymap.of([{ key: 'Mod-Enter', run: () => { runSelection(); return true } }]),
        basicSetup,
        syntaxHighlighting(HighlightStyle.define([
          { tag: [tags.keyword, tags.operator], color: 'var(--foreground)', fontWeight: '600' },
          { tag: [tags.string, tags.number, tags.bool, tags.null], color: 'var(--foreground)', fontStyle: 'italic' },
          { tag: tags.comment, color: 'var(--muted-foreground)' },
          { tag: [tags.name, tags.punctuation], color: 'var(--foreground)' },
        ])),
        language.of(languageExtension()),
        editable.of(EditorView.editable.of(!props.disabled)),
        EditorView.contentAttributes.of({ 'aria-label': 'Database query', spellcheck: 'false' }),
        EditorView.updateListener.of(update => {
          if (update.docChanged) emit('update:modelValue', update.state.doc.toString())
          if (update.selectionSet || update.docChanged) {
            selection.value = update.state.sliceDoc(update.state.selection.main.from, update.state.selection.main.to)
          }
        }),
        EditorView.theme({
          '&': { color: 'var(--foreground)', backgroundColor: 'var(--background)', fontSize: '13px' },
          '.cm-scroller': { fontFamily: 'ui-monospace, monospace', minHeight: '120px', maxHeight: '260px', overflow: 'auto' },
          '.cm-content': { padding: '12px 0' },
          '.cm-line': { padding: '0 12px' },
          '.cm-gutters': { backgroundColor: 'var(--muted)', color: 'var(--muted-foreground)', border: 'none' },
          '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'var(--accent)' },
          '.cm-cursor': { borderLeftColor: 'var(--foreground)' },
          '&.cm-focused .cm-selectionBackground, .cm-selectionBackground': { backgroundColor: 'color-mix(in srgb, var(--foreground) 20%, var(--background))' },
          '.cm-tooltip-autocomplete > ul > li[aria-selected]': { backgroundColor: 'var(--accent)', color: 'var(--foreground)' },
          '.cm-tooltip': { backgroundColor: 'var(--popover)', color: 'var(--popover-foreground)', borderColor: 'var(--border)' },
        }),
      ],
    }),
  })
})
watch(() => props.modelValue, value => {
  if (view && value !== view.state.doc.toString()) {
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value } })
  }
})
watch(() => [props.engine, props.tables, props.tableColumns], () => {
  view?.dispatch({ effects: language.reconfigure(languageExtension()) })
}, { deep: true })
watch(() => props.disabled, value => {
  view?.dispatch({ effects: editable.reconfigure(EditorView.editable.of(!value)) })
})
onBeforeUnmount(() => view?.destroy())
</script>

<template>
  <div class="space-y-2">
    <div ref="host" class="rounded-md bg-background outline outline-1 outline-border focus-within:outline-ring" />
    <div class="flex flex-wrap items-center gap-3">
      <Button :disabled="disabled || !modelValue.trim()" size="sm" @click="runSelection">
        {{ selection.trim() ? 'Run selection' : 'Run query' }}
      </Button>
      <Button v-if="isSql" variant="ghost" size="sm" :disabled="disabled || !modelValue.trim()" @click="formatQuery">
        Format SQL
      </Button>
      <span class="text-xs text-muted-foreground">Ctrl / ⌘ Enter to run · Ctrl Space for suggestions</span>
    </div>
    <p v-if="formatError" role="alert" class="text-sm text-destructive">{{ formatError }}</p>
  </div>
</template>
