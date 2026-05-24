/**
 * SQL builders used by the schema-page CRUD flow. Output is rendered into a
 * preview dialog so the user can review (and hand-edit) the statement before
 * it executes — so these helpers favour producing readable SQL over being
 * defensively over-quoted.
 *
 * Engines covered: postgres, mysql, mariadb, sqlite. Anything else is
 * rejected upstream by `isSqlEngine`.
 */

export type SqlEngine = 'postgres' | 'mysql' | 'mariadb' | 'sqlite'

export function isSqlEngine(engine: string): engine is SqlEngine {
  return engine === 'postgres' || engine === 'mysql' || engine === 'mariadb' || engine === 'sqlite'
}

/** Quote a table or column identifier. Schema-qualified when provided. */
export function quoteIdentifier(engine: string, name: string, schema?: string | null): string {
  if (engine === 'mysql' || engine === 'mariadb') {
    const q = (s: string) => '`' + s.replace(/`/g, '``') + '`'
    return schema ? `${q(schema)}.${q(name)}` : q(name)
  }
  // postgres + sqlite both use double quotes; doubling embedded quotes escapes them.
  const q = (s: string) => '"' + s.replace(/"/g, '""') + '"'
  return schema ? `${q(schema)}.${q(name)}` : q(name)
}

/**
 * Render a JS value as a SQL literal. The user can edit the resulting SQL in
 * the confirm dialog, so this only needs to cover the common scalar cases
 * cleanly; complex/binary types should be marked read-only in the row editor.
 *
 * When `dataType` is provided and indicates a JSON/JSONB column, the value
 * is emitted with the appropriate engine-specific cast so the DB stores it
 * as JSON rather than a plain text literal.
 */
export function quoteValue(engine: string, value: unknown, dataType?: string): string {
  if (value === null || value === undefined) return 'NULL'

  // JSON / JSONB columns need an engine-specific cast or wrapper. We accept
  // any JS value and serialise it to a JSON literal — strings already
  // containing JSON pass through `JSON.stringify`'s quoting, objects become
  // nested JSON.
  if (dataType && /\b(jsonb|json)\b/i.test(dataType)) {
    const jsonText = typeof value === 'string' ? value : JSON.stringify(value)
    const quoted = "'" + jsonText.replace(/'/g, "''") + "'"
    if (engine === 'postgres') return `${quoted}::jsonb`
    if (engine === 'mysql' || engine === 'mariadb') return `CAST(${quoted} AS JSON)`
    return quoted  // sqlite stores JSON as TEXT — no cast needed.
  }

  if (typeof value === 'boolean') {
    if (engine === 'sqlite') return value ? '1' : '0'
    return value ? 'TRUE' : 'FALSE'
  }
  if (typeof value === 'number') {
    return Number.isFinite(value) ? String(value) : 'NULL'
  }
  if (typeof value === 'bigint') return value.toString()
  // Strings (and anything else): single-quoted, with embedded quotes doubled.
  const s = typeof value === 'string' ? value : String(value)
  return "'" + s.replace(/'/g, "''") + "'"
}

/**
 * A user-entered cell value comes back from the row editor as a string (form
 * inputs are always strings). Map common entries back to typed values so the
 * generated SQL doesn't quote numbers or booleans as strings.
 *
 *   ''         -> null       (we expose a separate NULL toggle in the UI)
 *   'NULL'     -> null       (case-insensitive, convenience for power users)
 *   'true'/'false' (case-insensitive, when column type looks boolean) -> bool
 *   numeric    -> number     (when column type looks numeric)
 */
export function coerceCellInput(
  raw: string,
  isExplicitNull: boolean,
  dataType: string | undefined,
): unknown {
  if (isExplicitNull) return null
  if (raw === '') return null
  const upper = raw.toUpperCase()
  if (upper === 'NULL') return null

  const t = (dataType ?? '').toLowerCase()
  const isBoolType = /bool/.test(t)
  if (isBoolType) {
    if (upper === 'TRUE' || raw === '1') return true
    if (upper === 'FALSE' || raw === '0') return false
  }

  const isNumericType = /int|numeric|decimal|real|double|float|serial/.test(t)
  if (isNumericType) {
    // Reject things that look like numbers but aren't (e.g. "12abc") — fall
    // through to the string branch so the user sees their input verbatim and
    // the DB rejects it with a real error rather than us silently mangling.
    if (/^-?\d+(\.\d+)?$/.test(raw.trim())) return Number(raw)
  }

  return raw
}

export interface ValueBinding {
  column: string
  /** Pre-coerced value (null / number / boolean / string). For JSON columns
   *  the value is the raw JSON text the user typed; the builder serialises
   *  it with the right cast based on `dataType`. */
  value: unknown
  /** Column data type from the schema. Used by `quoteValue` to pick the
   *  right SQL representation for non-scalar columns like JSON/JSONB. */
  dataType?: string
}

export function buildInsert(
  engine: string,
  table: string,
  schema: string | null | undefined,
  bindings: ValueBinding[],
): string {
  if (bindings.length === 0) {
    throw new Error('INSERT requires at least one column')
  }
  const cols = bindings.map(b => quoteIdentifier(engine, b.column)).join(', ')
  const vals = bindings.map(b => quoteValue(engine, b.value, b.dataType)).join(', ')
  return `INSERT INTO ${quoteIdentifier(engine, table, schema ?? undefined)} (${cols})\nVALUES (${vals});`
}

/** A single PK column + its value on a specific row. */
export interface PkBinding {
  column: string
  value: unknown
}

/** Render `col1 = v1 AND col2 = v2` from a list of PK bindings. Each row
 *  is uniquely identified by the tuple of all its PK columns; this helper
 *  is the shared chokepoint for UPDATE / single-row DELETE / row identity. */
function pkEqualityClause(engine: string, pks: PkBinding[]): string {
  if (pks.length === 0) {
    throw new Error('At least one PK column is required to target a row')
  }
  return pks
    .map(pk => `${quoteIdentifier(engine, pk.column)} = ${quoteValue(engine, pk.value)}`)
    .join(' AND ')
}

export function buildUpdate(
  engine: string,
  table: string,
  schema: string | null | undefined,
  bindings: ValueBinding[],
  pks: PkBinding[],
): string {
  if (bindings.length === 0) {
    throw new Error('UPDATE requires at least one column to change')
  }
  const setClause = bindings
    .map(b => `${quoteIdentifier(engine, b.column)} = ${quoteValue(engine, b.value, b.dataType)}`)
    .join(',\n    ')
  return `UPDATE ${quoteIdentifier(engine, table, schema ?? undefined)}\nSET ${setClause}\nWHERE ${pkEqualityClause(engine, pks)};`
}

export function buildDelete(
  engine: string,
  table: string,
  schema: string | null | undefined,
  pks: PkBinding[],
): string {
  return `DELETE FROM ${quoteIdentifier(engine, table, schema ?? undefined)}\nWHERE ${pkEqualityClause(engine, pks)};`
}

/**
 * Bulk DELETE for N rows. Uses `WHERE col IN (...)` for single-column PKs and
 * `WHERE (col1, col2) IN ((v1,v2), (v3,v4))` (row-value IN) for composite PKs.
 * Postgres, MySQL, MariaDB, and SQLite all support row-value IN — this lets
 * the dialog show one clean statement rather than N chained DELETEs.
 *
 * `rows` is an array of PK tuples; every tuple must have the same column
 * order. An empty list throws — the caller should guard against this.
 */
// ============ DDL builders (Phase 6) ============

/** Column definition for CREATE TABLE / ADD COLUMN. */
export interface ColumnDef {
  name: string
  /** Engine-specific data type as the user typed it, e.g. `varchar(255)`,
   *  `bigint`, `JSON`, `TEXT`. We don't validate — the DB rejects bad
   *  types with a real error which is more informative than guessing. */
  dataType: string
  nullable: boolean
  /** Default expression, e.g. `0`, `'pending'`, `now()`, `gen_random_uuid()`.
   *  Emitted verbatim into the DDL — caller is responsible for quoting
   *  string literals. */
  default?: string
  isPrimaryKey: boolean
}

/** Engine-specific quoting + serialisation of one ColumnDef into a DDL
 *  column clause. Used by CREATE TABLE and ADD COLUMN. */
function columnDdl(engine: string, col: ColumnDef): string {
  const parts: string[] = [quoteIdentifier(engine, col.name), col.dataType]
  if (col.isPrimaryKey) parts.push('PRIMARY KEY')
  if (!col.nullable) parts.push('NOT NULL')
  if (col.default && col.default.trim() !== '') {
    parts.push(`DEFAULT ${col.default}`)
  }
  return parts.join(' ')
}

export function buildCreateTable(
  engine: string,
  table: string,
  schema: string | null | undefined,
  columns: ColumnDef[],
): string {
  if (columns.length === 0) {
    throw new Error('CREATE TABLE requires at least one column')
  }
  if (columns.filter(c => c.isPrimaryKey).length > 1) {
    throw new Error(
      'Multiple PRIMARY KEY columns — composite PKs are a separate DDL slice; '
      + 'list one column as PRIMARY KEY or hand-edit the SQL after preview.',
    )
  }
  const cols = columns.map(c => '  ' + columnDdl(engine, c)).join(',\n')
  const tbl = quoteIdentifier(engine, table, schema ?? undefined)
  // MySQL gets InnoDB suffix by default — matches the convention most modern
  // schemas use. Postgres + SQLite don't take a trailing storage clause.
  const suffix = (engine === 'mysql' || engine === 'mariadb') ? ' ENGINE=InnoDB' : ''
  return `CREATE TABLE ${tbl} (\n${cols}\n)${suffix};`
}

export function buildDropTable(
  engine: string,
  table: string,
  schema: string | null | undefined,
): string {
  return `DROP TABLE ${quoteIdentifier(engine, table, schema ?? undefined)};`
}

export function buildAddColumn(
  engine: string,
  table: string,
  schema: string | null | undefined,
  column: ColumnDef,
): string {
  const tbl = quoteIdentifier(engine, table, schema ?? undefined)
  return `ALTER TABLE ${tbl}\n  ADD COLUMN ${columnDdl(engine, column)};`
}

export function buildDropColumn(
  engine: string,
  table: string,
  schema: string | null | undefined,
  columnName: string,
): string {
  const tbl = quoteIdentifier(engine, table, schema ?? undefined)
  return `ALTER TABLE ${tbl}\n  DROP COLUMN ${quoteIdentifier(engine, columnName)};`
}

export function buildRenameColumn(
  engine: string,
  table: string,
  schema: string | null | undefined,
  oldName: string,
  newName: string,
): string {
  const tbl = quoteIdentifier(engine, table, schema ?? undefined)
  // pg/mysql8/sqlite3.25+ all support this form. Older MySQL needs CHANGE
  // COLUMN — we don't target it; users on legacy MySQL can hand-edit the
  // DDL in the confirm preview.
  return `ALTER TABLE ${tbl}\n  RENAME COLUMN ${quoteIdentifier(engine, oldName)} TO ${quoteIdentifier(engine, newName)};`
}

/** SQLite has no in-place column-type change. The dialog refuses to call
 *  this for sqlite and shows an explainer instead. */
export function buildAlterColumnType(
  engine: string,
  table: string,
  schema: string | null | undefined,
  columnName: string,
  newType: string,
  usingExpr?: string,
): string {
  const tbl = quoteIdentifier(engine, table, schema ?? undefined)
  const col = quoteIdentifier(engine, columnName)
  if (engine === 'postgres') {
    let stmt = `ALTER TABLE ${tbl}\n  ALTER COLUMN ${col} TYPE ${newType}`
    if (usingExpr && usingExpr.trim() !== '') stmt += `\n  USING ${usingExpr}`
    return stmt + ';'
  }
  if (engine === 'mysql' || engine === 'mariadb') {
    return `ALTER TABLE ${tbl}\n  MODIFY COLUMN ${col} ${newType};`
  }
  // SQLite — included for completeness but the dialog should block reaching
  // this branch. Emits a no-op comment + the intent so the user can
  // hand-edit if they really want to attempt a table rebuild.
  return `-- SQLite does not support in-place column type changes.\n-- Intended: ALTER COLUMN ${col} TYPE ${newType} on ${tbl}`
}

export function buildBulkDelete(
  engine: string,
  table: string,
  schema: string | null | undefined,
  pkColumns: string[],
  rows: unknown[][],
): string {
  if (pkColumns.length === 0) {
    throw new Error('Bulk DELETE requires at least one PK column')
  }
  if (rows.length === 0) {
    throw new Error('Bulk DELETE requires at least one row')
  }

  const quotedTable = quoteIdentifier(engine, table, schema ?? undefined)

  if (pkColumns.length === 1) {
    const col = quoteIdentifier(engine, pkColumns[0]!)
    const vals = rows.map(r => quoteValue(engine, r[0])).join(',\n  ')
    return `DELETE FROM ${quotedTable}\nWHERE ${col} IN (\n  ${vals}\n);`
  }

  // Composite PK: row-value IN
  const colTuple = '(' + pkColumns.map(c => quoteIdentifier(engine, c)).join(', ') + ')'
  const rowTuples = rows
    .map((r) => {
      if (r.length !== pkColumns.length) {
        throw new Error(`Composite-PK row has ${r.length} values but ${pkColumns.length} PK columns`)
      }
      return '(' + r.map(v => quoteValue(engine, v)).join(', ') + ')'
    })
    .join(',\n  ')
  return `DELETE FROM ${quotedTable}\nWHERE ${colTuple} IN (\n  ${rowTuples}\n);`
}
