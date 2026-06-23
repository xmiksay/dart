<script setup lang="ts">
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
    <GameConfig />
  </div>
</template>
