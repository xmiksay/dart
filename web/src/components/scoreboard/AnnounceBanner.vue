<script setup lang="ts">
defineProps<{
  kind: 'win' | 'bust'
  text: string
}>()
</script>

<template>
  <div
    class="pointer-events-none absolute inset-0 z-20 flex items-center justify-center"
  >
    <div
      class="animate-pop-in rounded-3xl px-16 py-10 text-center shadow-2xl"
      :class="kind === 'win' ? 'bg-accent-win' : 'bg-accent-bust'"
    >
      <p class="text-board-bg text-2xl font-bold uppercase tracking-widest">
        {{ kind === 'win' ? 'Winner' : 'Bust' }}
      </p>
      <p class="text-board-bg text-7xl font-black">{{ text }}</p>
    </div>
  </div>
</template>
