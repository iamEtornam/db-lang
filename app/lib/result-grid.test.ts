import { describe, expect, it } from 'vitest'
import { cellText, compareCells, columnKind, filterError, filterRows } from './result-grid'
const rows = [
  { amount: 0, active: false, note: null, data: { label: 'Alice' } },
  { amount: 12.5, active: true, note: 'null', data: { label: 'Bob' } },
  { amount: -3, active: true, data: { label: 'Carol' } },
]
describe('result grid filtering', () => {
  it('compares native numeric and boolean values without string truthiness', () => {
    expect(columnKind(rows, 'amount')).toBe('number')
    expect(filterRows(rows, '', [{ column: 'amount', operator: 'equals', value: '0' }])).toEqual([rows[0]])
    expect(filterRows(rows, '', [{ column: 'active', operator: 'equals', value: 'false' }])).toEqual([rows[0]])
    expect(filterRows(rows, '', [{ column: 'amount', operator: 'greater', value: '1' }])).toEqual([rows[1]])
  })
  it('keeps null, missing cells, and the string null distinct', () => {
    expect(filterRows(rows, '', [{ column: 'note', operator: 'is_null', value: '' }])).toEqual([rows[0]])
    expect(filterRows(rows, '', [{ column: 'note', operator: 'equals', value: 'null' }])).toEqual([rows[1]])
    expect(filterRows(rows, '', [{ column: 'note', operator: 'not_null', value: '' }])).toEqual([rows[1]])
  })
  it('searches structured JSON and combines column filters', () => {
    expect(filterRows(rows, 'alice', [])).toEqual([rows[0]])
    expect(filterRows(rows, '', [{ column: 'active', operator: 'equals', value: 'true' }, { column: 'amount', operator: 'less', value: '0' }])).toEqual([rows[2]])
    expect(cellText(rows[0]!.data, true)).toContain('\n')
  })
  it('rejects invalid numeric inputs and treats mixed types as text', () => {
    for (const value of ['', 'Infinity', 'abc']) expect(filterError({column: 'amount', operator: 'greater', value}, 'number')).toBeTruthy()
    expect(columnKind([{ value: 1 }, { value: '1' }], 'value')).toBe('text')
    expect(filterRows([], '', [])).toEqual([])
  })
})

it('sorts numbers, false, null, missing values and JSON deterministically', () => {
  expect(compareCells(2, 10)).toBeLessThan(0)
  expect(compareCells(false, true)).toBeLessThan(0)
  expect(compareCells(undefined, null)).toBeLessThan(0)
  expect(compareCells(null, 0)).toBeLessThan(0)
  expect(compareCells({ a: 1 }, { a: 2 })).toBeLessThan(0)
  expect(compareCells({ a: 2 }, { a: 1 })).toBeGreaterThan(0)
  expect(compareCells({ a: 1 }, { a: 1 })).toBe(0)
})

it('maintains transitive ordering across mixed result types', () => {
  const values = [undefined, null, false, true, 2, 10, { a: 1 }, { a: 2 }, '11', '15']
  for (const left of values) for (const middle of values) for (const right of values) {
    if (compareCells(left, middle) < 0 && compareCells(middle, right) < 0) expect(compareCells(left, right)).toBeLessThan(0)
  }
})
