import { describe, expect, it } from 'vitest'
import {
  isSqlEngine,
  quoteIdentifier,
  quoteValue,
  coerceCellInput,
  buildInsert,
  buildUpdate,
  buildDelete,
  buildBulkDelete,
  buildCreateTable,
  buildDropTable,
  buildAddColumn,
  buildDropColumn,
  buildRenameColumn,
  buildAlterColumnType,
  type ColumnDef,
} from './sql'

describe('isSqlEngine', () => {
  it.each(['postgres', 'mysql', 'mariadb', 'sqlite'])('accepts %s', (e) => {
    expect(isSqlEngine(e)).toBe(true)
  })
  it.each(['mongodb', 'redis', 'firestore', 'firebase_rtdb', '', 'pg'])(
    'rejects %s',
    (e) => {
      expect(isSqlEngine(e)).toBe(false)
    },
  )
})

describe('quoteIdentifier', () => {
  it('postgres uses double quotes', () => {
    expect(quoteIdentifier('postgres', 'users')).toBe('"users"')
  })
  it('sqlite uses double quotes', () => {
    expect(quoteIdentifier('sqlite', 'users')).toBe('"users"')
  })
  it('mysql uses backticks', () => {
    expect(quoteIdentifier('mysql', 'users')).toBe('`users`')
  })
  it('mariadb uses backticks', () => {
    expect(quoteIdentifier('mariadb', 'users')).toBe('`users`')
  })
  it('escapes embedded double quotes (pg)', () => {
    // A column literally named `weird"name` should round-trip as
    // `"weird""name"` — doubled-quote is SQL's standard escape.
    expect(quoteIdentifier('postgres', 'weird"name')).toBe('"weird""name"')
  })
  it('escapes embedded backticks (mysql)', () => {
    expect(quoteIdentifier('mysql', 'weird`name')).toBe('`weird``name`')
  })
  it('qualifies with schema when provided (pg)', () => {
    expect(quoteIdentifier('postgres', 'users', 'public')).toBe('"public"."users"')
  })
  it('qualifies with schema when provided (mysql)', () => {
    expect(quoteIdentifier('mysql', 'users', 'app')).toBe('`app`.`users`')
  })
  it('escapes quotes inside schema too', () => {
    expect(quoteIdentifier('postgres', 'users', 'we"ird')).toBe('"we""ird"."users"')
  })
})

describe('quoteValue', () => {
  it('null/undefined -> NULL', () => {
    expect(quoteValue('postgres', null)).toBe('NULL')
    expect(quoteValue('postgres', undefined)).toBe('NULL')
  })
  it('booleans: pg/mysql use TRUE/FALSE, sqlite uses 1/0', () => {
    expect(quoteValue('postgres', true)).toBe('TRUE')
    expect(quoteValue('postgres', false)).toBe('FALSE')
    expect(quoteValue('mysql', true)).toBe('TRUE')
    expect(quoteValue('sqlite', true)).toBe('1')
    expect(quoteValue('sqlite', false)).toBe('0')
  })
  it('numbers render as-is', () => {
    expect(quoteValue('postgres', 42)).toBe('42')
    expect(quoteValue('postgres', -3.14)).toBe('-3.14')
    expect(quoteValue('postgres', 0)).toBe('0')
  })
  it('non-finite numbers collapse to NULL', () => {
    expect(quoteValue('postgres', Number.NaN)).toBe('NULL')
    expect(quoteValue('postgres', Number.POSITIVE_INFINITY)).toBe('NULL')
  })
  it('strings get single-quoted and escape embedded singles', () => {
    expect(quoteValue('postgres', "hello")).toBe("'hello'")
    expect(quoteValue('postgres', "it's")).toBe("'it''s'")
    expect(quoteValue('postgres', "")).toBe("''")
  })
  it('bigint passes through', () => {
    expect(quoteValue('postgres', 9007199254740993n)).toBe('9007199254740993')
  })
  it('non-string non-scalar coerces via String()', () => {
    expect(quoteValue('postgres', { a: 1 })).toBe("'[object Object]'")
  })

  describe('json/jsonb columns', () => {
    it('postgres uses ::jsonb cast', () => {
      const out = quoteValue('postgres', '{"a":1}', 'jsonb')
      expect(out).toBe(`'{"a":1}'::jsonb`)
    })
    it('postgres "json" type also gets cast', () => {
      const out = quoteValue('postgres', '{"a":1}', 'json')
      expect(out).toBe(`'{"a":1}'::jsonb`)
    })
    it('mysql wraps in CAST(... AS JSON)', () => {
      const out = quoteValue('mysql', '{"a":1}', 'JSON')
      expect(out).toBe(`CAST('{"a":1}' AS JSON)`)
    })
    it('mariadb wraps in CAST(... AS JSON)', () => {
      const out = quoteValue('mariadb', '{"a":1}', 'json')
      expect(out).toBe(`CAST('{"a":1}' AS JSON)`)
    })
    it('sqlite stores JSON as plain text', () => {
      const out = quoteValue('sqlite', '{"a":1}', 'jsonb')
      expect(out).toBe(`'{"a":1}'`)
    })
    it('escapes embedded single quotes inside the JSON string', () => {
      const out = quoteValue('postgres', `{"name":"o'malley"}`, 'jsonb')
      expect(out).toBe(`'{"name":"o''malley"}'::jsonb`)
    })
    it('non-string JSON value gets JSON.stringify-ed first', () => {
      const out = quoteValue('postgres', { a: 1, b: 'x' }, 'jsonb')
      expect(out).toBe(`'{"a":1,"b":"x"}'::jsonb`)
    })
    it('null JSON column is still NULL', () => {
      expect(quoteValue('postgres', null, 'jsonb')).toBe('NULL')
    })
  })
})

