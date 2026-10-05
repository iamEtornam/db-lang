export interface QueryTab {
  id: string
  name: string
  query: string
  prompt: string
}

export interface QueryWorkspace {
  tabs: QueryTab[]
  activeId: string
}

export const MAX_QUERY_TABS = 20
const MAX_DRAFT_LENGTH = 200_000

export function newQueryTab(): QueryTab {
  return { id: crypto.randomUUID(), name: 'Untitled query', query: '', prompt: '' }
}

export function restoreWorkspace(raw: string): QueryWorkspace {
  const value: unknown = JSON.parse(raw)
  if (!value || typeof value !== 'object') throw new Error('Invalid workspace')
  const { tabs, activeId } = value as QueryWorkspace
  if (!Array.isArray(tabs) || tabs.length === 0 || tabs.length > MAX_QUERY_TABS
    || tabs.some(t => !t || typeof t.id !== 'string' || !t.id
      || typeof t.name !== 'string' || t.name.length > 100
      || typeof t.query !== 'string' || t.query.length > MAX_DRAFT_LENGTH
      || typeof t.prompt !== 'string' || t.prompt.length > MAX_DRAFT_LENGTH)
    || new Set(tabs.map(t => t.id)).size !== tabs.length) {
    throw new Error('Invalid query tabs')
  }
  return { tabs, activeId: tabs.some(t => t.id === activeId) ? activeId : tabs[0]!.id }
}

export function closeQueryTab(workspace: QueryWorkspace, id: string): QueryWorkspace {
  const index = workspace.tabs.findIndex(t => t.id === id)
  if (index < 0) return workspace
  const tabs = workspace.tabs.filter(t => t.id !== id)
  if (!tabs.length) tabs.push(newQueryTab())
  return {
    tabs,
    activeId: workspace.activeId === id ? tabs[Math.min(index, tabs.length - 1)]!.id : workspace.activeId,
  }
}

/** An explicit page always continues the executed query, including page one. */
export function queryForExecution(draft: string, executed: string, page?: number, selection?: string): string {
  return selection ?? (page !== undefined ? executed : draft)
}
