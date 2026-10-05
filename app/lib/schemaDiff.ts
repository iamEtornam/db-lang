import type { ColumnInfo, TableInfo } from '~/types/database'
export interface ComparisonTable { table: TableInfo; columns: ColumnInfo[]; special_columns: string[] }
export interface ComparisonSchema { engine: string; tables: ComparisonTable[] }
export interface SchemaChange { table: string; column: string | null; kind: 'add' | 'remove' | 'change'; source: string; target: string; sql: string | null }
const definition = (column: ColumnInfo) => `${column.data_type}${column.is_nullable ? ' NULL' : ' NOT NULL'}${column.column_default === null ? '' : ` DEFAULT ${column.column_default}`}${column.is_primary_key ? ' PRIMARY KEY' : ''}${column.is_foreign_key ? ' FOREIGN KEY' : ''}`
const signature = (column: ColumnInfo) => JSON.stringify([column.data_type, column.is_nullable, column.column_default, column.is_primary_key, column.is_foreign_key])
const quote = (engine: string, value: string) => engine === 'mysql' || engine === 'mariadb' ? `\`${value.replaceAll('`', '``')}\`` : `"${value.replaceAll('"', '""')}"`
// Only ordinary nullable columns produce executable drafts. Other differences need a native migration review.
function additiveSql(engine: string, table: TableInfo, column: ColumnInfo, special: boolean) {
  if (special || !column.is_nullable || column.column_default !== null || column.is_primary_key || column.is_foreign_key) return null
  if (!/^(?:smallint|integer|int|bigint|tinyint|mediumint|real|double precision|double|float|boolean|bool|text|blob|bytea|date|datetime|timestamp|time|uuid|jsonb?|numeric|decimal|varchar|character varying|character|char)(?:\(\d+(?:,\s*\d+)?\))?(?: unsigned)?$/i.test(column.data_type)) return null
  const name = [table.schema, table.name].filter((part): part is string => part !== null).map(part => quote(engine, part)).join('.')
  return `ALTER TABLE ${name} ADD COLUMN ${quote(engine, column.name)} ${column.data_type};`
}
export function compareSchemas(source: ComparisonSchema, target: ComparisonSchema, sourceSchema: string | null, targetSchema: string | null): SchemaChange[] {
  const family = (engine: string) => engine === 'mariadb' ? 'mysql' : engine
  if (family(source.engine) !== family(target.engine)) throw new Error('Choose connections using the same SQL engine family')
  const from = source.tables.filter(row => row.table.schema === sourceSchema)
  const to = target.tables.filter(row => row.table.schema === targetSchema)
  const sourceTables = new Map(from.map(row => [row.table.name, row]))
  const targetTables = new Map(to.map(row => [row.table.name, row]))
  const changes: SchemaChange[] = []
  for (const row of from) {
    const other = targetTables.get(row.table.name)
    if (!other) { changes.push({ table: row.table.name, column: null, kind: 'add', source: 'Table exists', target: 'Missing table', sql: null }); continue }
    const current = new Map(other.columns.map(column => [column.name, column]))
    const desired = new Map(row.columns.map(column => [column.name, column]))
    for (const column of row.columns) {
      const existing = current.get(column.name)
      if (!existing && other.special_columns.includes(column.name)) changes.push({ table: row.table.name, column: column.name, kind: 'change', source: definition(column), target: 'Generated or hidden column', sql: null })
      else if (!existing) changes.push({ table: row.table.name, column: column.name, kind: 'add', source: definition(column), target: 'Missing column', sql: additiveSql(target.engine, other.table, column, row.special_columns.includes(column.name)) })
      else if (signature(column) !== signature(existing) || row.special_columns.includes(column.name) !== other.special_columns.includes(column.name)) changes.push({ table: row.table.name, column: column.name, kind: 'change', source: definition(column), target: definition(existing), sql: null })
    }
    for (const column of other.columns) if (!desired.has(column.name)) changes.push({ table: row.table.name, column: column.name, kind: row.special_columns.includes(column.name) ? 'change' : 'remove', source: row.special_columns.includes(column.name) ? 'Generated or hidden column' : 'Missing column', target: definition(column), sql: null })
    // Generated SQLite columns can be absent from PRAGMA table_info, so keep their existence visible.
    for (const column of row.special_columns) if (!desired.has(column) && !current.has(column) && !other.special_columns.includes(column)) changes.push({ table: row.table.name, column, kind: 'add', source: 'Generated or hidden column', target: 'Missing column', sql: null })
    for (const column of other.special_columns) if (!current.has(column) && !desired.has(column) && !row.special_columns.includes(column)) changes.push({ table: row.table.name, column, kind: 'remove', source: 'Missing column', target: 'Generated or hidden column', sql: null })
  }
  for (const row of to) if (!sourceTables.has(row.table.name)) changes.push({ table: row.table.name, column: null, kind: 'remove', source: 'Missing table', target: 'Table exists', sql: null })
  return changes
}
export function migrationDraft(changes: SchemaChange[]) {
  return ['-- Review-only column migration draft. Not executed by QueryStudio.', '-- Indexes, full constraints, generated expressions, triggers and other objects require separate review.', ...changes.flatMap(change => change.sql ? [change.sql] : [`-- Manual review: ${change.kind} ${JSON.stringify([change.table, change.column])?.replaceAll('\n', ' ')}`])].join('\n')
}
