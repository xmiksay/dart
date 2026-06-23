<script setup lang="ts">
import { computed } from 'vue'
import { useGameStore } from '@/stores/game'
import { dartLabel } from '@/utils/dart'
import DartSlots from './DartSlots.vue'

const game = useGameStore()

const x01 = computed(() => game.state.x01)
const checkoutLabel = computed(() =>
  x01.value?.checkout?.map(dartLabel).join('  ') ?? null,
)

function remaining(id: number): number {
  return x01.value?.remaining[String(id)] ?? 0
}
</script>

<template>
  <div
    class="grid gap-5 p-6"
    :class="game.players.length > 2 ? 'lg:grid-cols-2' : 'grid-cols-1'"
  >
    <div
      v-for="p in game.players"
      :key="p.id"
      class="relative overflow-hidden rounded-3xl border-2 p-6 transition-all duration-300"
      :class="
        p.id === game.currentPlayerId
          ? 'border-accent-turn bg-board-panel shadow-[0_0_40px_-8px] shadow-accent-turn'
          : 'border-board-line bg-board-panel/60'
      "
    >
      <div class="flex items-center justify-between">
        <h3
          class="text-4xl font-black"
          :class="p.id === game.currentPlayerId ? 'text-accent-turn' : 'text-slate-200'"
        >
          {{ p.name }}
        </h3>
        <span
          v-if="p.id === game.currentPlayerId"
          class="rounded-full bg-accent-turn px-4 py-1 text-lg font-bold uppercase text-board-bg"
        >
          To throw
        </span>
      </div>

      <div class="my-2 text-right">
        <span class="font-score text-mega leading-none tracking-tighter">
          {{ remaining(p.id) }}
        </span>
      </div>

      <div
        v-if="p.id === game.currentPlayerId"
        class="mt-4 flex flex-col gap-4 border-t border-board-line pt-4"
      >
        <DartSlots :darts="game.currentTurn" :tick="game.dartTick" />
        <div
          v-if="checkoutLabel"
          class="rounded-xl bg-accent/10 px-4 py-3 text-2xl"
        >
          <span class="font-semibold uppercase text-accent">Checkout</span>
          <span class="ml-3 font-black tracking-wide text-accent">
            {{ checkoutLabel }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>
