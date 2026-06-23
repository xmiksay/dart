<script setup lang="ts">
import { onMounted } from 'vue'
import { useHistoryStore } from '@/stores/history'
import { dartLabel } from '@/utils/dart'

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
  <div class="mx-auto grid max-w-6xl gap-6 p-6 lg:grid-cols-[1fr_1.3fr]">
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
        <li
          v-for="g in history.games"
          :key="g.id"
          class="cursor-pointer rounded-xl bg-board-bg px-4 py-3 transition-colors hover:bg-board-line"
          :class="history.selected?.id === g.id ? 'ring-2 ring-accent' : ''"
          @click="history.loadGame(g.id)"
        >
          <div class="flex items-center justify-between">
            <span class="text-xl font-bold">{{ modeLabel(g.mode) }}</span>
            <span class="text-sm text-slate-500">{{ fmtDate(g.finished_at) }}</span>
          </div>
          <div class="mt-1 flex items-center justify-between text-slate-300">
            <span>{{ g.players.map((p) => p.name).join(', ') }}</span>
            <span v-if="g.winner" class="font-bold text-accent-win">
              🏆 {{ g.winner.name }}
            </span>
          </div>
        </li>
      </ul>
    </section>

    <section class="rounded-2xl bg-board-panel p-6">
      <h2 class="mb-4 text-2xl font-bold">Game detail</h2>
      <p v-if="!history.selected" class="text-slate-500">
        Select a game to see its turns.
      </p>
      <div v-else class="space-y-4">
        <div class="flex items-center justify-between">
          <span class="text-xl font-bold">
            {{ modeLabel(history.selected.mode) }} ·
            {{ fmtDate(history.selected.finished_at) }}
          </span>
          <span v-if="history.selected.winner" class="font-bold text-accent-win">
            🏆 {{ history.selected.winner.name }}
          </span>
        </div>

        <ol class="space-y-2">
          <li
            v-for="(turn, i) in history.selected.turns"
            :key="i"
            class="flex items-center gap-4 rounded-lg bg-board-bg px-4 py-2"
          >
            <span class="w-32 truncate font-semibold text-slate-300">
              {{
                history.selected.players.find((p) => p.id === turn.player_id)
                  ?.name ?? `#${turn.player_id}`
              }}
            </span>
            <span class="flex gap-2">
              <span
                v-for="(d, j) in turn.darts"
                :key="j"
                class="rounded bg-board-line px-2 py-0.5 font-mono text-lg"
              >
                {{ dartLabel(d) }}
              </span>
            </span>
          </li>
        </ol>
      </div>
    </section>
  </div>
</template>
