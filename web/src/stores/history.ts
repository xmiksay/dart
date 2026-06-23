import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '@/api/client'
import type { GameDetail, GameSummary } from '@/api/protocol'

/** Server state for finished games, fetched over REST. */
export const useHistoryStore = defineStore('history', () => {
  const games = ref<GameSummary[]>([])
  const selected = ref<GameDetail | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function loadGames() {
    loading.value = true
    error.value = null
    try {
      games.value = await api.listGames()
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'failed to load games'
    } finally {
      loading.value = false
    }
  }

  async function loadGame(id: number) {
    loading.value = true
    error.value = null
    selected.value = null
    try {
      selected.value = await api.getGame(id)
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'failed to load game'
    } finally {
      loading.value = false
    }
  }

  function clearSelected() {
    selected.value = null
  }

  return { games, selected, loading, error, loadGames, loadGame, clearSelected }
})
