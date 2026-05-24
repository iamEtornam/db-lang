/**
 * Frontend helpers for Firebase Realtime Database CRUD.
 *
 * The read path flattens each child node into a synthetic "row" with
 * metadata keys (`_key` for object children, `_index` for array children,
 * `_value` when the child is a primitive). The writers need the REAL
 * child value, so `rowToChildJson` reverses the flatten before we hand
 * the JSON to the editor.
 *
 * RTDB accepts any JSON shape at any path — string, number, bool, object,
 * array, null. Validation is intentionally looser than Mongo/Firestore;
 * we only require `JSON.parse` success.
 */

export function isRtdbEngine(engine: string | undefined | null): boolean {
  return engine === 'firebase_rtdb'
}

/** Pull the child's identity off a flattened row. Objects come back with
 *  `_key` (string); arrays come back with `_index` (number). Either is a
 *  valid path segment for the REST API — `users/abc/scores/3.json` and
 *  `users/abc/profiles/admin.json` both work. */
export function getRowKey(row: Record<string, unknown>): string | null {
  const k = row._key
  if (typeof k === 'string' && k !== '') return k
  if (typeof k === 'number') return String(k)
  const i = row._index
  if (typeof i === 'number') return String(i)
  if (typeof i === 'string' && i !== '') return i
  return null
}

/** Reconstruct the actual child JSON from a flattened row. If the row has
 *  a `_value` field the child was a primitive; otherwise the child was an
 *  object and the non-metadata fields ARE the child. */
export function rowToChildJson(row: Record<string, unknown>): unknown {
  if ('_value' in row) return row._value
  const { _key, _index, ...rest } = row
  return rest
}

export function prettyJson(value: unknown): string {
  return JSON.stringify(value ?? null, null, 2)
}

export interface JsonValidation {
  ok: boolean
  error?: string
}

/** RTDB accepts any JSON value — string, number, bool, object, array, null.
 *  Just verify parse success. */
export function validateJsonAny(raw: string): JsonValidation {
  const trimmed = raw.trim()
  if (trimmed === '') return { ok: false, error: 'Value is empty' }
  try {
    JSON.parse(trimmed)
  }
  catch (e) {
    return { ok: false, error: (e as Error).message }
  }
  return { ok: true }
}
