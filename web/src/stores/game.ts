import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { GameSocket, type WsStatus } from '@/api/ws'
import type {
  ClientCommand,
  Dart,
  GameState,
  NewGamePayload,
  PlayerId,
  ServerEvent,
} from '@/api/protocol'

const IDLE_STATE: GameState = {
  mode: 'idle',
  players: [],
  current_player_id: null,
  current_turn: [],
  turns: [],
  winner_id: null,
}

/** Which dart the operator is currently editing (`turns[t].darts[d]`). */
export interface EditTarget {
  turnIndex: number
  dartIndex: number
}

/**
 * Holds the single source of truth for live play: the latest broadcast
 * GameState, the WS connection status, and the last server error. All mutations
 * happen on the server — we only send commands and apply the echoed `state`.
 */
export const useGameStore = defineStore('game', () => {
  const state = ref<GameState>(IDLE_STATE)
  const status = ref<WsStatus>('closed')
  const lastError = ref<string | null>(null)
  // Bumped whenever a fresh dart arrives so views can trigger land animations.
  const dartTick = ref(0)
  // The dart the operator picked to correct; null = next pick is a fresh throw.
  const editTarget = ref<EditTarget | null>(null)

  let socket: GameSocket | null = null
  let prevTurnLen = 0

  function applyEvent(event: ServerEvent) {
    if (event.type === 'state') {
      const next = event.state
      if (next.current_turn.length > prevTurnLen) dartTick.value++
      prevTurnLen = next.current_turn.length
      state.value = next
      lastError.value = null
      // Drop a stale edit target if the leg shrank (undo/reset) past it.
      const t = editTarget.value
      if (t && !next.turns[t.turnIndex]?.darts[t.dartIndex]) {
        editTarget.value = null
      }
    } else if (event.type === 'error') {
      lastError.value = event.message
    }
  }

  function connect() {
    if (socket) return
    socket = new GameSocket({
      onEvent: applyEvent,
      onStatus: (s) => (status.value = s),
    })
    socket.connect()
  }

  function disconnect() {
    socket?.close()
    socket = null
  }

  function send(command: ClientCommand) {
    socket?.send(command)
  }

  function clearError() {
    lastError.value = null
  }

  // --- roster commands ---
  const addPlayer = (name: string) => send({ type: 'player_add', name })
  const renamePlayer = (id: PlayerId, name: string) =>
    send({ type: 'player_rename', id, name })
  const removePlayer = (id: PlayerId) => send({ type: 'player_remove', id })
  const reorderPlayer = (id: PlayerId, index: number) =>
    send({ type: 'player_reorder', id, index })

  // --- game lifecycle ---
  const newGame = (payload: NewGamePayload) =>
    send({ type: 'game_new', ...payload })
  const reset = () => send({ type: 'reset' })

  // --- play / correction ---
  const throwDart = (dart: Dart) => send({ type: 'throw', dart })
  const undoLast = () => send({ type: 'undo_last' })
  const editDart = (turnIndex: number, dartIndex: number, dart: Dart) =>
    send({ type: 'edit_dart', turn_index: turnIndex, dart_index: dartIndex, dart })
  const endTurn = () => send({ type: 'end_turn' })

  // --- edit-target selection (shared by history view + operator picker) ---
  const setEditTarget = (turnIndex: number, dartIndex: number) => {
    editTarget.value = { turnIndex, dartIndex }
  }
  const clearEditTarget = () => {
    editTarget.value = null
  }

  // --- derived views ---
  const players = computed(() => state.value.players)
  const mode = computed(() => state.value.mode)
  const isIdle = computed(() => state.value.mode === 'idle')
  const currentPlayerId = computed(() => state.value.current_player_id)
  const currentTurn = computed(() => state.value.current_turn)
  const turns = computed(() => state.value.turns)
  const winnerId = computed(() => state.value.winner_id)
  const winner = computed(
    () => players.value.find((p) => p.id === winnerId.value) ?? null,
  )
  const connected = computed(() => status.value === 'open')

  function playerName(id: PlayerId | null): string {
    if (id === null) return ''
    return players.value.find((p) => p.id === id)?.name ?? `#${id}`
  }

  return {
    state,
    status,
    lastError,
    dartTick,
    editTarget,
    connect,
    disconnect,
    clearError,
    addPlayer,
    renamePlayer,
    removePlayer,
    reorderPlayer,
    newGame,
    reset,
    throwDart,
    undoLast,
    editDart,
    endTurn,
    setEditTarget,
    clearEditTarget,
    players,
    mode,
    isIdle,
    currentPlayerId,
    currentTurn,
    turns,
    winnerId,
    winner,
    connected,
    playerName,
  }
})
