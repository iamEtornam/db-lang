export interface QueryPlan {
  engine: string
  format: 'postgres-json' | 'mysql-json' | 'sqlite-tree'
  rows: Record<string, unknown>[]
}
export interface PlanNode {
  id: string
  operation: string
  relation: string | null
  metrics: { label: string; value: string }[]
  details: Record<string, unknown>
  children: PlanNode[]
}
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Expected a plan object')
  return value as Record<string, unknown>
}
function json(value: unknown): unknown {
  return typeof value === 'string' ? JSON.parse(value) : value
}
function metric(details: Record<string, unknown>, keys: [string, string][]) {
  return keys.flatMap(([key, label]) => {
    const value = details[key]
    return (typeof value === 'number' && Number.isFinite(value)) || typeof value === 'string'
      ? [{ label, value: String(value) }] : []
  })
}

/** Keep native plan estimates distinct from measured execution times. */
export function planTree(plan: QueryPlan): PlanNode[] {
  if (!plan.rows.length) return []
  let count = 0
  function budget(depth: number) {
    if (depth > 80 || ++count > 2000) throw new Error('This plan is too large for the tree view. Read the raw output instead.')
  }
  if (plan.format === 'postgres-json') {
    const roots = json(plan.rows[0]?.['QUERY PLAN'])
    if (!Array.isArray(roots)) throw new Error('PostgreSQL did not return a JSON plan array')
    function node(value: unknown, id: string, depth: number): PlanNode {
      budget(depth)
      const data = object(value)
      if (typeof data['Node Type'] !== 'string') throw new Error('PostgreSQL plan step has no node type')
      const { Plans, ...details } = data
      if (Plans !== undefined && !Array.isArray(Plans)) throw new Error('Invalid PostgreSQL child plans')
      const relation = typeof data['Relation Name'] === 'string' ? [data.Schema, data['Relation Name']].filter(Boolean).join('.') : null
      return { id, operation: data['Node Type'], relation,
        metrics: metric(data, [['Total Cost', 'Estimated cost'], ['Plan Rows', 'Estimated rows'], ['Plan Width', 'Row width']]),
        details, children: (Plans as unknown[] | undefined ?? []).map((child, i) => node(child, `${id}.${i}`, depth + 1)) }
    }
    return roots.map((root, i) => node(object(root).Plan, String(i), 0))
  }
  if (plan.format === 'mysql-json') {
    const row = plan.rows[0]!
    const key = Object.keys(row).find(key => key.toLowerCase() === 'explain')
    if (!key) throw new Error('MySQL did not return a JSON EXPLAIN column')
    const data = object(json(row[key]))
    function node(value: unknown, name: string, id: string, depth: number): PlanNode {
      budget(depth)
      const data = object(value)
      const children: PlanNode[] = []
      const details: Record<string, unknown> = {}
      for (const [key, value] of Object.entries(data)) {
        if (key !== 'cost_info' && value && typeof value === 'object' && !Array.isArray(value)) {
          children.push(node(value, key, `${id}.${children.length}`, depth + 1))
        }
        else if (Array.isArray(value) && value.some(v => v && typeof v === 'object')) {
          const groupId = `${id}.${children.length}`
          budget(depth + 1)
          const steps = value.map((child, i) => {
            const entries = Object.entries(object(child))
            const single = entries.length === 1 ? entries[0] : undefined
            if (single && single[1] && typeof single[1] === 'object' && !Array.isArray(single[1]) && single[0] !== 'cost_info') {
              return node(single[1], single[0], `${groupId}.${i}`, depth + 2)
            }
            // Subquery wrappers carry dependent/cacheable flags beside query_block.
            return node(child, `${key}_item_${i + 1}`, `${groupId}.${i}`, depth + 2)
          })
          children.push({ id: groupId, operation: key.replaceAll('_', ' '), relation: null, metrics: [], details: {}, children: steps })
        }
        else details[key] = value
      }
      const cost = data.cost_info ? object(data.cost_info) : {}
      return { id, operation: typeof data.access_type === 'string' ? `${name.replaceAll('_', ' ')} · ${data.access_type}` : name.replaceAll('_', ' '),
        relation: typeof data.table_name === 'string' ? data.table_name : null,
        metrics: [...metric(data, [['rows_examined_per_scan', 'Estimated rows per scan'], ['rows', 'Estimated rows'], ['filtered', 'Filtered %']]), ...metric(cost, [['query_cost', 'Estimated cost'], ['prefix_cost', 'Estimated prefix cost']])],
        details, children }
    }
    return Object.entries(data).map(([name, value], i) => node(value, name, String(i), 0))
  }
  if (plan.format === 'sqlite-tree') {
    const nodes = new Map<number, PlanNode>()
    for (const [i, row] of plan.rows.entries()) {
      budget(0)
      if (!Number.isInteger(row.id) || !Number.isInteger(row.parent) || typeof row.detail !== 'string') throw new Error('SQLite returned an invalid plan row')
      if (nodes.has(row.id as number)) throw new Error('SQLite returned duplicate plan identifiers')
      nodes.set(row.id as number, { id: String(i), operation: row.detail, relation: null, metrics: [], details: row, children: [] })
    }
    const roots: PlanNode[] = []
    for (const row of plan.rows) {
      const node = nodes.get(row.id as number)!
      const parent = nodes.get(row.parent as number)
      if (parent) parent.children.push(node)
      else roots.push(node)
    }
    function check(node: PlanNode, path: Set<string>, depth: number): number {
      if (depth > 80 || path.has(node.id)) throw new Error('SQLite returned a cyclic or excessively deep plan')
      const next = new Set(path).add(node.id)
      return 1 + node.children.reduce((sum, child) => sum + check(child, next, depth + 1), 0)
    }
    if (roots.reduce((sum, node) => sum + check(node, new Set(), 0), 0) !== nodes.size) throw new Error('SQLite returned a cyclic plan')
    return roots
  }
  throw new Error('Unsupported query plan format')
}
