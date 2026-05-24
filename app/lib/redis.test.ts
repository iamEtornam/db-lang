import { describe, expect, it } from 'vitest'
import {
  defaultValueFor,
  getRedisKey,
  getRedisType,
  isRedisEngine,
  REDIS_TYPES,
  prettyJson,
  validateForType,
} from './redis'

describe('isRedisEngine', () => {
  it('accepts redis only', () => {
    expect(isRedisEngine('redis')).toBe(true)
    expect(isRedisEngine('mongodb')).toBe(false)
    expect(isRedisEngine(undefined)).toBe(false)
  })
})

describe('REDIS_TYPES', () => {
  it('lists the six supported types', () => {
    const values = REDIS_TYPES.map(t => t.value)
    expect(values).toEqual(['string', 'hash', 'list', 'set', 'zset', 'stream'])
  })
})

describe('getRedisKey / getRedisType', () => {
  it('extracts string key', () => {
    expect(getRedisKey({ key: 'users:42', type: 'hash' })).toBe('users:42')
    expect(getRedisKey({ key: '' })).toBe(null)
    expect(getRedisKey({ type: 'hash' })).toBe(null)
  })
  it('validates type', () => {
    expect(getRedisType({ type: 'hash' })).toBe('hash')
    expect(getRedisType({ type: 'stream' })).toBe('stream')
    expect(getRedisType({ type: 'unknown' })).toBe(null)
    expect(getRedisType({})).toBe(null)
  })
})

describe('defaultValueFor', () => {
  it('produces parseable JSON for every type', () => {
    for (const { value } of REDIS_TYPES) {
      const seed = defaultValueFor(value)
      expect(() => JSON.parse(seed)).not.toThrow()
    }
  })
  it('hash seed is an object', () => {
    expect(JSON.parse(defaultValueFor('hash'))).toEqual({ field: 'value' })
  })
  it('list/set seeds are arrays', () => {
    expect(Array.isArray(JSON.parse(defaultValueFor('list')))).toBe(true)
    expect(Array.isArray(JSON.parse(defaultValueFor('set')))).toBe(true)
  })
  it('zset seed has numeric scores', () => {
    const parsed = JSON.parse(defaultValueFor('zset'))
    for (const score of Object.values(parsed)) {
      expect(typeof score).toBe('number')
    }
  })
})

describe('validateForType', () => {
  it('string accepts scalars but rejects objects/arrays', () => {
    expect(validateForType('"hello"', 'string').ok).toBe(true)
    expect(validateForType('42', 'string').ok).toBe(true)
    expect(validateForType('true', 'string').ok).toBe(true)
    expect(validateForType('null', 'string').ok).toBe(true)
    expect(validateForType('{}', 'string').ok).toBe(false)
    expect(validateForType('[]', 'string').ok).toBe(false)
  })
  it('hash requires an object', () => {
    expect(validateForType('{"a":"1"}', 'hash').ok).toBe(true)
    expect(validateForType('[]', 'hash').ok).toBe(false)
    expect(validateForType('"a"', 'hash').ok).toBe(false)
  })
  it('list requires an array', () => {
    expect(validateForType('["a","b"]', 'list').ok).toBe(true)
    expect(validateForType('{}', 'list').ok).toBe(false)
  })
  it('set requires an array', () => {
    expect(validateForType('["a"]', 'set').ok).toBe(true)
    expect(validateForType('{}', 'set').ok).toBe(false)
  })
  it('zset requires object with numeric scores', () => {
    expect(validateForType('{"m":1.5}', 'zset').ok).toBe(true)
    expect(validateForType('{"m":"not a number"}', 'zset').ok).toBe(false)
    expect(validateForType('[]', 'zset').ok).toBe(false)
  })
  it('stream requires object with >=1 field', () => {
    expect(validateForType('{"k":"v"}', 'stream').ok).toBe(true)
    expect(validateForType('{}', 'stream').ok).toBe(false)
    expect(validateForType('[]', 'stream').ok).toBe(false)
  })
  it('empty string rejected for all types', () => {
    for (const { value } of REDIS_TYPES) {
      expect(validateForType('', value).ok).toBe(false)
    }
  })
  it('malformed JSON rejected', () => {
    expect(validateForType('{a:1}', 'hash').ok).toBe(false)
  })
})

describe('prettyJson', () => {
  it('handles any JSON value', () => {
    expect(prettyJson({ a: 1 })).toBe('{\n  "a": 1\n}')
    expect(prettyJson([1, 2])).toBe('[\n  1,\n  2\n]')
  })
})
