import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { toast } from 'vue-sonner'
import type { SchemaKnowledgeBase, SchemaSnapshot } from '~/types/database'

interface KbProgressEvent {
  connection_id: string
  table_name: string
  current: number
  total: number
  status: 'processing' | 'done' | 'error'
}

export function useSchemaKb() {
  const kb = ref<SchemaKnowledgeBase | null>(null)
  const isGenerating = ref(false)
  const progress = ref({ current: 0, total: 0, currentTable: '' })
  const error = ref<string | null>(null)

  async function getKb(connectionId: string): Promise<SchemaKnowledgeBase | null> {
    try {
      const result = await invoke<SchemaKnowledgeBase | null>('get_schema_kb', { connectionId })
      kb.value = result
      return result
    }
    catch {
      return null
    }
  }

  async function generateKb(connectionId: string): Promise<void> {
    isGenerating.value = true
    progress.value = { current: 0, total: 0, currentTable: '' }
    error.value = null

    const unlisten = await listen<KbProgressEvent>('schema_kb_progress', (event) => {
      if (event.payload.connection_id === connectionId) {
        progress.value = {
          current: event.payload.current,
          total: event.payload.total,
          currentTable: event.payload.table_name,
        }
      }
    })

    try {
      await invoke('generate_schema_kb', { connectionId })
      await getKb(connectionId)
      toast.success('Schema analyzed', {
        description: 'AI knowledge base is ready for intelligent querying',
      })
    }
    catch (err) {
      error.value = err as string
      toast.error('Schema analysis failed', { description: err as string })
    }
    finally {
      isGenerating.value = false
      unlisten()
    }
  }

  async function refreshKb(connectionId: string): Promise<void> {
    await generateKb(connectionId)
  }

  async function updateTableDescription(tableDescId: string, description: string): Promise<void> {
    try {
      await invoke('update_table_description', { tableDescId, description })

      if (kb.value) {
        const idx = kb.value.tables.findIndex(t => t.id === tableDescId)
        if (idx !== -1) {
          kb.value.tables[idx].ai_description = description
        }
      }

      toast.success('Description updated')
    }
    catch (err) {
      toast.error('Failed to update description', { description: err as string })
    }
  }

  function getSnapshotStatus(connectionId: string): SchemaSnapshot['status'] | 'none' {
    if (!kb.value) return 'none'
    if (kb.value.snapshot.connection_id !== connectionId) return 'none'
    return kb.value.snapshot.status
  }

  return {
    kb,
    isGenerating,
    progress,
    error,
    getKb,
    generateKb,
    refreshKb,
    updateTableDescription,
    getSnapshotStatus,
  }
}
