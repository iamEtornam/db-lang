/**
 * Frontend helpers for MongoDB CRUD on the schema page. The backend driver
 * does the ObjectId hex-string promotion, so on this side we only need to
 * locate `_id` on a row, build the filter JSON the user will see in the
 * confirm dialog, and format documents for editing.
 */

export function isMongoEngine(engine: string | undefined | null): boolean {
  return engine === 'mongodb'
}

/** Extract the document identifier from a preview row. Returns null if the
 *  row has no `_id` field (collections without explicit `_id` shouldn't
 *  exist in MongoDB, but be defensive). */
export function getDocId(row: Record<string, unknown>): unknown {
  if (!('_id' in row)) return null
  const v = row._id
  return v === undefined ? null : v
}

/** Build the JSON string for a `{ "_id": <id> }` filter. Numbers and
 *  booleans round-trip as JSON; everything else (including the hex
 *  ObjectId strings the read path produces) is JSON-stringified, which
 *  always yields a quoted string — the backend then promotes valid hex
 *  to an ObjectId. */
export function buildIdFilter(id: unknown): string {
  return JSON.stringify({ _id: id }, null, 2)
}

/** Pretty-print a JS value as a 2-space-indented JSON document. Used to
 *  seed the document editor with the row being edited. */
export function prettyJson(value: unknown): string {
  return JSON.stringify(value ?? {}, null, 2)
}

export interface JsonValidation {
  ok: boolean
  /** Human-readable parse error, if any. */
  error?: string
  /** Top-level shape check — Mongo always operates on objects. */
  isObject?: boolean
}

export function validateJsonObject(raw: string): JsonValidation {
  const trimmed = raw.trim()
  if (trimmed === '') return { ok: false, error: 'Document is empty' }
  let parsed: unknown
  try {
    parsed = JSON.parse(trimmed)
  }
  catch (e) {
    return { ok: false, error: (e as Error).message }
  }
  if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
    return { ok: false, isObject: false, error: 'Top level must be a JSON object' }
  }
  return { ok: true, isObject: true }
}
