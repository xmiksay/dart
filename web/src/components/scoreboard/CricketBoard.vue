<script setup lang="ts">
import { computed } from 'vue'
import { useGameStore } from '@/stores/game'
import type { GameState } from '@/api/protocol'
import RoundGrid from './RoundGrid.vue'

// `source` provided => render that finished game; omitted => live game store.
const props = defineProps<{ source?: GameState | null }>()
const game = useGameStore()

// Marks order left-to-right; 25 is the bull.
const TARGETS = [20, 19, 18, 17, 16, 15, 25] as const

const view = computed(() => props.source ?? game.state)
const cricket = computed(() => view.value.cricket)

function targetLabel(n: number): string {
  return n === 25 ? 'B' : String(n)
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
  <div class="flex h-full flex-col p-4">
    <RoundGrid :source="props.source ?? null">
      <template #header="{ player, active }">
        <div class="px-2 py-2">
          <div class="flex items-baseline justify-between gap-2">
            <p
              class="truncate text-2xl font-black leading-tight"
              :class="active ? 'text-accent-turn' : 'text-slate-200'"
            >
              {{ player.name }}
            </p>
            <p
              class="font-score text-4xl font-black leading-none"
              :class="active ? 'text-accent-turn' : 'text-slate-100'"
            >
              {{ points(player.id) }}
            </p>
          </div>

          <div class="mt-1 grid grid-cols-7 gap-0.5 text-center">
            <div
              v-for="t in TARGETS"
              :key="t"
              class="rounded bg-board-bg/60 py-0.5"
            >
              <p class="text-[0.6rem] font-bold leading-none text-slate-500">
                {{ targetLabel(t) }}
              </p>
              <p
                class="text-lg font-black leading-none"
                :class="marks(player.id, t) >= 3 ? 'text-accent-win' : 'text-accent'"
              >
                {{ markGlyph(marks(player.id, t)) || '·' }}
              </p>
            </div>
          </div>
        </div>
      </template>
    </RoundGrid>
  </div>
</template>
