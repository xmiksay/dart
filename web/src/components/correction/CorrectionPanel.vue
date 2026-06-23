<script setup lang="ts">
import { computed } from 'vue'
import { useGameStore } from '@/stores/game'
import type { Dart } from '@/api/protocol'
import { dartLabel } from '@/utils/dart'
import DartPicker from './DartPicker.vue'

const game = useGameStore()

// `turns` index of the in-progress turn, or -1 when none is live.
const liveTurnIndex = computed(() =>
  game.winnerId === null && game.currentPlayerId !== null
    ? game.turns.length - 1
    : -1,
)

// Is the current edit target a dart in the live turn?
function isLiveEditing(dartIndex: number): boolean {
  const t = game.editTarget
  return t !== null && t.turnIndex === liveTurnIndex.value && t.dartIndex === dartIndex
}

const editLabel = computed(() => {
  const t = game.editTarget
  if (t === null) return 'Add dart'
  const player = game.turns[t.turnIndex]?.player ?? null
  const who = player === null ? '' : ` (${game.playerName(player)})`
  return `Editing dart ${t.dartIndex + 1}${who}`
})

function toggleLive(dartIndex: number) {
  if (isLiveEditing(dartIndex)) game.clearEditTarget()
  else game.setEditTarget(liveTurnIndex.value, dartIndex)
}

function onPick(dart: Dart) {
  const t = game.editTarget
  if (t === null) {
    game.throwDart(dart)
  } else {
    game.editDart(t.turnIndex, t.dartIndex, dart)
    game.clearEditTarget()
  }
}
</script>

<template>
  <aside
    class="flex h-full w-[26rem] flex-col gap-4 border-l border-board-line bg-board-panel p-5"
  >
    <header class="flex items-center justify-between">
      <h2 class="text-2xl font-bold">Operator</h2>
      <span class="text-sm text-slate-500">manual correction</span>
    </header>

    <p
      v-if="game.lastError"
      class="cursor-pointer rounded-lg bg-accent-bust/20 px-4 py-3 text-accent-bust"
      @click="game.clearError()"
    >
      {{ game.lastError }} <span class="opacity-60">(dismiss)</span>
    </p>

    <!-- Current turn: tap a dart to edit it (or tap any dart in the leg log). -->
    <div v-if="liveTurnIndex >= 0">
      <p class="mb-2 text-sm uppercase text-slate-500">Current turn</p>
      <div class="flex gap-2">
        <button
          v-for="(d, i) in game.currentTurn"
          :key="i"
          class="flex-1 rounded-lg border-2 py-3 text-xl font-bold transition-colors"
          :class="
            isLiveEditing(i)
              ? 'border-accent-turn bg-board-bg text-accent-turn'
              : 'border-board-line bg-board-bg'
          "
          @click="toggleLive(i)"
        >
          {{ dartLabel(d) }}
        </button>
        <span
          v-if="!game.currentTurn.length"
          class="flex-1 rounded-lg border-2 border-dashed border-board-line py-3 text-center text-slate-600"
        >
          no darts yet
        </span>
      </div>
    </div>

    <p class="text-sm text-slate-500">
      Tap any dart in the leg history to edit it.
    </p>

    <p
      class="rounded-lg px-3 py-2 text-center text-lg font-semibold"
      :class="
        game.editTarget === null
          ? 'bg-accent/15 text-accent'
          : 'bg-accent-turn/15 text-accent-turn'
      "
    >
      {{ editLabel }}
    </p>

    <DartPicker @pick="onPick" />

    <div class="mt-auto grid grid-cols-2 gap-3 pt-3">
      <button
        class="rounded-lg bg-board-bg py-3 text-lg font-bold hover:bg-board-line"
        @click="game.undoLast()"
      >
        Undo last
      </button>
      <button
        class="rounded-lg bg-board-bg py-3 text-lg font-bold hover:bg-board-line"
        @click="game.endTurn()"
      >
        End turn
      </button>
      <button
        class="col-span-2 rounded-lg bg-accent-bust/20 py-3 text-lg font-bold text-accent-bust hover:bg-accent-bust/30"
        @click="game.reset()"
      >
        Reset game
      </button>
    </div>
  </aside>
</template>
