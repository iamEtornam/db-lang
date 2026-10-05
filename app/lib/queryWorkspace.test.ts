import { describe, expect, it } from 'vitest'
import { closeQueryTab, queryForExecution, restoreWorkspace, type QueryWorkspace } from './queryWorkspace'

const workspace: QueryWorkspace = {
  tabs: [
    { id: 'a', name: 'First', query: 'select 1', prompt: '' },
    { id: 'b', name: 'Second', query: 'select 2', prompt: 'two' },
  ],
  activeId: 'b',
}

describe('query drafts', () => {
  it('restores both manual queries and prompts', () => {
    expect(restoreWorkspace(JSON.stringify(workspace))).toEqual(workspace)
  })
  it('recovers an absent active tab', () => {
    expect(restoreWorkspace(JSON.stringify({ ...workspace, activeId: 'missing' })).activeId).toBe('a')
  })
  it('rejects corrupt, oversized, and duplicate tabs', () => {
    for (const value of [null, {}, { tabs: [] }, { tabs: [workspace.tabs[0], workspace.tabs[0]] },
      { tabs: [{ ...workspace.tabs[0], query: 'x'.repeat(200_001) }] }]) {
      expect(() => restoreWorkspace(JSON.stringify(value))).toThrow()
    }
  })
  it('selects a surviving tab without losing its draft', () => {
    const next = closeQueryTab(workspace, 'b')
    expect(next.activeId).toBe('a')
    expect(next.tabs[0]!.query).toBe('select 1')
    expect(workspace.tabs).toHaveLength(2)
  })
  it('retains one empty tab when the final tab is closed', () => {
    const next = closeQueryTab({ tabs: [workspace.tabs[0]!], activeId: 'a' }, 'a')
    expect(next.tabs).toHaveLength(1)
    expect(next.tabs[0]!.query).toBe('')
    expect(next.activeId).toBe(next.tabs[0]!.id)
  })
})


describe('execution query selection', () => {
  it('runs a selection and paginates it in both directions after the draft changes', () => {
    const draft = 'SELECT 1; SELECT 2;'
    const selected = 'SELECT 2;'
    expect(queryForExecution(draft, '', 1, selected)).toBe(selected)
    expect(queryForExecution('SELECT 3;', selected, 2)).toBe(selected)
    expect(queryForExecution('SELECT 3;', selected, 1)).toBe(selected)
    expect(queryForExecution('SELECT 3;', selected)).toBe('SELECT 3;')
  })
})
