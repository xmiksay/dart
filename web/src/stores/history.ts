import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '@/api/client'
import type { GameState, GameSummary } from '@/api/protocol'

/** Server state for finished games, fetched over REST. */
export const useHistoryStore = defineStore('history', () => {
  const games = ref<GameSummary[]>([])
  // A finished game reconstructed as a full board snapshot for the grid view.
  const board = ref<GameState | null>(null)
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

  async function loadBoard(id: string) {
    loading.value = true
    error.value = null
    board.value = null
    try {
      board.value = await api.getGameState(id)
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'failed to load game'
    } finally {
      loading.value = false
    }
  }

  function clearBoard() {
    board.value = null
  }

  return { games, board, loading, error, loadGames, loadBoard, clearBoard }
})
