<script setup lang="ts">
import { ref } from 'vue'
import type { Dart } from '@/api/protocol'
import {
  type Multiplier,
  makeDart,
  outerBull,
  innerBull,
  miss,
  BOARD_NUMBERS,
} from '@/utils/dart'

const emit = defineEmits<{ pick: [dart: Dart] }>()

const multiplier = ref<Multiplier>('Single')
const multipliers: Multiplier[] = ['Single', 'Double', 'Treble']
const mLabel: Record<Multiplier, string> = {
  Single: 'Single',
  Double: 'Double',
  Treble: 'Treble',
}

function pickSegment(n: number) {
  emit('pick', makeDart(n, multiplier.value))
}
</script>

<template>
  <div class="space-y-4">
    <div class="grid grid-cols-3 gap-2">
      <button
        v-for="m in multipliers"
        :key="m"
        class="rounded-lg py-3 text-xl font-bold"
        :class="multiplier === m ? 'bg-accent text-board-bg' : 'bg-board-bg'"
        @click="multiplier = m"
      >
        {{ mLabel[m] }}
      </button>
    </div>

    <div class="grid grid-cols-5 gap-2">
      <button
        v-for="n in BOARD_NUMBERS"
        :key="n"
        class="rounded-lg bg-board-bg py-3 text-2xl font-bold hover:bg-board-line"
        @click="pickSegment(n)"
      >
        {{ n }}
      </button>
    </div>

    <div class="grid grid-cols-3 gap-2">
      <button
        class="rounded-lg bg-board-bg py-3 text-xl font-bold hover:bg-board-line"
        @click="emit('pick', outerBull())"
      >
        25
      </button>
      <button
        class="rounded-lg bg-board-bg py-3 text-xl font-bold hover:bg-board-line"
        @click="emit('pick', innerBull())"
      >
        Bull 50
      </button>
      <button
        class="rounded-lg bg-board-bg py-3 text-xl font-bold text-slate-400 hover:bg-board-line"
        @click="emit('pick', miss())"
      >
        Miss
      </button>
    </div>
  </div>
</template>
