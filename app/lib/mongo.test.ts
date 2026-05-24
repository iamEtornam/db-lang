import { describe, expect, it } from 'vitest'
import { buildIdFilter, getDocId, isMongoEngine, prettyJson, validateJsonObject } from './mongo'

describe('isMongoEngine', () => {
  it('accepts mongodb only', () => {
    expect(isMongoEngine('mongodb')).toBe(true)
    expect(isMongoEngine('mongo')).toBe(false)
    expect(isMongoEngine('postgres')).toBe(false)
    expect(isMongoEngine(undefined)).toBe(false)
    expect(isMongoEngine(null)).toBe(false)
  })
})

describe('getDocId', () => {
  it('returns string _id', () => {
    expect(getDocId({ _id: '507f1f77bcf86cd799439011', name: 'x' })).toBe('507f1f77bcf86cd799439011')
  })
  it('returns numeric _id (some apps use ints)', () => {
    expect(getDocId({ _id: 42, name: 'x' })).toBe(42)
  })
  it('null _id returns null', () => {
    expect(getDocId({ _id: null })).toBe(null)
  })
  it('absent _id returns null', () => {
    expect(getDocId({ name: 'x' })).toBe(null)
  })
})

describe('buildIdFilter', () => {
  it('wraps a hex string id', () => {
    const filter = buildIdFilter('507f1f77bcf86cd799439011')
    expect(JSON.parse(filter)).toEqual({ _id: '507f1f77bcf86cd799439011' })
  })
  it('wraps a numeric id', () => {
    const filter = buildIdFilter(42)
    expect(JSON.parse(filter)).toEqual({ _id: 42 })
  })
  it('is pretty-printed for the dialog', () => {
    const filter = buildIdFilter('abc')
    expect(filter).toContain('\n')
  })
})

describe('validateJsonObject', () => {
  it('accepts an object', () => {
    expect(validateJsonObject('{"a":1}').ok).toBe(true)
  })
  it('rejects empty', () => {
    expect(validateJsonObject('').ok).toBe(false)
  })
  it('rejects invalid json', () => {
    expect(validateJsonObject('{a:1}').ok).toBe(false)
  })
  it('rejects arrays', () => {
    const r = validateJsonObject('[1,2]')
    expect(r.ok).toBe(false)
    expect(r.error).toMatch(/JSON object/)
  })
  it('rejects scalars at the top', () => {
    expect(validateJsonObject('42').ok).toBe(false)
    expect(validateJsonObject('"hello"').ok).toBe(false)
    expect(validateJsonObject('null').ok).toBe(false)
  })
})

describe('prettyJson', () => {
  it('pretty-prints with 2-space indent', () => {
    expect(prettyJson({ a: 1, b: 2 })).toBe('{\n  "a": 1,\n  "b": 2\n}')
  })
  it('null/undefined fall back to {}', () => {
    expect(prettyJson(null)).toBe('{}')
    expect(prettyJson(undefined)).toBe('{}')
  })
})
