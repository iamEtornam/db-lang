import { toast } from 'vue-sonner'
import { closeQueryTab, MAX_QUERY_TABS, newQueryTab, restoreWorkspace, type QueryWorkspace } from '~/lib/queryWorkspace'

/** Drafts are local to this installation and scoped to a saved connection. */
export function useQueryWorkspace(connectionId: Ref<string | undefined>) {
  const tab = newQueryTab()
  const workspace = ref<QueryWorkspace>({ tabs: [tab], activeId: tab.id })
  const storageError = ref('')
  const pendingCloseId = ref<string | null>(null)
  let loaded = false
  const key = () => `qs-query-workspace-v1:${connectionId.value ?? 'unconnected'}`

  function save() {
    if (!loaded) return
    try {
      const serialized = JSON.stringify(workspace.value)
      restoreWorkspace(serialized)
      localStorage.setItem(key(), serialized)
      storageError.value = ''
    }
    catch {
      storageError.value = 'Drafts could not be saved. Copy your query before closing the app.'
    }
  }

  watch(connectionId, () => {
    loaded = false
    pendingCloseId.value = null
    storageError.value = ''
    const initial = newQueryTab()
    workspace.value = { tabs: [initial], activeId: initial.id }
    try {
      const raw = localStorage.getItem(key())
      if (raw) workspace.value = restoreWorkspace(raw)
    }
    catch {
      storageError.value = 'Saved drafts could not be restored. The previous saved data has been kept.'
      return // Do not overwrite an unreadable saved workspace.
    }
    loaded = true
  }, { immediate: true, flush: 'sync' })
  watch(workspace, save, { deep: true, flush: 'sync' })

  const activeTab = computed(() => workspace.value.tabs.find(t => t.id === workspace.value.activeId)!)
  function addTab() {
    if (workspace.value.tabs.length >= MAX_QUERY_TABS) {
      toast.info(`Close a tab before opening more than ${MAX_QUERY_TABS} queries`)
      return
    }
    const next = newQueryTab()
    workspace.value = { tabs: [...workspace.value.tabs, next], activeId: next.id }
  }
  function closeTab(id: string) {
    const closing = workspace.value.tabs.find(t => t.id === id)
    if (closing && (closing.query.trim() || closing.prompt.trim())) {
      pendingCloseId.value = id
      return
    }
    workspace.value = closeQueryTab(workspace.value, id)
  }
  function confirmClose() {
    if (pendingCloseId.value) workspace.value = closeQueryTab(workspace.value, pendingCloseId.value)
    pendingCloseId.value = null
  }
  return { workspace, activeTab, storageError, pendingCloseId, addTab, closeTab, confirmClose }
}
