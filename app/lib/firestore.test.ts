import { describe, expect, it } from 'vitest'
import {
  getDocId,
  isFirestoreEngine,
  prettyJson,
  stripMetadata,
  validateJsonObject,
} from './firestore'

describe('isFirestoreEngine', () => {
  it('accepts firestore only', () => {
    expect(isFirestoreEngine('firestore')).toBe(true)
    expect(isFirestoreEngine('firebase_rtdb')).toBe(false)
    expect(isFirestoreEngine(undefined)).toBe(false)
  })
})

describe('getDocId', () => {
  it('returns the _id string', () => {
    expect(getDocId({ _id: 'abc', name: 'X' })).toBe('abc')
  })
  it('rejects non-string / empty _id', () => {
    expect(getDocId({ _id: 42 })).toBe(null)
    expect(getDocId({ _id: '' })).toBe(null)
    expect(getDocId({ name: 'X' })).toBe(null)
  })
})

describe('stripMetadata', () => {
  it('removes the three read-path metadata keys', () => {
    const row = {
      _id: 'abc',
      _createTime: '2024-01-01',
      _updateTime: '2024-01-02',
      name: 'X',
      tags: ['a'],
    }
    expect(stripMetadata(row)).toEqual({ name: 'X', tags: ['a'] })
  })
  it('passes plain rows through unchanged shape', () => {
    expect(stripMetadata({ a: 1 })).toEqual({ a: 1 })
  })
})

describe('validateJsonObject', () => {
  it('accepts an object', () => {
    expect(validateJsonObject('{"a":1}').ok).toBe(true)
  })
  it('rejects arrays + scalars + invalid + empty', () => {
    expect(validateJsonObject('[1]').ok).toBe(false)
    expect(validateJsonObject('42').ok).toBe(false)
    expect(validateJsonObject('').ok).toBe(false)
    expect(validateJsonObject('{a:1}').ok).toBe(false)
  })
})

describe('prettyJson', () => {
  it('renders with 2-space indent', () => {
    expect(prettyJson({ a: 1 })).toBe('{\n  "a": 1\n}')
  })
})
