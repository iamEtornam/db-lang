import { describe, expect, it } from 'vitest'
import { buildChartPoints, inferNumericColumns, toNumber } from './chart'
import type { ChartDefinition } from '~/types/database'

function def(overrides: Partial<ChartDefinition> = {}): ChartDefinition {
  return {
    chartType: 'bar',
    categoryField: 'category',
    valueField: 'amount',
    aggregate: 'sum',
    topN: 0,
    ...overrides,
  }
}

const rows = [
  { category: 'A', amount: 10 },
  { category: 'B', amount: 5 },
  { category: 'A', amount: 20 },
  { category: 'C', amount: 7 },
  { category: 'B', amount: 15 },
]

describe('toNumber', () => {
  it('coerces numbers, numeric strings and booleans', () => {
    expect(toNumber(3.5)).toBe(3.5)
    expect(toNumber('42')).toBe(42)
    expect(toNumber(true)).toBe(1)
    expect(toNumber(false)).toBe(0)
  })
  it('returns null for non-numeric input', () => {
    expect(toNumber('abc')).toBeNull()
    expect(toNumber('')).toBeNull()
    expect(toNumber(null)).toBeNull()
    expect(toNumber(undefined)).toBeNull()
    expect(toNumber(Number.NaN)).toBeNull()
  })
})

describe('buildChartPoints', () => {
  it('sums values grouped by category', () => {
    expect(buildChartPoints(rows, def({ aggregate: 'sum' }))).toEqual([
      { label: 'A', value: 30 },
      { label: 'B', value: 20 },
      { label: 'C', value: 7 },
    ])
  })

  it('averages values grouped by category', () => {
    expect(buildChartPoints(rows, def({ aggregate: 'avg' }))).toEqual([
      { label: 'A', value: 15 },
      { label: 'B', value: 10 },
      { label: 'C', value: 7 },
    ])
  })

  it('counts rows per category, ignoring the value field', () => {
    expect(buildChartPoints(rows, def({ aggregate: 'count' }))).toEqual([
      { label: 'A', value: 2 },
      { label: 'B', value: 2 },
      { label: 'C', value: 1 },
    ])
  })

  it('takes min and max per category', () => {
    expect(buildChartPoints(rows, def({ aggregate: 'min' }))).toEqual([
      { label: 'A', value: 10 },
      { label: 'B', value: 5 },
      { label: 'C', value: 7 },
    ])
    expect(buildChartPoints(rows, def({ aggregate: 'max' }))).toEqual([
      { label: 'A', value: 20 },
      { label: 'B', value: 15 },
      { label: 'C', value: 7 },
    ])
  })

  it('emits one point per row when aggregate is none', () => {
    const points = buildChartPoints(rows, def({ aggregate: 'none' }))
    expect(points).toHaveLength(5)
    expect(points[0]).toEqual({ label: 'A', value: 10 })
  })

  it('keeps only the top N points by value', () => {
    expect(buildChartPoints(rows, def({ aggregate: 'sum', topN: 2 }))).toEqual([
      { label: 'A', value: 30 },
      { label: 'B', value: 20 },
    ])
  })

  it('preserves first-seen category order for sorted (time-like) input', () => {
    const sorted = [
      { category: '2024-01', amount: 1 },
      { category: '2024-02', amount: 2 },
      { category: '2024-03', amount: 3 },
    ]
    expect(buildChartPoints(sorted, def({ aggregate: 'sum' })).map(p => p.label)).toEqual([
      '2024-01', '2024-02', '2024-03',
    ])
  })

  it('treats missing/non-numeric values as skipped (0 for empty group)', () => {
    const dirty = [
      { category: 'A', amount: 'oops' },
      { category: 'A', amount: 5 },
      { category: 'B', amount: null },
    ]
    expect(buildChartPoints(dirty, def({ aggregate: 'sum' }))).toEqual([
      { label: 'A', value: 5 },
      { label: 'B', value: 0 },
    ])
  })

  it('returns empty for empty rows or missing category field', () => {
    expect(buildChartPoints([], def())).toEqual([])
    expect(buildChartPoints(rows, def({ categoryField: '' }))).toEqual([])
  })
})

describe('inferNumericColumns', () => {
  it('detects fully-numeric columns only', () => {
    const data = [
      { id: 1, name: 'x', score: '9.5' },
      { id: 2, name: 'y', score: '3' },
    ]
    expect(inferNumericColumns(data, ['id', 'name', 'score'])).toEqual(['id', 'score'])
  })
})
