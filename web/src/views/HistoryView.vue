<script setup lang="ts">
import { onMounted } from 'vue'
import { RouterLink } from 'vue-router'
import { useHistoryStore } from '@/stores/history'

const history = useHistoryStore()

onMounted(() => history.loadGames())

function fmtDate(iso: string): string {
  const d = new Date(iso)
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString()
}

function modeLabel(mode: string): string {
  return mode === 'x01' ? 'X01' : mode === 'cricket' ? 'Cricket' : mode
}
</script>

<template>
  <div class="mx-auto max-w-3xl p-6">
    <section class="rounded-2xl bg-board-panel p-6">
      <div class="mb-4 flex items-center justify-between">
        <h2 class="text-2xl font-bold">Finished games</h2>
        <button
          class="rounded-lg bg-board-bg px-4 py-2 font-semibold hover:bg-board-line"
          @click="history.loadGames()"
        >
          Refresh
        </button>
      </div>

      <p v-if="history.loading" class="text-slate-500">Loading…</p>
      <p v-else-if="history.error" class="text-accent-bust">{{ history.error }}</p>
      <p v-else-if="!history.games.length" class="text-slate-500">
        No finished games yet.
      </p>

      <ul v-else class="space-y-2">
        <RouterLink
          v-for="g in history.games"
          :key="g.id"
          :to="`/history/${g.id}`"
          custom
          v-slot="{ navigate }"
        >
          <li
            class="cursor-pointer rounded-xl bg-board-bg px-4 py-3 transition-colors hover:bg-board-line"
            @click="navigate"
          >
            <div class="flex items-center justify-between">
              <span class="text-xl font-bold">{{ modeLabel(g.mode) }}</span>
              <span class="text-sm text-slate-500">{{ fmtDate(g.finished_at) }}</span>
            </div>
            <div class="mt-1 flex items-center justify-between text-slate-300">
              <span>{{ g.players.join(', ') }}</span>
              <span v-if="g.winner" class="font-bold text-accent-win">
                🏆 {{ g.winner }}
              </span>
            </div>
          </li>
        </RouterLink>
      </ul>
    </section>
  </div>
</template>