describe('coerceCellInput', () => {
  it('isExplicitNull short-circuits', () => {
    expect(coerceCellInput('anything', true, 'text')).toBe(null)
  })
  it('empty string -> null', () => {
    expect(coerceCellInput('', false, 'text')).toBe(null)
  })
  it('literal NULL (case-insensitive) -> null', () => {
    expect(coerceCellInput('NULL', false, 'text')).toBe(null)
    expect(coerceCellInput('null', false, 'text')).toBe(null)
  })
  it('boolean columns: TRUE/FALSE + 1/0', () => {
    expect(coerceCellInput('true', false, 'boolean')).toBe(true)
    expect(coerceCellInput('TRUE', false, 'bool')).toBe(true)
    expect(coerceCellInput('false', false, 'boolean')).toBe(false)
    expect(coerceCellInput('1', false, 'boolean')).toBe(true)
    expect(coerceCellInput('0', false, 'boolean')).toBe(false)
  })
  it('numeric columns: parse digits/decimals only', () => {
    expect(coerceCellInput('42', false, 'integer')).toBe(42)
    expect(coerceCellInput('-3.14', false, 'numeric')).toBe(-3.14)
    expect(coerceCellInput('0', false, 'int')).toBe(0)
    // Bad numbers stay as strings — DB will reject with a real error.
    expect(coerceCellInput('12abc', false, 'integer')).toBe('12abc')
  })
  it('text columns keep value as string', () => {
    expect(coerceCellInput('hello', false, 'text')).toBe('hello')
    expect(coerceCellInput('42', false, 'varchar')).toBe('42')
  })
  it('missing dataType still produces a sensible value', () => {
    expect(coerceCellInput('hello', false, undefined)).toBe('hello')
    expect(coerceCellInput('', false, undefined)).toBe(null)
  })
})

describe('buildInsert', () => {
  it('basic single-column insert (postgres)', () => {
    const sql = buildInsert('postgres', 'users', 'public', [
      { column: 'name', value: 'Alice' },
    ])
    expect(sql).toBe(
      `INSERT INTO "public"."users" ("name")\nVALUES ('Alice');`,
    )
  })
  it('multi-column insert (mysql)', () => {
    const sql = buildInsert('mysql', 'users', null, [
      { column: 'name', value: 'Alice' },
      { column: 'age', value: 30 },
      { column: 'active', value: true },
    ])
    expect(sql).toBe(
      "INSERT INTO `users` (`name`, `age`, `active`)\nVALUES ('Alice', 30, TRUE);",
    )
  })
  it('null-valued column emits NULL', () => {
    const sql = buildInsert('postgres', 'users', null, [
      { column: 'email', value: null },
    ])
    expect(sql).toBe(`INSERT INTO "users" ("email")\nVALUES (NULL);`)
  })
  it('json column uses cast per engine', () => {
    const sql = buildInsert('postgres', 'events', null, [
      { column: 'payload', value: '{"k":1}', dataType: 'jsonb' },
    ])
    expect(sql).toContain(`'{"k":1}'::jsonb`)
  })
  it('throws on empty bindings', () => {
    expect(() => buildInsert('postgres', 'users', null, [])).toThrow(
      /at least one column/,
    )
  })
})

