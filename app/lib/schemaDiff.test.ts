import { describe, it, expect } from 'vitest'
import { compareSchemas, migrationDraft } from './schemaDiff'
import type { ComparisonSchema } from './schemaDiff'
import type { ColumnInfo } from '~/types/database'
const column = (name: string, extra: Partial<ColumnInfo> = {}): ColumnInfo => ({name,data_type:'text',is_nullable:true,column_default:null,is_primary_key:false,is_foreign_key:false,referenced_table:null,referenced_column:null,...extra})
const snapshot = (columns: ColumnInfo[], schema: string | null = 'public', engine = 'postgres', special_columns: string[] = []): ComparisonSchema => ({engine,tables:[{table:{name:'people',schema,table_type:'BASE TABLE'},columns,special_columns}]})
describe('schema comparison and conservative migration draft', () => {
  it('compares directionally, retains native lengths and quotes target identifiers', () => {
    const source = snapshot([column('name', {data_type:'character varying(30)'}),column('new"column')])
    const target = snapshot([column('name', {data_type:'character varying(10)'}),column('old')], 'stage"schema')
    const changes = compareSchemas(source,target,'public','stage"schema')
    expect(changes.map(row => row.kind)).toEqual(['change','add','remove'])
    expect(changes[1]?.sql).toBe('ALTER TABLE "stage""schema"."people" ADD COLUMN "new""column" text;')
    expect(migrationDraft(changes)).not.toContain('DROP COLUMN')
    expect(compareSchemas(source,source,'public','public')).toEqual([])
  })
  it('marks unsafe additions, missing tables and generated columns for manual review', () => {
    const source = snapshot([column('required',{is_nullable:false}),column('defaulted',{column_default:"'x'"}),column('key',{is_primary_key:true}),column('derived'),column('custom',{data_type:'text; DROP TABLE people;'})], 'public','postgres',['derived'])
    expect(compareSchemas(source,snapshot([]),'public','public').every(row => row.sql === null)).toBe(true)
    expect(compareSchemas(source,{engine:'postgres',tables:[]},'public',null)[0]?.column).toBe(null)
    const sqlite = snapshot([],null,'sqlite',['generated'])
    const ordinary = snapshot([column('generated')],null,'sqlite')
    for (const [from,to] of [[sqlite,ordinary],[ordinary,sqlite]] as const) {
      const differences = compareSchemas(from,to,null,null)
      expect(differences).toHaveLength(1)
      expect(differences[0]?.kind).toBe('change')
      expect(differences[0]?.sql).toBeNull()
    }
    expect(compareSchemas(sqlite,snapshot([],null,'sqlite'),null,null)[0]?.source).toBe('Generated or hidden column')
  })
  it('rejects cross-engine diffs, supports MariaDB/MySQL and handles empty databases', () => {
    expect(() => compareSchemas(snapshot([]),snapshot([],null,'sqlite'),'public',null)).toThrow('same SQL engine')
    expect(compareSchemas(snapshot([column('value',{data_type:'varchar(30)'})],'dev','mariadb'),snapshot([],'prod','mysql'),'dev','prod')[0]?.sql).toBe('ALTER TABLE `prod`.`people` ADD COLUMN `value` varchar(30);')
    expect(compareSchemas({engine:'sqlite',tables:[]},{engine:'sqlite',tables:[]},null,null)).toEqual([])
  })
})
