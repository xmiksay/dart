<script setup lang="ts">
import { computed } from 'vue'
import type { Dart } from '@/api/protocol'
import { dartLabel } from '@/utils/dart'

const props = defineProps<{
  darts: Dart[]
  /** highlight the slot that just changed (drives the pop animation). */
  tick?: number
}>()

// Always render three slots; fill from the current turn.
const slots = computed(() =>
  [0, 1, 2].map((i) => (i < props.darts.length ? props.darts[i] : null)),
)
const turnTotal = computed(() =>
  props.darts.reduce((sum, d) => sum + d.value, 0),
)
</script>

<template>
  <div class="flex items-center gap-3">
    <div
      v-for="(d, i) in slots"
      :key="i"
      class="flex h-20 w-24 flex-col items-center justify-center rounded-xl border-2 text-center transition-colors"
      :class="
        d
          ? 'border-accent bg-board-bg'
          : 'border-dashed border-board-line bg-transparent'
      "
    >
      <span
        v-if="d"
        :key="`${tick}-${i}`"
        class="animate-pop-in text-3xl font-black"
      >
        {{ dartLabel(d) }}
      </span>
      <span v-else class="text-2xl text-board-line">·</span>
      <span v-if="d" class="text-sm text-slate-500">{{ d.value }}</span>
    </div>
    <div class="ml-2 flex flex-col items-center">
      <span class="text-sm uppercase text-slate-500">Turn</span>
      <span class="text-4xl font-black text-accent-turn">{{ turnTotal }}</span>
    </div>
  </div>
</template>
