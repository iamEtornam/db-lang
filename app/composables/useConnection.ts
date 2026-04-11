import { storeToRefs } from 'pinia'
import { useConnectionsStore } from '~/stores/connections'

export function useConnection() {
  const store = useConnectionsStore()
  const { connections, activeConnection, activeConnectionId, activeConnectionString, isLoading, error } = storeToRefs(store)

  return {
    connections,
    activeConnection,
    activeConnectionId,
    activeConnectionString,
    isLoading,
    error,
    loadConnections: store.loadConnections,
    addConnection: store.addConnection,
    deleteConnection: store.deleteConnection,
    setActiveConnection: store.setActiveConnection,
    clearError: store.clearError,
  }
}
