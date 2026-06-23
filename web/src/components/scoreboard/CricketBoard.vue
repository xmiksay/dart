<script setup lang="ts">
import { computed } from 'vue'
import { useGameStore } from '@/stores/game'
import DartSlots from './DartSlots.vue'

const game = useGameStore()

// Display order top-to-bottom; 25 is the bull row.
const ROWS = [20, 19, 18, 17, 16, 15, 25] as const

const cricket = computed(() => game.state.cricket)

function rowLabel(n: number): string {
  return n === 25 ? 'Bull' : String(n)
}

function marks(id: number, n: number): number {
  return cricket.value?.marks[String(id)]?.[String(n)] ?? 0
}

function points(id: number): number {
  return cricket.value?.points[String(id)] ?? 0
}

// Mark glyphs: 1 = "/", 2 = "X", 3 = closed (circled X).
function markGlyph(count: number): string {
  return ['', '/', '✕', 'Ⓧ'][Math.min(count, 3)]
}
</script>

<template>
  <div class="p-6">
    <table class="w-full table-fixed border-collapse">
      <thead>
        <tr>
          <th class="w-28 py-3 text-left text-2xl text-slate-500">#</th>
          <th
            v-for="p in game.players"
            :key="p.id"
            class="py-3 text-center text-3xl font-black"
            :class="
              p.id === game.currentPlayerId ? 'text-accent-turn' : 'text-slate-200'
            "
          >
            {{ p.name }}
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="n in ROWS" :key="n" class="border-t border-board-line">
          <td class="py-4 text-3xl font-black text-slate-400">
            {{ rowLabel(n) }}
          </td>
          <td
            v-for="p in game.players"
            :key="p.id"
            class="py-4 text-center"
            :class="
              p.id === game.currentPlayerId ? 'bg-board-panel/60' : ''
            "
          >
            <span
              class="text-5xl font-black"
              :class="marks(p.id, n) >= 3 ? 'text-accent-win' : 'text-accent'"
            >
              {{ markGlyph(marks(p.id, n)) }}
            </span>
          </td>
        </tr>
        <tr class="border-t-2 border-board-line">
          <td class="py-4 text-2xl uppercase text-slate-500">Points</td>
          <td
            v-for="p in game.players"
            :key="p.id"
            class="py-4 text-center"
          >
            <span class="font-score text-6xl font-black">{{ points(p.id) }}</span>
          </td>
        </tr>
      </tbody>
    </table>

    <div class="mt-6 flex items-center justify-between rounded-2xl bg-board-panel p-5">
      <div>
        <p class="text-sm uppercase text-slate-500">Throwing</p>
        <p class="text-3xl font-black text-accent-turn">
          {{ game.playerName(game.currentPlayerId) }}
        </p>
      </div>
      <DartSlots :darts="game.currentTurn" :tick="game.dartTick" />
    </div>
  </div>
</template>
