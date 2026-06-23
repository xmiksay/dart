<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { usePlayersStore } from '@/stores/players'
import { useGameStore } from '@/stores/game'
import type { SavedPlayer } from '@/api/protocol'

const players = usePlayersStore()
const game = useGameStore()

const managing = ref(false)
const editingId = ref<string | null>(null)
const editName = ref('')

onMounted(() => players.load())

const rosterNames = computed(() => new Set(game.players.map((p) => p.name)))

function inRoster(p: SavedPlayer): boolean {
  return rosterNames.value.has(p.name)
}

function pick(p: SavedPlayer) {
  if (managing.value || inRoster(p)) return
  game.addPlayer(p.name)
}

function startEdit(p: SavedPlayer) {
  editingId.value = p.id
  editName.value = p.name
}

function commitEdit() {
  if (editingId.value === null) return
  const name = editName.value.trim()
  if (name) players.rename(editingId.value, name)
  editingId.value = null
}
</script>

<template>
  <section class="rounded-2xl bg-board-panel p-6">
    <div class="mb-4 flex items-center justify-between">
      <h2 class="text-2xl font-bold text-slate-100">Saved players</h2>
      <button
        class="rounded-lg px-3 py-1 text-sm font-semibold"
        :class="managing ? 'bg-accent text-board-bg' : 'bg-board-bg text-slate-300'"
        @click="managing = !managing"
      >
        {{ managing ? 'Done' : 'Manage' }}
      </button>
    </div>

    <p v-if="players.loading" class="px-4 py-3 text-slate-500">Loading…</p>
    <p v-else-if="players.error" class="px-4 py-3 text-accent-bust">
      {{ players.error }}
    </p>

    <ul v-else class="space-y-2">
      <li
        v-for="p in players.players"
        :key="p.id"
        class="flex items-center gap-3 rounded-xl bg-board-bg px-4 py-3"
      >
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
          <button
            class="flex flex-1 items-center gap-3 text-left disabled:cursor-default"
            :disabled="managing || inRoster(p)"
            @click="pick(p)"
          >
            <span
              class="flex h-7 w-7 items-center justify-center rounded-md border-2 text-board-bg"
              :class="
                inRoster(p)
                  ? 'border-accent-win bg-accent-win'
                  : 'border-board-line'
              "
            >
              <span v-if="inRoster(p)" class="text-base font-bold">✓</span>
            </span>
            <span class="text-xl font-semibold text-slate-100">{{ p.name }}</span>
          </button>

          <template v-if="managing">
            <button
              class="rounded-lg px-3 py-1 text-slate-300 hover:bg-board-line"
              @click="startEdit(p)"
            >
              Rename
            </button>
            <button
              class="rounded-lg px-3 py-1 text-accent-bust hover:bg-board-line"
              @click="players.remove(p.id)"
            >
              Delete
            </button>
          </template>
          <span v-else-if="inRoster(p)" class="text-sm font-semibold text-accent-win">
            Added
          </span>
        </template>
      </li>

      <li v-if="!players.players.length" class="px-4 py-3 text-slate-500">
        No saved players yet — names you add are remembered here.
      </li>
    </ul>
  </section>
</template>
