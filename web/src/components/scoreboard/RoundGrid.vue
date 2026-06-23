<script setup lang="ts">
import { computed } from 'vue'
import { useGameStore } from '@/stores/game'
import { dartLabel } from '@/utils/dart'
import type { Turn } from '@/api/protocol'

/**
 * Unified round-grid: columns are players (roster order), rows are rounds.
 * Each cell holds a player's three darts for that round and stays editable.
 *
 * Cell mapping (n = player count): the engine appends one turn per player per
 * round in roster order, so the flat `turns` array index for (round r, column
 * c) is `r * n + c`. That same index is the `turn_index` for `edit_dart`; the
 * dart's position in the cell is its `dart_index`.
 */
const game = useGameStore()

const players = computed(() => game.players)
const n = computed(() => players.value.length)

// Round count = ceil(turns / players); the last round may be partially filled.
const roundCount = computed(() =>
  n.value === 0 ? 0 : Math.ceil(game.turns.length / n.value),
)
const rounds = computed(() => Array.from({ length: roundCount.value }, (_, r) => r))

// The in-progress cell is the last `turns` entry while the leg is live.
const liveIndex = computed(() =>
  game.winnerId === null && game.currentPlayerId !== null
    ? game.turns.length - 1
    : -1,
)

function turnIndex(round: number, col: number): number {
  return round * n.value + col
}

function cellTurn(round: number, col: number): Turn | undefined {
  return game.turns[turnIndex(round, col)]
}

function turnTotal(turn: Turn): number {
  return turn.darts.reduce((sum, d) => sum + d.value, 0)
}

function isCurrentColumn(col: number): boolean {
  return players.value[col]?.id === game.currentPlayerId
}

function isLiveCell(round: number, col: number): boolean {
  return turnIndex(round, col) === liveIndex.value
}

function isEditing(round: number, col: number, dartIndex: number): boolean {
  const t = game.editTarget
  return (
    t !== null && t.turnIndex === turnIndex(round, col) && t.dartIndex === dartIndex
  )
}

function selectDart(round: number, col: number, dartIndex: number) {
  if (isEditing(round, col, dartIndex)) game.clearEditTarget()
  else game.setEditTarget(turnIndex(round, col), dartIndex)
}
</script>

<template>
  <div class="overflow-hidden rounded-2xl border border-board-line">
    <table class="w-full table-fixed border-collapse">
      <colgroup>
        <col class="w-16" />
        <col v-for="p in players" :key="p.id" />
      </colgroup>

      <!-- Per-column headers live in the parent (name + key number). -->
      <thead>
        <tr>
          <th class="bg-board-panel" />
          <th
            v-for="(p, col) in players"
            :key="p.id"
            class="border-l border-board-line bg-board-panel p-0 align-top"
            :class="isCurrentColumn(col) ? 'ring-2 ring-inset ring-accent-turn' : ''"
          >
            <slot name="header" :player="p" :col="col" :active="isCurrentColumn(col)" />
          </th>
        </tr>
      </thead>

      <tbody>
        <tr
          v-for="round in rounds"
          :key="round"
          class="border-t border-board-line"
        >
          <td
            class="bg-board-panel/60 text-center font-score text-3xl font-bold text-slate-500"
          >
            R{{ round + 1 }}
          </td>
          <td
            v-for="(p, col) in players"
            :key="p.id"
            class="border-l border-board-line p-1 align-middle"
            :class="[
              isCurrentColumn(col) ? 'bg-board-panel/40' : '',
              isLiveCell(round, col) ? 'bg-accent-turn/10' : '',
            ]"
          >
            <div v-if="cellTurn(round, col)" class="flex items-center gap-1">
              <button
                v-for="(d, dartIndex) in cellTurn(round, col)!.darts"
                :key="dartIndex"
                class="min-w-0 flex-1 rounded border py-1 text-center text-5xl font-black leading-tight transition-colors"
                :class="
                  isEditing(round, col, dartIndex)
                    ? 'border-accent-turn bg-accent-turn/25 text-accent-turn'
                    : 'border-board-line bg-board-bg/60 text-slate-100 hover:border-accent'
                "
                @click="selectDart(round, col, dartIndex)"
              >
                {{ dartLabel(d) }}
              </button>
              <span
                class="w-14 shrink-0 text-right font-score text-3xl font-black text-accent-turn"
              >
                {{ turnTotal(cellTurn(round, col)!) }}
              </span>
            </div>
          </td>
        </tr>

        <tr v-if="roundCount === 0">
          <td
            :colspan="n + 1"
            class="py-6 text-center text-base text-slate-600"
          >
            No throws yet
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
