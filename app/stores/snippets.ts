import { invoke } from '@tauri-apps/api/core'
import { defineStore } from 'pinia'
import type { Snippet } from '~/types/database'

export interface CreateSnippetRequest {
  name: string
  description?: string | null
  natural_query: string
  sql_query: string
  tags?: string | null
}

export interface UpdateSnippetRequest {
  id: string
  name: string
  description?: string | null
  natural_query: string
  sql_query: string
  tags?: string | null
}

export const useSnippetsStore = defineStore('snippets', () => {
  const snippets = ref<Snippet[]>([])
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  const allTags = computed(() => {
    const tags = new Set<string>()
    snippets.value.forEach((snippet) => {
      if (snippet.tags) {
        snippet.tags.split(',').forEach((tag) => {
          const trimmed = tag.trim()
          if (trimmed) tags.add(trimmed)
        })
      }
    })
    return Array.from(tags).sort()
  })

  async function loadSnippets() {
    isLoading.value = true
    error.value = null

    try {
      snippets.value = await invoke<Snippet[]>('get_snippets')
    }
    catch (err) {
      error.value = err as string
    }
    finally {
      isLoading.value = false
    }
  }

  async function createSnippet(snippet: CreateSnippetRequest): Promise<Snippet | null> {
    isLoading.value = true
    error.value = null

    try {
      const result = await invoke<Snippet>('create_snippet', { snippet })
      snippets.value.unshift(result)
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

  async function updateSnippet(snippet: UpdateSnippetRequest): Promise<boolean> {
    isLoading.value = true
    error.value = null

    try {
      await invoke<boolean>('update_snippet', { snippet })

      const index = snippets.value.findIndex(s => s.id === snippet.id)
      if (index !== -1) {
        snippets.value[index] = {
          ...snippets.value[index],
          name: snippet.name,
          description: snippet.description ?? null,
          natural_query: snippet.natural_query,
          sql_query: snippet.sql_query,
          tags: snippet.tags ?? '',
          updated_at: new Date().toISOString(),
        }
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

  async function deleteSnippet(snippetId: string): Promise<boolean> {
    isLoading.value = true
    error.value = null

    try {
      await invoke<boolean>('delete_snippet', { snippetId })
      snippets.value = snippets.value.filter(s => s.id !== snippetId)
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

  function searchSnippets(searchTerm: string): Snippet[] {
    const term = searchTerm.toLowerCase()
    return snippets.value.filter(s =>
      s.name.toLowerCase().includes(term)
      || s.natural_query.toLowerCase().includes(term)
      || s.sql_query.toLowerCase().includes(term)
      || s.tags.toLowerCase().includes(term)
      || (s.description && s.description.toLowerCase().includes(term)),
    )
  }

  function filterByTag(tag: string): Snippet[] {
    return snippets.value.filter(s =>
      s.tags.toLowerCase().split(',').map(t => t.trim()).includes(tag.toLowerCase()),
    )
  }

  function clearError() {
    error.value = null
  }

  return {
    snippets,
    isLoading,
    error,
    allTags,
    loadSnippets,
    createSnippet,
    updateSnippet,
    deleteSnippet,
    searchSnippets,
    filterByTag,
    clearError,
  }
})
