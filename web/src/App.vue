<script setup lang="ts">
import { onMounted, onBeforeUnmount } from 'vue'
import { RouterLink, RouterView, useRoute } from 'vue-router'
import { useGameStore } from '@/stores/game'

const game = useGameStore()
const route = useRoute()

onMounted(() => game.connect())
onBeforeUnmount(() => game.disconnect())
</script>

<template>
  <div class="flex h-full flex-col bg-board-bg text-slate-100">
    <header
      class="flex items-center justify-between border-b border-board-line bg-board-panel px-6 py-3"
    >
      <div class="flex items-center gap-3">
        <span class="text-2xl font-black tracking-tight text-accent">
          dartscore
        </span>
        <span
          class="h-3 w-3 rounded-full"
          :class="game.connected ? 'bg-accent-win' : 'bg-accent-bust animate-flash'"
          :title="game.connected ? 'connected' : 'disconnected'"
        />
      </div>
      <nav class="flex gap-2 text-lg font-semibold">
        <RouterLink
          v-for="link in [
            { to: '/', label: 'Scoreboard' },
            { to: '/setup', label: 'Setup' },
            { to: '/history', label: 'History' },
            { to: '/calibrate', label: 'Calibrate' },
          ]"
          :key="link.to"
          :to="link.to"
          class="rounded-lg px-4 py-1.5 transition-colors"
          :class="
            route.path === link.to
              ? 'bg-accent text-board-bg'
              : 'text-slate-300 hover:bg-board-line'
          "
        >
          {{ link.label }}
        </RouterLink>
      </nav>
    </header>

    <main class="min-h-0 flex-1 overflow-auto">
      <RouterView />
    </main>
  </div>
</template>
