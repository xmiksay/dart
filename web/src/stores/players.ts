import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api, ApiError } from '@/api/client'
import type { SavedPlayer } from '@/api/protocol'

/** Server state for the DB-backed roster of saved players, fetched over REST. */
export const usePlayersStore = defineStore('players', () => {
  const players = ref<SavedPlayer[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  function sortByName() {
    players.value.sort((a, b) => a.name.localeCompare(b.name))
  }

  async function load() {
    loading.value = true
    error.value = null
    try {
      players.value = await api.listPlayers()
      sortByName()
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'failed to load players'
    } finally {
      loading.value = false
    }
  }

  /**
   * Persist a newly typed name. A 409 means the name already exists, which is a
   * no-op for our purposes (the caller still adds it to the live roster).
   */
  async function create(name: string) {
    error.value = null
    try {
      const created = await api.createPlayer(name)
      players.value.push(created)
      sortByName()
    } catch (err) {
      if (err instanceof ApiError && err.status === 409) return
      error.value = err instanceof Error ? err.message : 'failed to save player'
    }
  }

  async function rename(id: string, name: string) {
    error.value = null
    try {
      const updated = await api.renamePlayer(id, name)
      const i = players.value.findIndex((p) => p.id === id)
      if (i !== -1) players.value[i] = updated
      sortByName()
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'failed to rename player'
    }
  }

  async function remove(id: string) {
    error.value = null
    try {
      await api.deletePlayer(id)
      players.value = players.value.filter((p) => p.id !== id)
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'failed to delete player'
    }
  }

  return { players, loading, error, load, create, rename, remove }
})
