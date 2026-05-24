import { describe, expect, it } from 'vitest'
import {
  getRowKey,
  isRtdbEngine,
  prettyJson,
  rowToChildJson,
  validateJsonAny,
} from './rtdb'

describe('isRtdbEngine', () => {
  it('accepts firebase_rtdb only', () => {
    expect(isRtdbEngine('firebase_rtdb')).toBe(true)
    expect(isRtdbEngine('firestore')).toBe(false)
    expect(isRtdbEngine(undefined)).toBe(false)
  })
})

describe('getRowKey', () => {
  it('prefers _key (string)', () => {
    expect(getRowKey({ _key: 'profile-1', name: 'X' })).toBe('profile-1')
  })
  it('coerces numeric _key to string', () => {
    expect(getRowKey({ _key: 42 } as Record<string, unknown>)).toBe('42')
  })
  it('falls through to _index when _key absent (Phase 8 arrays)', () => {
    expect(getRowKey({ _index: 3, _value: 10 })).toBe('3')
  })
  it('rejects empty _key and missing identity', () => {
    expect(getRowKey({ _key: '' })).toBe(null)
    expect(getRowKey({ name: 'X' })).toBe(null)
  })
})

describe('rowToChildJson', () => {
  it('extracts the inner _value for primitive children', () => {
    expect(rowToChildJson({ _key: 'a', _value: 'hello' })).toBe('hello')
    expect(rowToChildJson({ _index: 0, _value: 42 })).toBe(42)
    expect(rowToChildJson({ _key: 'a', _value: true })).toBe(true)
    expect(rowToChildJson({ _key: 'a', _value: null })).toBe(null)
  })
  it('strips metadata from object children', () => {
    expect(rowToChildJson({ _key: 'a', x: 1, y: 2 })).toEqual({ x: 1, y: 2 })
    expect(rowToChildJson({ _index: 0, x: 1 })).toEqual({ x: 1 })
  })
})

describe('validateJsonAny', () => {
  it('accepts any valid JSON value', () => {
    expect(validateJsonAny('"hello"').ok).toBe(true)
    expect(validateJsonAny('42').ok).toBe(true)
    expect(validateJsonAny('true').ok).toBe(true)
    expect(validateJsonAny('null').ok).toBe(true)
    expect(validateJsonAny('[1,2,3]').ok).toBe(true)
    expect(validateJsonAny('{"a":1}').ok).toBe(true)
  })
  it('rejects empty and malformed', () => {
    expect(validateJsonAny('').ok).toBe(false)
    expect(validateJsonAny('{a:1}').ok).toBe(false)
  })
})

describe('prettyJson', () => {
  it('handles primitives + nulls', () => {
    expect(prettyJson('hello')).toBe('"hello"')
    expect(prettyJson(42)).toBe('42')
    expect(prettyJson(null)).toBe('null')
  })
})
