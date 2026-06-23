<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useGameStore } from '@/stores/game'
import type {
  GameMode,
  InRule,
  OutRule,
  X01Start,
} from '@/api/protocol'

const game = useGameStore()
const router = useRouter()

const mode = ref<Exclude<GameMode, 'idle'>>('x01')
const start = ref<X01Start>(501)
const inRule = ref<InRule>('Straight')
const outRule = ref<OutRule>('Double')
const cutThroat = ref(false)

const starts: X01Start[] = [301, 501, 701, 901]
const inRules: InRule[] = ['Straight', 'Double']
const outRules: OutRule[] = ['Straight', 'Double', 'Triple', 'Master']

function startGame() {
  if (mode.value === 'x01') {
    game.newGame({
      mode: 'x01',
      x01: { start: start.value, in_rule: inRule.value, out_rule: outRule.value },
    })
  } else {
    game.newGame({ mode: 'cricket', cricket: { cut_throat: cutThroat.value } })
  }
  router.push('/')
}
</script>

<template>
  <section class="rounded-2xl bg-board-panel p-6">
    <h2 class="mb-4 text-2xl font-bold">Game</h2>

    <div class="mb-6 grid grid-cols-2 gap-3">
      <button
        v-for="m in (['x01', 'cricket'] as const)"
        :key="m"
        class="rounded-xl py-4 text-2xl font-bold uppercase transition-colors"
        :class="
          mode === m ? 'bg-accent text-board-bg' : 'bg-board-bg text-slate-300'
        "
        @click="mode = m"
      >
        {{ m === 'x01' ? 'X01' : 'Cricket' }}
      </button>
    </div>

    <div v-if="mode === 'x01'" class="space-y-5">
      <div>
        <p class="mb-2 text-lg font-semibold text-slate-400">Start score</p>
        <div class="grid grid-cols-4 gap-2">
          <button
            v-for="s in starts"
            :key="s"
            class="rounded-lg py-3 text-xl font-bold"
            :class="start === s ? 'bg-accent text-board-bg' : 'bg-board-bg'"
            @click="start = s"
          >
            {{ s }}
          </button>
        </div>
      </div>

      <div>
        <p class="mb-2 text-lg font-semibold text-slate-400">In</p>
        <div class="grid grid-cols-2 gap-2">
          <button
            v-for="r in inRules"
            :key="r"
            class="rounded-lg py-3 text-xl font-bold"
            :class="inRule === r ? 'bg-accent text-board-bg' : 'bg-board-bg'"
            @click="inRule = r"
          >
            {{ r }}
          </button>
        </div>
      </div>

      <div>
        <p class="mb-2 text-lg font-semibold text-slate-400">Out</p>
        <div class="grid grid-cols-4 gap-2">
          <button
            v-for="r in outRules"
            :key="r"
            class="rounded-lg py-3 text-xl font-bold"
            :class="outRule === r ? 'bg-accent text-board-bg' : 'bg-board-bg'"
            @click="outRule = r"
          >
            {{ r }}
          </button>
        </div>
      </div>
    </div>

    <div v-else class="space-y-5">
      <label class="flex cursor-pointer items-center justify-between rounded-xl bg-board-bg px-4 py-4">
        <span class="text-xl font-semibold">Cut-throat</span>
        <input v-model="cutThroat" type="checkbox" class="h-6 w-6 accent-cyan-400" />
      </label>
      <p class="text-slate-500">
        In cut-throat, points are added to opponents — lowest score wins.
      </p>
    </div>

    <button
      class="mt-8 w-full rounded-xl bg-accent-win py-5 text-3xl font-black text-board-bg transition disabled:opacity-40 hover:brightness-110"
      :disabled="!game.players.length"
      @click="startGame"
    >
      Start game
    </button>
    <p v-if="!game.players.length" class="mt-2 text-center text-accent-bust">
      Add at least one player first.
    </p>
  </section>
</template>
