<script setup lang="ts">
import { onMounted, ref, computed } from 'vue'
import { useStatsStore } from '@/stores/stats'
import type { WindowStats } from '@/api/protocol'

const stats = useStatsStore()
const selectedDays = ref(7)

onMounted(() => stats.load())

const current = computed<WindowStats | null>(
  () => stats.windows.find((w) => w.days === selectedDays.value) ?? null,
)

function avg(n: number): string {
  return n.toFixed(2)
}
function pct(n: number): string {
  return `${Math.round(n * 100)}%`
}
function fmtDate(iso: string): string {
  const d = new Date(iso)
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleDateString()
}
</script>

<template>
  <div class="mx-auto max-w-4xl p-6">
    <div class="mb-4 flex items-center justify-between">
      <h2 class="text-2xl font-bold">X01 statistics</h2>
      <button
        class="rounded-lg bg-board-panel px-4 py-2 font-semibold text-slate-300 hover:bg-board-line"
        @click="stats.load()"
      >
        Refresh
      </button>
    </div>

    <!-- Window selector -->
    <div class="mb-6 flex gap-2">
      <button
        v-for="w in stats.windows"
        :key="w.days"
        class="rounded-lg px-4 py-2 font-semibold transition-colors"
        :class="
          w.days === selectedDays
            ? 'bg-accent text-board-bg'
            : 'bg-board-panel text-slate-300 hover:bg-board-line'
        "
        @click="selectedDays = w.days"
      >
        {{ w.days }} days
      </button>
    </div>

    <p v-if="stats.loading" class="text-slate-500">Loading…</p>
    <p v-else-if="stats.error" class="text-accent-bust">{{ stats.error }}</p>

    <div v-else-if="current" class="grid gap-4 md:grid-cols-2">
      <!-- Headline counters -->
      <section class="rounded-2xl bg-board-panel p-6">
        <p class="text-sm font-semibold uppercase tracking-wide text-slate-500">
          Games played
        </p>
        <p class="font-score text-6xl font-black text-slate-100">
          {{ current.games_played }}
        </p>
      </section>

      <section class="rounded-2xl bg-board-panel p-6">
        <p class="text-sm font-semibold uppercase tracking-wide text-slate-500">
          Number of 60s (T20)
        </p>
        <p class="font-score text-6xl font-black text-accent">
          {{ current.sixties }}
        </p>
      </section>

      <!-- Best player -->
      <section class="rounded-2xl bg-board-panel p-6">
        <h3 class="mb-3 text-lg font-bold">Best player (saved)</h3>
        <div v-if="current.best_average_player" class="mb-3">
          <p class="text-xs uppercase tracking-wide text-slate-500">
            Highest 3-dart average
          </p>
          <p class="text-2xl font-black text-slate-100">
            {{ current.best_average_player.name }}
            <span class="font-score text-accent-win">
              {{ avg(current.best_average_player.average) }}
            </span>
          </p>
          <p class="text-sm text-slate-500">
            {{ current.best_average_player.games }} games ·
            {{ current.best_average_player.darts }} darts
          </p>
        </div>
        <div v-if="current.best_winrate_player">
          <p class="text-xs uppercase tracking-wide text-slate-500">
            Highest win rate
          </p>
          <p class="text-2xl font-black text-slate-100">
            {{ current.best_winrate_player.name }}
            <span class="font-score text-accent-win">
              {{ pct(current.best_winrate_player.win_rate) }}
            </span>
          </p>
          <p class="text-sm text-slate-500">
            {{ current.best_winrate_player.wins }}/{{
              current.best_winrate_player.games
            }}
            won
          </p>
        </div>
        <p
          v-if="!current.best_average_player && !current.best_winrate_player"
          class="text-slate-500"
        >
          No saved-player games in this window.
        </p>
      </section>

      <!-- Best game -->
      <section class="rounded-2xl bg-board-panel p-6">
        <h3 class="mb-3 text-lg font-bold">Best game</h3>
        <div v-if="current.best_game_average" class="mb-3">
          <p class="text-xs uppercase tracking-wide text-slate-500">
            Highest single-game average
          </p>
          <p class="text-2xl font-black text-slate-100">
            {{ current.best_game_average.player }}
            <span class="font-score text-accent-win">
              {{ avg(current.best_game_average.average) }}
            </span>
          </p>
          <p class="text-sm text-slate-500">
            {{ current.best_game_average.start }} ·
            {{ fmtDate(current.best_game_average.finished_at) }}
          </p>
        </div>
        <div v-if="current.fewest_darts_game">
          <p class="text-xs uppercase tracking-wide text-slate-500">
            Fewest darts to win
          </p>
          <p class="text-2xl font-black text-slate-100">
            {{ current.fewest_darts_game.player }}
            <span class="font-score text-accent-win">
              {{ current.fewest_darts_game.darts }} darts
            </span>
          </p>
          <p class="text-sm text-slate-500">
            {{ current.fewest_darts_game.start }} ·
            {{ fmtDate(current.fewest_darts_game.finished_at) }}
          </p>
        </div>
        <p
          v-if="!current.best_game_average && !current.fewest_darts_game"
          class="text-slate-500"
        >
          No games in this window.
        </p>
      </section>
    </div>
  </div>
</template>
