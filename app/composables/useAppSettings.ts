import { invoke } from '@tauri-apps/api/core'
import type { UserSettings } from '~/types/database'

export function useAppSettings() {
  const settings = ref<UserSettings | null>(null)
  const isLoading = ref(false)

  async function loadSettings() {
    isLoading.value = true
    try {
      settings.value = await invoke<UserSettings>('get_settings')
    }
    catch (err) {
      console.error('Failed to load settings:', err)
    }
    finally {
      isLoading.value = false
    }
  }

  async function updateSettings(updates: Partial<UserSettings>) {
    try {
      settings.value = await invoke<UserSettings>('update_settings', { settings: updates })
    }
    catch (err) {
      console.error('Failed to update settings:', err)
    }
  }

  return {
    settings,
    isLoading,
    loadSettings,
    updateSettings,
  }
}