describe('buildUpdate', () => {
  it('single-column update with single-PK (pg)', () => {
    const sql = buildUpdate(
      'postgres',
      'users',
      'public',
      [{ column: 'name', value: 'Bob' }],
      [{ column: 'id', value: 7 }],
    )
    expect(sql).toBe(
      `UPDATE "public"."users"\nSET "name" = 'Bob'\nWHERE "id" = 7;`,
    )
  })
  it('multi-column update with composite PK', () => {
    const sql = buildUpdate(
      'postgres',
      'user_roles',
      null,
      [{ column: 'role', value: 'admin' }],
      [
        { column: 'user_id', value: 1 },
        { column: 'org_id', value: 42 },
      ],
    )
    expect(sql).toContain('WHERE "user_id" = 1 AND "org_id" = 42;')
  })
  it('json column UPDATE emits cast', () => {
    const sql = buildUpdate(
      'postgres',
      'events',
      null,
      [{ column: 'payload', value: '{"k":2}', dataType: 'jsonb' }],
      [{ column: 'id', value: 1 }],
    )
    expect(sql).toContain(`"payload" = '{"k":2}'::jsonb`)
  })
  it('throws when no bindings', () => {
    expect(() =>
      buildUpdate('postgres', 'users', null, [], [{ column: 'id', value: 1 }]),
    ).toThrow(/at least one column/)
  })
  it('throws when no PK bindings', () => {
    expect(() =>
      buildUpdate(
        'postgres',
        'users',
        null,
        [{ column: 'name', value: 'X' }],
        [],
      ),
    ).toThrow(/PK column is required/)
  })
})

describe('buildDelete', () => {
  it('single-PK delete', () => {
    const sql = buildDelete('postgres', 'users', null, [
      { column: 'id', value: 5 },
    ])
    expect(sql).toBe(`DELETE FROM "users"\nWHERE "id" = 5;`)
  })
  it('composite-PK delete', () => {
    const sql = buildDelete('mysql', 'user_roles', null, [
      { column: 'user_id', value: 1 },
      { column: 'role_id', value: 2 },
    ])
    expect(sql).toBe(
      'DELETE FROM `user_roles`\nWHERE `user_id` = 1 AND `role_id` = 2;',
    )
  })
})

describe('buildBulkDelete', () => {
  it('single-column PK uses IN (...)', () => {
    const sql = buildBulkDelete('postgres', 'users', null, ['id'], [[1], [2], [3]])
    expect(sql).toBe(
      'DELETE FROM "users"\nWHERE "id" IN (\n  1,\n  2,\n  3\n);',
    )
  })
  it('composite PK uses row-value IN', () => {
    const sql = buildBulkDelete(
      'postgres',
      'user_roles',
      null,
      ['user_id', 'role_id'],
      [
        [1, 2],
        [3, 4],
      ],
    )
    expect(sql).toContain('WHERE ("user_id", "role_id") IN (')
    expect(sql).toContain('(1, 2)')
    expect(sql).toContain('(3, 4)')
  })
  it('throws on empty inputs', () => {
    expect(() => buildBulkDelete('postgres', 'users', null, [], [[1]])).toThrow(
      /at least one PK column/,
    )
    expect(() => buildBulkDelete('postgres', 'users', null, ['id'], [])).toThrow(
      /at least one row/,
    )
  })
  it('throws when tuple width mismatches PK count', () => {
    expect(() =>
      buildBulkDelete('postgres', 't', null, ['a', 'b'], [[1]]),
    ).toThrow(/values but 2 PK columns/)
  })
})

