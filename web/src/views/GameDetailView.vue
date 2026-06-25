<script setup lang="ts">
import { computed, onMounted, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useHistoryStore } from '@/stores/history'
import X01Board from '@/components/scoreboard/X01Board.vue'
import CricketBoard from '@/components/scoreboard/CricketBoard.vue'

const route = useRoute()
const history = useHistoryStore()

const id = computed(() => String(route.params.id))
const board = computed(() => history.board)
const winner = computed(() => {
  const b = history.board
  if (!b || b.winner_id === null) return null
  return b.players.find((p) => p.id === b.winner_id)?.name ?? null
})

function load() {
  history.loadBoard(id.value)
}
onMounted(load)
watch(id, load)
</script>

<template>
  <div class="flex h-full flex-col">
    <div
      class="flex items-center justify-between border-b border-board-line px-6 py-3"
    >
      <RouterLink
        to="/history"
        class="rounded-lg bg-board-panel px-4 py-2 font-semibold text-slate-300 hover:bg-board-line"
      >
        ← Back to games
      </RouterLink>
      <span
        v-if="winner"
        class="text-xl font-bold text-accent-win"
      >
        🏆 {{ winner }}
      </span>
    </div>

    <p v-if="history.loading" class="p-6 text-slate-500">Loading…</p>
    <p v-else-if="history.error" class="p-6 text-accent-bust">
      {{ history.error }}
    </p>

    <div v-else-if="board" class="min-h-0 flex-1 overflow-auto">
      <X01Board v-if="board.mode === 'x01'" :source="board" />
      <CricketBoard v-else-if="board.mode === 'cricket'" :source="board" />
      <p v-else class="p-6 text-slate-500">This game has no board to show.</p>
    </div>
  </div>
</template>
