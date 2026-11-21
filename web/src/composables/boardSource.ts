import { computed } from 'vue'
import { useGameStore, type EditTarget } from '@/stores/game'
import type { GameState } from '@/api/protocol'

/**
 * Resolves what a scoreboard renders from. When `getStatic` yields a snapshot
 * (history view), the board is read-only and bound to it; otherwise it follows
 * the live game store and stays editable. This lets the live and historical
 * boards share one set of components.
 */
export function useBoardSource(getStatic: () => GameState | null | undefined) {
  const game = useGameStore()
  const isLive = computed(() => getStatic() == null)
  const state = computed<GameState>(() => getStatic() ?? game.state)
  const editTarget = computed<EditTarget | null>(() =>
    isLive.value ? game.editTarget : null,
  )

  function selectDart(turnIndex: number, dartIndex: number) {
    if (!isLive.value) return
    const t = game.editTarget
    if (t && t.turnIndex === turnIndex && t.dartIndex === dartIndex)
      game.clearEditTarget()
    else game.setEditTarget(turnIndex, dartIndex)
  }

  return { isLive, state, editTarget, selectDart }
}
