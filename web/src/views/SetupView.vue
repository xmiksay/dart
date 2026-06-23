<script setup lang="ts">
import { RouterLink } from 'vue-router'
import PlayerRoster from '@/components/setup/PlayerRoster.vue'
import SavedPlayers from '@/components/setup/SavedPlayers.vue'
import GameConfig from '@/components/setup/GameConfig.vue'
</script>

<template>
  <div class="mx-auto grid max-w-6xl gap-6 p-6 lg:grid-cols-2">
    <div class="space-y-6">
      <PlayerRoster />
      <SavedPlayers />
    </div>
    <div class="space-y-6">
      <GameConfig />
      <RouterLink
        to="/calibrate"
        class="flex items-center justify-between rounded-2xl bg-board-panel p-6 transition-colors hover:bg-board-line"
      >
        <span>
          <span class="block text-2xl font-bold text-slate-100">Camera calibration</span>
          <span class="block text-slate-400">Mark the four board points for the fixed camera.</span>
        </span>
        <span class="text-2xl font-black text-accent">→</span>
      </RouterLink>
    </div>
  </div>
</template>
