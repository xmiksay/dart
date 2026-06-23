<script setup lang="ts">
import { ref } from 'vue'
import { useGameStore } from '@/stores/game'
import type { Dart } from '@/api/protocol'
import { dartLabel } from '@/utils/dart'
import DartPicker from './DartPicker.vue'

const game = useGameStore()

// null = manual throw (append); number = editing that dart of the current turn.
const editIndex = ref<number | null>(null)

function onPick(dart: Dart) {
  if (editIndex.value === null) {
    game.throwDart(dart)
  } else {
    game.editDart(editIndex.value, dart)
    editIndex.value = null
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

    <!-- Current turn: tap a dart to edit it. -->
    <div>
      <p class="mb-2 text-sm uppercase text-slate-500">Current turn</p>
      <div class="flex gap-2">
        <button
          v-for="(d, i) in game.currentTurn"
          :key="i"
          class="flex-1 rounded-lg border-2 py-3 text-xl font-bold transition-colors"
          :class="
            editIndex === i
              ? 'border-accent-turn bg-board-bg text-accent-turn'
              : 'border-board-line bg-board-bg'
          "
          @click="editIndex = editIndex === i ? null : i"
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

    <p
      class="rounded-lg px-3 py-2 text-center text-lg font-semibold"
      :class="editIndex === null ? 'bg-accent/15 text-accent' : 'bg-accent-turn/15 text-accent-turn'"
    >
      {{ editIndex === null ? 'Add dart' : `Editing dart ${editIndex + 1}` }}
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
