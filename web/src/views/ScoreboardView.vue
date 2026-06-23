<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink } from 'vue-router'
import { useGameStore } from '@/stores/game'
import X01Board from '@/components/scoreboard/X01Board.vue'
import CricketBoard from '@/components/scoreboard/CricketBoard.vue'
import AnnounceBanner from '@/components/scoreboard/AnnounceBanner.vue'
import CorrectionPanel from '@/components/correction/CorrectionPanel.vue'

const game = useGameStore()
const showOperator = ref(true)

// Win banner from winner_id; bust banner inferred from the server error event.
const banner = computed<{ kind: 'win' | 'bust'; text: string } | null>(() => {
  if (game.winner) return { kind: 'win', text: game.winner.name }
  if (game.lastError && /bust/i.test(game.lastError)) {
    return { kind: 'bust', text: game.playerName(game.currentPlayerId) }
  }
  return null
})
</script>

<template>
  <div class="relative flex h-full">
    <section class="relative min-h-0 flex-1 overflow-auto">
      <AnnounceBanner v-if="banner" :kind="banner.kind" :text="banner.text" />

      <X01Board v-if="game.mode === 'x01'" />
      <CricketBoard v-else-if="game.mode === 'cricket'" />

      <div
        v-else
        class="flex h-full flex-col items-center justify-center gap-6 text-center"
      >
        <p class="text-5xl font-black text-slate-300">No game in progress</p>
        <RouterLink
          to="/setup"
          class="rounded-xl bg-accent px-8 py-4 text-2xl font-bold text-board-bg"
        >
          Set up a game
        </RouterLink>
      </div>

      <button
        v-if="game.mode !== 'idle'"
        class="absolute bottom-4 right-4 z-10 rounded-full bg-board-panel px-5 py-2 text-lg font-semibold text-slate-300 shadow-lg"
        @click="showOperator = !showOperator"
      >
        {{ showOperator ? 'Hide operator' : 'Show operator' }}
      </button>
    </section>

    <CorrectionPanel v-if="showOperator && game.mode !== 'idle'" />
  </div>
</template>
