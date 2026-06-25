import { invoke } from '@tauri-apps/api/core'
import { defineStore } from 'pinia'
import type { Script, ScriptParam } from '~/types/database'

export interface CreateScriptRequest {
  name: string
  description?: string | null
  engine: string
  query_language: string
  body: string
  params_json?: string | null
  tags?: string | null
}

export interface UpdateScriptRequest extends CreateScriptRequest {
  id: string
}

export function parseScriptParams(script: Script): ScriptParam[] {
  try {
    const parsed = JSON.parse(script.params_json || '[]')
    return Array.isArray(parsed) ? parsed : []
  }
  catch {
    return []
  }
}

export const useScriptsStore = defineStore('scripts', () => {
  const scripts = ref<Script[]>([])
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  const builtinScripts = computed(() => scripts.value.filter(s => s.is_builtin))
  const userScripts = computed(() => scripts.value.filter(s => !s.is_builtin))

  async function loadScripts() {
    isLoading.value = true
    error.value = null
    try {
      scripts.value = await invoke<Script[]>('get_scripts')
    }
    catch (err) {
      error.value = err as string
    }
    finally {
      isLoading.value = false
    }
  }

  async function createScript(script: CreateScriptRequest): Promise<Script | null> {
    error.value = null
    try {
      const result = await invoke<Script>('create_script', { script })
      scripts.value.unshift(result)
      return result
    }
    catch (err) {
      error.value = err as string
      return null
    }
  }

  async function updateScript(script: UpdateScriptRequest): Promise<boolean> {
    error.value = null
    try {
      await invoke<boolean>('update_script', { script })
      await loadScripts()
      return true
    }
    catch (err) {
      error.value = err as string
      return false
    }
  }

  async function deleteScript(scriptId: string): Promise<boolean> {
    error.value = null
    try {
      await invoke<boolean>('delete_script', { scriptId })
      scripts.value = scripts.value.filter(s => s.id !== scriptId)
      return true
    }
    catch (err) {
      error.value = err as string
      return false
    }
  }

  /** Run a script against a connection. Returns parsed rows or throws. */
  async function runScript(
    connectionId: string,
    scriptId: string,
    params: Record<string, string>,
  ): Promise<unknown[]> {
    const json = await invoke<string>('run_script', {
      connectionId,
      scriptId,
      params,
    })
    return JSON.parse(json)
  }

  function clearError() {
    error.value = null
  }

  return {
    scripts,
    isLoading,
    error,
    builtinScripts,
    userScripts,
    loadScripts,
    createScript,
    updateScript,
    deleteScript,
    runScript,
    clearError,
  }
})