describe('buildCreateTable', () => {
  const cols: ColumnDef[] = [
    { name: 'id', dataType: 'bigserial', nullable: false, isPrimaryKey: true },
    { name: 'name', dataType: 'text', nullable: false, isPrimaryKey: false },
    { name: 'created_at', dataType: 'timestamp', nullable: true, default: 'now()', isPrimaryKey: false },
  ]
  it('postgres CREATE TABLE shape', () => {
    const sql = buildCreateTable('postgres', 'users', 'public', cols)
    expect(sql).toContain('CREATE TABLE "public"."users" (')
    expect(sql).toContain('"id" bigserial PRIMARY KEY NOT NULL')
    expect(sql).toContain('"name" text NOT NULL')
    expect(sql).toContain('"created_at" timestamp DEFAULT now()')
    expect(sql).not.toContain('ENGINE=InnoDB')
  })
  it('mysql gets InnoDB suffix', () => {
    const sql = buildCreateTable('mysql', 'users', null, cols)
    expect(sql).toContain('ENGINE=InnoDB')
    expect(sql).toContain('`users`')
  })
  it('throws on empty columns', () => {
    expect(() => buildCreateTable('postgres', 't', null, [])).toThrow(
      /at least one column/,
    )
  })
  it('throws when multiple PKs', () => {
    const dual: ColumnDef[] = [
      { name: 'a', dataType: 'int', nullable: false, isPrimaryKey: true },
      { name: 'b', dataType: 'int', nullable: false, isPrimaryKey: true },
    ]
    expect(() => buildCreateTable('postgres', 't', null, dual)).toThrow(
      /Multiple PRIMARY KEY/,
    )
  })
})

describe('DDL: drop/add/rename/alter column', () => {
  it('buildDropTable', () => {
    expect(buildDropTable('postgres', 'users', 'public')).toBe(
      `DROP TABLE "public"."users";`,
    )
  })
  it('buildAddColumn', () => {
    const sql = buildAddColumn('postgres', 'users', null, {
      name: 'email',
      dataType: 'text',
      nullable: true,
      isPrimaryKey: false,
    })
    expect(sql).toBe(`ALTER TABLE "users"\n  ADD COLUMN "email" text;`)
  })
  it('buildAddColumn with default + NOT NULL', () => {
    const sql = buildAddColumn('postgres', 'users', null, {
      name: 'status',
      dataType: 'text',
      nullable: false,
      default: "'pending'",
      isPrimaryKey: false,
    })
    expect(sql).toBe(
      `ALTER TABLE "users"\n  ADD COLUMN "status" text NOT NULL DEFAULT 'pending';`,
    )
  })
  it('buildDropColumn', () => {
    expect(buildDropColumn('postgres', 'users', null, 'email')).toBe(
      `ALTER TABLE "users"\n  DROP COLUMN "email";`,
    )
  })
  it('buildRenameColumn', () => {
    expect(
      buildRenameColumn('postgres', 'users', null, 'email', 'email_address'),
    ).toBe(`ALTER TABLE "users"\n  RENAME COLUMN "email" TO "email_address";`)
  })
  it('buildAlterColumnType (postgres)', () => {
    const sql = buildAlterColumnType('postgres', 'users', null, 'age', 'bigint')
    expect(sql).toBe(`ALTER TABLE "users"\n  ALTER COLUMN "age" TYPE bigint;`)
  })
  it('buildAlterColumnType (postgres) with USING', () => {
    const sql = buildAlterColumnType(
      'postgres',
      'users',
      null,
      'age',
      'text',
      'age::text',
    )
    expect(sql).toContain('ALTER COLUMN "age" TYPE text')
    expect(sql).toContain('USING age::text')
  })
  it('buildAlterColumnType (mysql) uses MODIFY', () => {
    const sql = buildAlterColumnType('mysql', 'users', null, 'age', 'bigint')
    expect(sql).toBe('ALTER TABLE `users`\n  MODIFY COLUMN `age` bigint;')
  })
  it('buildAlterColumnType (sqlite) emits commented stub', () => {
    const sql = buildAlterColumnType('sqlite', 'users', null, 'age', 'integer')
    expect(sql).toContain('SQLite does not support')
  })
})
