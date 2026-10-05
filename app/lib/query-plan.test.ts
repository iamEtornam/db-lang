import { describe, expect, it } from 'vitest'
import { planTree } from './query-plan'
import type { QueryPlan } from './query-plan'
const plan = (format: QueryPlan['format'], rows: Record<string, unknown>[]): QueryPlan => ({ format, rows, engine: '' })
describe('query plan trees', () => {
  it('preserves PostgreSQL hierarchy, zero estimates, and native detail fields', () => {
    const nodes = planTree(plan('postgres-json', [{ 'QUERY PLAN': [{ Plan: { 'Node Type': 'Nested Loop', 'Total Cost': 0, Plans: [{ 'Node Type': 'Index Scan', Schema: 'public', 'Relation Name': 'users', 'Plan Rows': 12, 'Index Cond': 'id = 1' }] } }] }]))
    expect(nodes[0]?.metrics).toEqual([{ label: 'Estimated cost', value: '0' }])
    expect(nodes[0]?.children[0]).toMatchObject({ operation: 'Index Scan', relation: 'public.users', details: { 'Index Cond': 'id = 1' } })
  })
  it('reads string JSON and MySQL nested loops without turning cost metadata into steps', () => {
    const tree = planTree(plan('mysql-json', [{ EXPLAIN: JSON.stringify({ query_block: { cost_info: { query_cost: '3.40' }, nested_loop: [{ table: { table_name: 'users', access_type: 'ALL', rows_examined_per_scan: 10, cost_info: { prefix_cost: '3.40' } } }, { table: { table_name: 'orders', access_type: 'ref' } }] } }) }]))
    expect(tree[0]?.metrics).toEqual([{ label: 'Estimated cost', value: '3.40' }])
    expect(tree[0]?.children).toHaveLength(1)
    expect(tree[0]?.children[0]?.children).toHaveLength(2)
    expect(tree[0]?.children[0]?.children[0]).toMatchObject({ relation: 'users', operation: 'table · ALL' })
  })
  it('links SQLite integer ids independently of input ordering', () => {
    const nodes = planTree(plan('sqlite-tree', [{ id: 7, parent: 3, detail: 'SEARCH items USING INDEX ix' }, { id: 3, parent: 0, detail: 'SCAN root' }]))
    expect(nodes).toHaveLength(1)
    expect(nodes[0]?.children[0]?.operation).toBe('SEARCH items USING INDEX ix')
  })
  it('preserves MySQL subquery and UNION wrapper flags beside structural children', () => {
    const query_block = { select_id: 2, table: { table_name: 'children', access_type: 'ref' } }
    const tree = planTree(plan('mysql-json', [{ EXPLAIN: { query_block: {
      attached_subqueries: [{ dependent: true, cacheable: false, query_block }],
      union_result: { query_specifications: [{ dependent: false, cacheable: true, query_block }] },
    } } }]))
    const subquery = tree[0]?.children[0]?.children[0]
    expect(subquery?.details).toEqual({ dependent: true, cacheable: false })
    expect(subquery?.children[0]?.children[0]?.relation).toBe('children')
    const union = tree[0]?.children[1]?.children[0]?.children[0]
    expect(union?.details).toEqual({ dependent: false, cacheable: true })
    expect(union?.children[0]?.details.select_id).toBe(2)
  })
  it('keeps empty plans and rejects malformed or cyclic output rather than showing invented steps', () => {
    expect(planTree(plan('sqlite-tree', []))).toEqual([])
    expect(() => planTree(plan('postgres-json', [{ 'QUERY PLAN': 'broken' }]))).toThrow()
    expect(() => planTree(plan('sqlite-tree', [{ id: 1, parent: 2, detail: 'a' }, { id: 2, parent: 1, detail: 'b' }]))).toThrow(/cyclic/)
    expect(() => planTree(plan('sqlite-tree', [{ id: 1, parent: 0, detail: 'a' }, { id: 1, parent: 0, detail: 'b' }]))).toThrow(/duplicate/)
  })
})
