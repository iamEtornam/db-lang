export type FilterOperator = 'contains' | 'equals' | 'greater' | 'less' | 'is_null' | 'not_null'
export interface ColumnFilter { column: string; operator: FilterOperator; value: string }
export type ValueKind = 'number' | 'boolean' | 'text'

export function cellText(value: unknown, pretty = false): string {
  if (value === undefined) return ''
  if (value === null) return 'null'
  if (typeof value === 'object') return JSON.stringify(value, null, pretty ? 2 : undefined)
  return String(value)
}
export function columnKind(rows: Record<string, unknown>[], column: string): ValueKind {
  const values = rows.map(row => row[column]).filter(value => value !== null && value !== undefined)
  if (values.length && values.every(value => typeof value === 'number')) return 'number'
  if (values.length && values.every(value => typeof value === 'boolean')) return 'boolean'
  return 'text'
}
export function filterError(filter: ColumnFilter, kind: ValueKind): string | null {
  if (filter.operator === 'is_null' || filter.operator === 'not_null') return null
  if (filter.operator === 'greater' || filter.operator === 'less' || (filter.operator === 'equals' && kind === 'number')) {
    if (!filter.value.trim() || !Number.isFinite(Number(filter.value))) return 'Enter a finite number for this filter.'
  }
  if (filter.operator === 'equals' && kind === 'boolean' && !['true', 'false'].includes(filter.value)) return 'Choose true or false for this filter.'
  return null
}
export function matchesFilter(value: unknown, filter: ColumnFilter, kind: ValueKind): boolean {
  if (filter.operator === 'is_null') return value === null
  if (filter.operator === 'not_null') return value !== null && value !== undefined
  if (filterError(filter, kind)) return false
  if (filter.operator === 'contains') return cellText(value).toLowerCase().includes(filter.value.toLowerCase())
  if (filter.operator === 'greater' || filter.operator === 'less') {
    if (typeof value !== 'number') return false
    return filter.operator === 'greater' ? value > Number(filter.value) : value < Number(filter.value)
  }
  if (kind === 'number') return typeof value === 'number' && value === Number(filter.value)
  if (kind === 'boolean') return typeof value === 'boolean' && value === (filter.value === 'true')
  return value !== null && value !== undefined && cellText(value) === filter.value
}

export function filterRows(rows: Record<string, unknown>[], search: string, filters: ColumnFilter[]): Record<string, unknown>[] {
  const term = search.toLowerCase()
  const kinds = new Map([...new Set(filters.map(filter => filter.column))].map(column => [column, columnKind(rows, column)]))
  return rows.filter(row => (!term || Object.values(row).some(value => cellText(value).toLowerCase().includes(term)))
    && filters.every(filter => matchesFilter(row[filter.column], filter, kinds.get(filter.column)!)))
}

export function compareCells(left: unknown, right: unknown): number {
  if (left === right) return 0
  if (left === undefined) return -1
  if (right === undefined) return 1
  if (left === null) return -1
  if (right === null) return 1
  if (typeof left !== typeof right) return typeof left < typeof right ? -1 : 1
  if (typeof left === 'number' && typeof right === 'number') return left - right
  if (typeof left === 'boolean' && typeof right === 'boolean') return Number(left) - Number(right)
  return cellText(left).localeCompare(cellText(right))
}
