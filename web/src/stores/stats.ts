import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '@/api/client'
import type { WindowStats } from '@/api/protocol'

/** X01 statistics over rolling windows, fetched over REST. */
export const useStatsStore = defineStore('stats', () => {
  const windows = ref<WindowStats[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function load() {
    loading.value = true
    error.value = null
    try {
      windows.value = (await api.getStats()).windows
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'failed to load stats'
    } finally {
      loading.value = false
    }
  }

  return { windows, loading, error, load }
})
