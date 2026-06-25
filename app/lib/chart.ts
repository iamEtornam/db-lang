// Chart data transforms (issue #10).
//
// Pure functions that turn raw query rows (the uniform `Record<string, unknown>[]`
// shape every driver returns) into the `{ label, value }[]` series the local
// SVG/HTML renderer consumes. Kept dependency-free and side-effect-free so the
// aggregate/group-by/top-N logic is unit-testable in isolation.

import type { AggregateFn, ChartDefinition } from '~/types/database'

export interface ChartPoint {
  label: string
  value: number
}

type Row = Record<string, unknown>

/** Coerce an arbitrary cell to a finite number, or null if it isn't numeric. */
export function toNumber(v: unknown): number | null {
  if (typeof v === 'number') return Number.isFinite(v) ? v : null
  if (typeof v === 'boolean') return v ? 1 : 0
  if (typeof v === 'string' && v.trim() !== '') {
    const n = Number(v)
    return Number.isFinite(n) ? n : null
  }
  return null
}

/**
 * Infer whether a column is numeric by sampling its non-null values.
 * Used to pre-select sensible value-field candidates in the UI.
 */
export function inferNumericColumns(rows: Row[], columns: string[]): string[] {
  return columns.filter((col) => {
    let seen = 0
    let numeric = 0
    for (const row of rows) {
      const cell = row[col]
      if (cell === null || cell === undefined || cell === '') continue
      seen++
      if (toNumber(cell) !== null) numeric++
      if (seen >= 20) break
    }
    return seen > 0 && numeric === seen
  })
}

function applyAggregate(fn: AggregateFn, values: number[]): number {
  if (fn === 'count') return values.length
  if (values.length === 0) return 0
  switch (fn) {
    case 'sum': return values.reduce((a, b) => a + b, 0)
    case 'avg': return values.reduce((a, b) => a + b, 0) / values.length
    // reduce, not Math.min/max(...values): spreading tens of thousands of rows
    // overflows the call stack. values is non-empty here (guarded above).
    case 'min': return values.reduce((min, v) => v < min ? v : min, values[0]!)
    case 'max': return values.reduce((max, v) => v > max ? v : max, values[0]!)
    default: return values[0]!
  }
}

/**
 * Build chart points from rows per a ChartDefinition.
 *
 * - aggregate 'none' → one point per row (no grouping).
 * - aggregate 'count' → counts rows per category (value field ignored).
 * - other aggregates → group rows by category, fold the value field.
 * - topN > 0 → keep the N largest points by value (descending).
 *
 * Insertion order of categories is preserved (Map keeps first-seen order),
 * which matters for time-like categories that arrive already sorted.
 */
export function buildChartPoints(rows: Row[], def: ChartDefinition): ChartPoint[] {
  if (!rows.length || !def.categoryField) return []

  let points: ChartPoint[]

  if (def.aggregate === 'none') {
    points = rows.map(row => ({
      label: String(row[def.categoryField] ?? ''),
      value: toNumber(row[def.valueField]) ?? 0,
    }))
  }
  else {
    // Group by category, collecting numeric values for the value field.
    const groups = new Map<string, number[]>()
    for (const row of rows) {
      const label = String(row[def.categoryField] ?? '')
      const bucket = groups.get(label) ?? []
      if (def.aggregate === 'count') {
        bucket.push(1) // count ignores the value field
      }
      else {
        const n = toNumber(row[def.valueField])
        if (n !== null) bucket.push(n)
      }
      groups.set(label, bucket)
    }
    points = Array.from(groups.entries()).map(([label, values]) => ({
      label,
      value: applyAggregate(def.aggregate, values),
    }))
  }

  if (def.topN && def.topN > 0 && points.length > def.topN) {
    points = [...points].sort((a, b) => b.value - a.value).slice(0, def.topN)
  }

  return points
}
