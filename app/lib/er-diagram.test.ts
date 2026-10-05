import { describe, expect, it } from 'vitest'
import { buildErGraph, tableId, columnHandle } from './er-diagram'
import type { DiagramTable, ForeignKey } from '~/types/database'
const table = (schema: string | null, name: string): DiagramTable => ({ schema, name, table_type: 'TABLE', columns: [], columns_error: null })
const fk = (source_schema: string | null, target_schema: string | null, source_column = 'owner_id'): ForeignKey => ({ source_schema, target_schema, source_table: 'items', target_table: 'items', source_column, target_column: 'id', relationship_type: null })

describe('ER graph', () => {
  it('separates equal names in different schemas and keeps exact column endpoints', () => {
    const graph = buildErGraph({ tables: [table('public', 'items'), table('archive', 'items')], relationships: [fk('public', 'archive')] })
    expect(graph.nodes).toHaveLength(2)
    expect(graph.edges[0]).toMatchObject({ source: tableId('public', 'items'), target: tableId('archive', 'items'), sourceHandle: columnHandle('source', 'owner_id'), targetHandle: columnHandle('target', 'id') })
    expect(graph.nodes.every(n => n.data?.columns.length)).toBe(true)
  })
  it('preserves composite and self-reference column links, deduplicating repeated metadata', () => {
    const first = fk(null, null, 'first')
    const graph = buildErGraph({ tables: [table(null, 'items')], relationships: [first, fk(null, null, 'second'), first] })
    expect(graph.edges).toHaveLength(2)
    expect(graph.edges.every(e => e.source === e.target)).toBe(true)
    expect(graph.nodes[0]?.data?.columns.map(c => c.name)).toEqual(['first', 'id', 'second'])
  })
  it('labels external targets and retains introspection errors without mutating the snapshot', () => {
    const source = { ...table('public', 'items'), columns_error: 'Permission denied' }
    const graph = buildErGraph({ tables: [source], relationships: [fk('public', 'external')] })
    expect(graph.nodes.find(n => n.id === tableId('external', 'items'))?.data?.external).toBe(true)
    expect(graph.nodes.find(n => n.id === tableId('public', 'items'))?.data?.error).toBe('Permission denied')
    expect(source.columns).toEqual([])
  })
  it('handles empty schemas and delimiter-containing names without identity collisions', () => {
    expect(buildErGraph({ tables: [], relationships: [] })).toEqual({ nodes: [], edges: [] })
    expect(tableId('a.b', 'c')).not.toBe(tableId('a', 'b.c'))
    expect(columnHandle('source', 'a')).not.toBe(columnHandle('target', 'a'))
  })
})
