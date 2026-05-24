/**
 * Frontend helpers for Firestore CRUD on the schema page.
 *
 * Read path injects three metadata fields onto each row — `_id`,
 * `_createTime`, `_updateTime`. We strip them on the way back into the
 * editor so the user doesn't see them, and the backend strips them again
 * defensively before hitting the wire.
 */

export function isFirestoreEngine(engine: string | undefined | null): boolean {
  return engine === 'firestore'
}

const METADATA_KEYS = new Set(['_id', '_createTime', '_updateTime'])

/** Pull `_id` off a row. Firestore always assigns one, so this should never
 *  be null in practice — defensive null returns let the UI degrade gracefully
 *  if the read shape ever changes. */
export function getDocId(row: Record<string, unknown>): string | null {
  const v = row._id
  if (typeof v !== 'string' || v === '') return null
  return v
}

/** Strip read-path metadata (`_id`, `_createTime`, `_updateTime`) from a row
 *  before seeding the document editor. The user manages document fields, not
 *  Firestore-managed timestamps. */
export function stripMetadata(row: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {}
  for (const [k, v] of Object.entries(row)) {
    if (!METADATA_KEYS.has(k)) out[k] = v
  }
  return out
}

export function prettyJson(value: unknown): string {
  return JSON.stringify(value ?? {}, null, 2)
}

export interface JsonValidation {
  ok: boolean
  error?: string
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
    return { ok: false, error: 'Top level must be a JSON object' }
  }
  return { ok: true }
}
