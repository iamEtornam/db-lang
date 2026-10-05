import type { Edge, Node } from '@vue-flow/core'
import { MarkerType } from '@vue-flow/core'
import type { ColumnInfo, ErSchema, ForeignKey, TableInfo } from '~/types/database'

export interface DiagramNodeData {
  label: string
  columns: ColumnInfo[]
  error: string | null
  external: boolean
}

export function tableId(schema: string | null, name: string): string {
  return JSON.stringify([schema, name])
}
export function tableLabel(table: Pick<TableInfo, 'schema' | 'name'>): string {
  return table.schema ? `${table.schema}.${table.name}` : table.name
}
export function relationshipLabel(fk: ForeignKey): string {
  return `${tableLabel({ schema: fk.source_schema, name: fk.source_table })}.${fk.source_column} → ${tableLabel({ schema: fk.target_schema, name: fk.target_table })}.${fk.target_column}`
}
export function columnHandle(kind: 'source' | 'target', column: string): string {
  return JSON.stringify([kind, column])
}

export function buildErGraph(schema: ErSchema): { nodes: Node<DiagramNodeData>[]; edges: Edge[] } {
  const nodes = new Map<string, Node<DiagramNodeData>>()
  for (const table of schema.tables) {
    const id = tableId(table.schema, table.name)
    nodes.set(id, {
      id, type: 'table', position: { x: 0, y: 0 }, connectable: false,
      ariaLabel: tableLabel(table),
      data: { label: tableLabel(table), columns: [...table.columns], error: table.columns_error, external: false },
    })
  }
  const edges = new Map<string, Edge>()
  for (const fk of schema.relationships) {
    const source = tableId(fk.source_schema, fk.source_table)
    const target = tableId(fk.target_schema, fk.target_table)
    for (const [id, name, column] of [
      [source, tableLabel({ schema: fk.source_schema, name: fk.source_table }), fk.source_column],
      [target, tableLabel({ schema: fk.target_schema, name: fk.target_table }), fk.target_column],
    ]) {
      if (!nodes.has(id!)) {
        nodes.set(id!, {
          id: id!, type: 'table', position: { x: 0, y: 0 }, connectable: false, ariaLabel: name!,
          data: { label: name!, columns: [], error: null, external: true },
        })
      }
      const data = nodes.get(id!)!.data!
      // Keep a visible attachment even when column introspection was denied.
      if (!data.columns.some(c => c.name === column)) {
        data.columns.push({ name: column!, data_type: 'metadata unavailable', is_nullable: false,
          column_default: null, is_primary_key: false, is_foreign_key: false,
          referenced_table: null, referenced_column: null })
      }
    }
    const id = JSON.stringify([source, fk.source_column, target, fk.target_column])
    edges.set(id, {
      id, source, target, sourceHandle: columnHandle('source', fk.source_column),
      targetHandle: columnHandle('target', fk.target_column), type: 'smoothstep',
      ariaLabel: relationshipLabel(fk), markerEnd: MarkerType.ArrowClosed, selectable: false,
    })
  }
  const ordered = [...nodes.values()].sort((a, b) => a.data!.label.localeCompare(b.data!.label))
  // ponytail: deterministic rows, automatic graph layout if dense schemas need it.
  const columns = Math.max(1, Math.ceil(Math.sqrt(ordered.length)))
  let y = 0
  for (let start = 0; start < ordered.length; start += columns) {
    const row = ordered.slice(start, start + columns)
    for (const [index, node] of row.entries()) node.position = { x: index * 400, y }
    y += Math.max(...row.map(n => 140 + n.data!.columns.length * 36)) + 100
  }
  return { nodes: ordered, edges: [...edges.values()] }
}
