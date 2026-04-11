import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export function useTauri() {
  async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    return tauriInvoke<T>(command, args)
  }

  async function onEvent<T>(event: string, callback: (payload: T) => void) {
    return listen<T>(event, e => callback(e.payload))
  }

  return { invoke, onEvent }
}
