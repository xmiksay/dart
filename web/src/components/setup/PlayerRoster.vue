<script setup lang="ts">
import { ref } from 'vue'
import { useGameStore } from '@/stores/game'
import { usePlayersStore } from '@/stores/players'
import type { PlayerId } from '@/api/protocol'

const game = useGameStore()
const players = usePlayersStore()
const newName = ref('')
const editingId = ref<PlayerId | null>(null)
const editName = ref('')

function add() {
  const name = newName.value.trim()
  if (!name) return
  game.addPlayer(name)
  // Persist the name so it shows up in the saved-players picker next time.
  // A duplicate (409) is handled gracefully inside the store.
  players.create(name)
  newName.value = ''
}

function startEdit(id: PlayerId, name: string) {
  editingId.value = id
  editName.value = name
}

function commitEdit() {
  if (editingId.value === null) return
  const name = editName.value.trim()
  if (name) game.renamePlayer(editingId.value, name)
  editingId.value = null
}

function move(id: PlayerId, from: number, delta: number) {
  const to = from + delta
  if (to < 0 || to >= game.players.length) return
  game.reorderPlayer(id, to)
}
</script>

<template>
  <section class="rounded-2xl bg-board-panel p-6">
    <h2 class="mb-4 text-2xl font-bold text-slate-100">Players</h2>

    <ul class="mb-4 space-y-2">
      <li
        v-for="(p, i) in game.players"
        :key="p.id"
        class="flex items-center gap-3 rounded-xl bg-board-bg px-4 py-3"
      >
        <span class="w-8 text-center text-xl font-bold text-slate-500">
          {{ i + 1 }}
        </span>

        <template v-if="editingId === p.id">
          <input
            v-model="editName"
            class="flex-1 rounded-lg bg-board-line px-3 py-2 text-xl text-slate-100 outline-none focus:ring-2 focus:ring-accent"
            @keyup.enter="commitEdit"
            @keyup.escape="editingId = null"
          />
          <button
            class="rounded-lg bg-accent px-4 py-2 font-semibold text-board-bg"
            @click="commitEdit"
          >
            Save
          </button>
        </template>

        <template v-else>
          <span class="flex-1 text-xl font-semibold">{{ p.name }}</span>
          <button
            class="rounded-lg px-3 py-1 text-slate-400 hover:bg-board-line disabled:opacity-30"
            :disabled="i === 0"
            title="Move up"
            @click="move(p.id, i, -1)"
          >
            ▲
          </button>
          <button
            class="rounded-lg px-3 py-1 text-slate-400 hover:bg-board-line disabled:opacity-30"
            :disabled="i === game.players.length - 1"
            title="Move down"
            @click="move(p.id, i, 1)"
          >
            ▼
          </button>
          <button
            class="rounded-lg px-3 py-1 text-slate-300 hover:bg-board-line"
            @click="startEdit(p.id, p.name)"
          >
            Rename
          </button>
          <button
            class="rounded-lg px-3 py-1 text-accent-bust hover:bg-board-line"
            @click="game.removePlayer(p.id)"
          >
            Remove
          </button>
        </template>
      </li>
      <li v-if="!game.players.length" class="px-4 py-3 text-slate-500">
        No players yet — add someone below.
      </li>
    </ul>

    <div class="flex gap-3">
      <input
        v-model="newName"
        placeholder="Add player…"
        class="flex-1 rounded-lg bg-board-bg px-4 py-3 text-xl text-slate-100 outline-none focus:ring-2 focus:ring-accent"
        @keyup.enter="add"
      />
      <button
        class="rounded-lg bg-accent px-6 py-3 text-xl font-bold text-board-bg hover:brightness-110"
        @click="add"
      >
        Add
      </button>
    </div>
  </section>
</template>
