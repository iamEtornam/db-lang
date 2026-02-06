import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';

export interface Connection {
  id: string;
  name: string;
  db_type: string;
  host: string;
  port: string;
  database: string;
  username: string;
  password: string;
  ssl_enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateConnectionRequest {
  name: string;
  db_type: string;
  host: string;
  port: string;
  database: string;
  username: string;
  password: string;
  ssl_enabled: boolean;
}

export const useConnectionsStore = defineStore('connections', () => {
  const connections = ref<Connection[]>([]);
  const activeConnectionId = ref<string | null>(null);
  const isLoading = ref(false);
  const error = ref<string | null>(null);

  const activeConnection = computed(() => 
    connections.value.find(c => c.id === activeConnectionId.value) || null
  );

  async function loadConnections() {
    isLoading.value = true;
    error.value = null;

    try {
      const result = await invoke<Connection[]>('get_connections');
      connections.value = result;
      
      if (!activeConnectionId.value && result.length > 0) {
        activeConnectionId.value = result[0].id;
      }
    } catch (err) {
      error.value = err as string;
    } finally {
      isLoading.value = false;
    }
  }

  async function addConnection(connection: CreateConnectionRequest): Promise<Connection | null> {
    isLoading.value = true;
    error.value = null;

    try {
      const result = await invoke<Connection>('save_connection', { connection });
      connections.value.unshift(result);
      activeConnectionId.value = result.id;
      return result;
    } catch (err) {
      error.value = err as string;
      return null;
    } finally {
      isLoading.value = false;
    }
  }

  async function deleteConnection(connectionId: string): Promise<boolean> {
    isLoading.value = true;
    error.value = null;

    try {
      await invoke<boolean>('delete_connection_record', { connectionId });
      connections.value = connections.value.filter(c => c.id !== connectionId);
      
      if (activeConnectionId.value === connectionId) {
        activeConnectionId.value = connections.value[0]?.id || null;
      }
      return true;
    } catch (err) {
      error.value = err as string;
      return false;
    } finally {
      isLoading.value = false;
    }
  }

  function setActiveConnection(connectionId: string) {
    activeConnectionId.value = connectionId;
  }

  function buildConnectionString(conn: Connection): string {
    const { db_type, host, port, database, username, password } = conn;
    const dbName = database || getDefaultDatabase(db_type);
    
    switch (db_type) {
      case 'postgres':
        return `postgresql://${username}:${password}@${host}:${port}/${dbName}`;
      case 'mysql':
        return `mysql://${username}:${password}@${host}:${port}/${dbName}`;
      case 'sqlite':
        return host;
      case 'mssql':
        return `mssql://${username}:${password}@${host}:${port}/${dbName}`;
      default:
        return '';
    }
  }

  function getDefaultDatabase(dbType: string): string {
    switch (dbType) {
      case 'postgres': return 'postgres';
      case 'mysql': return 'mysql';
      case 'mssql': return 'master';
      default: return '';
    }
  }

  function clearError() {
    error.value = null;
  }

  return {
    connections,
    activeConnectionId,
    activeConnection,
    isLoading,
    error,
    loadConnections,
    addConnection,
    deleteConnection,
    setActiveConnection,
    buildConnectionString,
    clearError,
  };
});
