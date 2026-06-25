import { invoke } from '@tauri-apps/api/core'
import { defineStore } from 'pinia'
import type { Chart } from '~/types/database'

export interface SaveChartRequest {
  id?: string | null
  name: string
  description?: string | null
  connection_id?: string | null
  engine: string
  query: string
  chart_type: string
  config_json: string
}

export const useChartsStore = defineStore('charts', () => {
  const charts = ref<Chart[]>([])
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  async function loadCharts() {
    isLoading.value = true
    error.value = null
    try {
      charts.value = await invoke<Chart[]>('list_charts')
    }
    catch (err) {
      error.value = err as string
    }
    finally {
      isLoading.value = false
    }
  }

  /** Upsert: omit `id` to create, include it to update. Keeps the list sorted by recency. */
  async function saveChart(chart: SaveChartRequest): Promise<Chart | null> {
    isLoading.value = true
    error.value = null
    try {
      const result = await invoke<Chart>('save_chart', { chart })
      const index = charts.value.findIndex(c => c.id === result.id)
      if (index !== -1) charts.value.splice(index, 1)
      charts.value.unshift(result)
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

  async function deleteChart(chartId: string): Promise<boolean> {
    isLoading.value = true
    error.value = null
    try {
      await invoke<boolean>('delete_chart', { chartId })
      charts.value = charts.value.filter(c => c.id !== chartId)
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

  /** Re-run a saved chart's stored query and return fresh rows. */
  async function runChart(chartId: string): Promise<Record<string, unknown>[]> {
    const data = await invoke<string>('run_chart', { chartId })
    return JSON.parse(data) as Record<string, unknown>[]
  }

  function clearError() {
    error.value = null
  }

  return {
    charts,
    isLoading,
    error,
    loadCharts,
    saveChart,
    deleteChart,
    runChart,
    clearError,
  }
})
