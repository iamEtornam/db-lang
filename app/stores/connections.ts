import { invoke } from '@tauri-apps/api/core'
import { defineStore } from 'pinia'
import type { ColumnInfo, Connection, CreateConnectionRequest, TableInfo } from '~/types/database'

export const useConnectionsStore = defineStore('connections', () => {
  const connections = ref<Connection[]>([])
  const activeConnectionId = ref<string | null>(null)
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  const tables = ref<TableInfo[]>([])
  const tableColumns = ref<Record<string, ColumnInfo[]>>({})
  const schemaContext = ref('')
  const isLoadingSchema = ref(false)
  const schemaLoadedForConnectionId = ref('')

  const activeConnection = computed(() =>
    connections.value.find(c => c.id === activeConnectionId.value) ?? null,
  )

  async function loadConnections() {
    isLoading.value = true
    error.value = null

    try {
      const result = await invoke<Connection[]>('get_connections')
      connections.value = result
    }
    catch (err) {
      error.value = err as string
    }
    finally {
      isLoading.value = false
    }
  }

  async function addConnection(connection: CreateConnectionRequest): Promise<Connection> {
    isLoading.value = true
    error.value = null

    try {
      const result = await invoke<Connection>('save_connection', { connection })
      connections.value.unshift(result)
      activeConnectionId.value = result.id
      return result
    }
    catch (err) {
      error.value = err as string
      throw err
    }
    finally {
      isLoading.value = false
    }
  }

  async function updateConnection(connection: Connection): Promise<Connection | null> {
    isLoading.value = true
    error.value = null

    try {
      const result = await invoke<Connection>('update_connection', { connection })
      const idx = connections.value.findIndex(c => c.id === result.id)
      if (idx !== -1) connections.value[idx] = result
      // Reload schema if this is the active connection
      if (activeConnectionId.value === result.id) {
        clearSchema()
      }
      return result
    }
    catch (err) {
      error.value = err as string
      return null
    }
    finally {
      isLoading.value = false
    }
  }

  async function deleteConnection(connectionId: string): Promise<boolean> {
    isLoading.value = true
    error.value = null

    try {
      await invoke<boolean>('delete_connection_record', { connectionId })
      connections.value = connections.value.filter(c => c.id !== connectionId)

      if (activeConnectionId.value === connectionId) {
        activeConnectionId.value = connections.value[0]?.id ?? null
        clearSchema()
      }
      return true
    }
    catch (err) {
      error.value = err as string
      return false
    }
    finally {
      isLoading.value = false
    }
  }

  function setActiveConnection(connectionId: string) {
    activeConnectionId.value = connectionId || null
    if (!connectionId) {
      clearSchema()
    }
  }

  function clearSchema() {
    tables.value = []
    tableColumns.value = {}
    schemaContext.value = ''
    schemaLoadedForConnectionId.value = ''
  }

  async function loadSchema(): Promise<boolean> {
    const conn = activeConnection.value
    if (!conn) return false

    if (schemaLoadedForConnectionId.value === conn.id && tables.value.length > 0) {
      return true
    }

    isLoadingSchema.value = true

    try {
      const loadedTables = await invoke<TableInfo[]>('get_tables', {
        connectionId: conn.id,
      })

      tables.value = loadedTables

      let ctx = `Database engine: ${conn.db_type}\n`
      ctx += `IMPORTANT: Table and column names are CASE-SENSITIVE. Use the EXACT names shown below.\n\n`
      ctx += `Tables:\n`

      const colMap: Record<string, ColumnInfo[]> = {}

      for (const table of loadedTables) {
        const fullName = table.schema ? `"${table.schema}"."${table.name}"` : `"${table.name}"`

        try {
          const columns = await invoke<ColumnInfo[]>('get_table_columns', {
            connectionId: conn.id,
            tableName: table.name,
            schemaName: table.schema,
          })

          colMap[table.name] = columns

          const colDescriptions = columns.map((col) => {
            let desc = `    - "${col.name}" (${col.data_type})`
            if (col.is_primary_key) desc += ' [PK]'
            if (col.is_foreign_key && col.referenced_table) {
              desc += ` [FK -> "${col.referenced_table}"."${col.referenced_column}"]`
            }
            if (!col.is_nullable) desc += ' NOT NULL'
            return desc
          })

          ctx += `\n  Table ${fullName} (${table.table_type}):\n${colDescriptions.join('\n')}\n`
        }
        catch {
          ctx += `\n  Table ${fullName} (${table.table_type}): [columns unavailable]\n`
        }
      }

      tableColumns.value = colMap
      schemaContext.value = ctx
      schemaLoadedForConnectionId.value = conn.id

      return true
    }
    catch (err) {
      console.error('Failed to load schema:', err)
      clearSchema()
      return false
    }
    finally {
      isLoadingSchema.value = false
    }
  }

  function clearError() {
    error.value = null
  }

  return {
    connections,
    activeConnectionId,
    activeConnection,
    isLoading,
    error,
    tables,
    tableColumns,
    schemaContext,
    isLoadingSchema,
    schemaLoadedForConnectionId,
    loadConnections,
    addConnection,
    updateConnection,
    deleteConnection,
    setActiveConnection,
    loadSchema,
    clearSchema,
    clearError,
  }
})
