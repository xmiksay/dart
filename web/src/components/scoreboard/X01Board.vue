<script setup lang="ts">
import { computed } from 'vue'
import { useGameStore } from '@/stores/game'
import { dartLabel } from '@/utils/dart'
import type { GameState } from '@/api/protocol'
import RoundGrid from './RoundGrid.vue'

// `source` provided => render that finished game; omitted => live game store.
const props = defineProps<{ source?: GameState | null }>()
const game = useGameStore()

const view = computed(() => props.source ?? game.state)
const x01 = computed(() => view.value.x01)
const checkoutLabel = computed(() =>
  x01.value?.checkout?.map(dartLabel).join(' ') ?? null,
)

function remaining(id: number): number {
  return x01.value?.remaining[String(id)] ?? 0
}
</script>

<template>
  <div class="flex h-full flex-col p-4">
    <RoundGrid :source="props.source ?? null">
      <template #header="{ player, active }">
        <div class="px-2 py-2 text-center">
          <p
            class="truncate text-2xl font-black leading-tight"
            :class="active ? 'text-accent-turn' : 'text-slate-200'"
          >
            {{ player.name }}
          </p>
          <p
            class="font-score text-6xl font-black leading-none tracking-tighter"
            :class="active ? 'text-accent-turn' : 'text-slate-100'"
          >
            {{ remaining(player.id) }}
          </p>
          <p
            v-if="active && checkoutLabel"
            class="mt-1 truncate text-base font-bold tracking-wide text-accent"
          >
            ✓ {{ checkoutLabel }}
          </p>
        </div>
      </template>
    </RoundGrid>
  </div>
</template>
