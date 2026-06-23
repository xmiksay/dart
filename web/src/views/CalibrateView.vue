<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useCalibrationStore } from '@/stores/calibration'
import type { CalibrationPoints, PixelPoint } from '@/api/protocol'
import CalibrationCanvas from '@/components/calibrate/CalibrationCanvas.vue'

const cal = useCalibrationStore()

// The four board points in canonical order, with operator-facing guidance.
const STEPS = [
  { label: 'top', hint: 'intersection of 5 & 20' },
  { label: 'right', hint: 'intersection of 13 & 6' },
  { label: 'bottom', hint: 'intersection of 17 & 3' },
  { label: 'left', hint: 'intersection of 8 & 11' },
] as const

const selectedCamera = ref(0)
// Points being placed on the freshly captured frame (original-image pixels).
const points = ref<PixelPoint[]>([])
const saved = ref(false)

const accepting = computed(() => points.value.length < STEPS.length)
const complete = computed(() => points.value.length === STEPS.length)
const currentStep = computed(() => STEPS[points.value.length] ?? null)

onMounted(async () => {
  await cal.init()
  if (cal.cameras.length) selectedCamera.value = cal.cameras[0].index
})
onBeforeUnmount(() => cal.clearSnapshot())

async function capture() {
  saved.value = false
  cal.centre = null
  points.value = []
  await cal.capture(selectedCamera.value)
}

function place(p: PixelPoint) {
  if (points.value.length < STEPS.length) points.value.push(p)
}

function undo() {
  points.value.pop()
  saved.value = false
}

function reset() {
  points.value = []
  saved.value = false
  cal.centre = null
}

async function save() {
  if (!complete.value) return
  const quad = [...points.value] as CalibrationPoints
  saved.value = await cal.save(quad)
}
</script>

<template>
  <div class="mx-auto max-w-6xl space-y-6 p-6">
    <header class="space-y-1">
      <h1 class="text-3xl font-black text-accent">Camera calibration</h1>
      <p class="text-slate-400">
        One-time setup for the fixed camera: capture a frame, click the four
        board points in order, then save.
      </p>
    </header>

    <p
      v-if="cal.captureUnavailable"
      class="rounded-xl bg-board-panel px-5 py-4 text-lg text-accent-bust"
    >
      This server was built without camera capture support
      (<code>--features capture</code>), so calibration is unavailable here.
    </p>

    <template v-else>
      <p v-if="cal.error" class="rounded-xl bg-board-panel px-5 py-3 text-accent-bust">
        {{ cal.error }}
      </p>

      <p
        v-if="cal.savedPoints && !saved"
        class="rounded-xl bg-board-panel px-5 py-3 text-accent-win"
      >
        Already calibrated. Capture a frame to verify or redo the calibration.
      </p>

      <!-- Controls -->
      <div class="flex flex-wrap items-center gap-3 rounded-2xl bg-board-panel p-5">
        <label class="text-lg font-semibold text-slate-200">Camera</label>
        <select
          v-model.number="selectedCamera"
          class="rounded-lg bg-board-bg px-4 py-2 text-lg text-slate-100 outline-none focus:ring-2 focus:ring-accent"
          :disabled="!cal.cameras.length"
        >
          <option v-for="c in cal.cameras" :key="c.index" :value="c.index">
            {{ c.name }} (#{{ c.index }})
          </option>
        </select>

        <button
          class="rounded-lg bg-accent px-5 py-2 text-lg font-bold text-board-bg disabled:opacity-40"
          :disabled="cal.loading || !cal.cameras.length"
          @click="capture"
        >
          Capture frame
        </button>

        <span class="ml-auto flex gap-2">
          <button
            class="rounded-lg bg-board-bg px-4 py-2 font-semibold text-slate-200 disabled:opacity-40"
            :disabled="!points.length"
            @click="undo"
          >
            Undo last
          </button>
          <button
            class="rounded-lg bg-board-bg px-4 py-2 font-semibold text-slate-200 disabled:opacity-40"
            :disabled="!points.length"
            @click="reset"
          >
            Reset
          </button>
        </span>
      </div>

      <!-- Frame + markers -->
      <div v-if="cal.snapshotUrl" class="space-y-4">
        <div
          class="rounded-2xl px-5 py-3 text-xl font-bold"
          :class="complete ? 'bg-accent-win/20 text-accent-win' : 'bg-board-panel text-accent'"
        >
          <template v-if="currentStep">
            {{ points.length + 1 }}/{{ STEPS.length }} — {{ currentStep.label }}:
            {{ currentStep.hint }}
          </template>
          <template v-else>
            All four points placed — review the markers, then save.
          </template>
        </div>

        <div class="overflow-auto rounded-2xl bg-board-panel p-4">
          <CalibrationCanvas
            :src="cal.snapshotUrl"
            :points="points"
            :accepting="accepting"
            @place="place"
          />
        </div>

        <div class="flex flex-wrap items-center gap-4">
          <button
            class="rounded-lg bg-accent px-6 py-3 text-xl font-bold text-board-bg disabled:opacity-40"
            :disabled="!complete"
            @click="save"
          >
            Save calibration
          </button>

          <p v-if="saved && cal.centre" class="text-lg font-semibold text-accent-win">
            Saved. Centre maps to ({{ cal.centre[0].toFixed(1) }},
            {{ cal.centre[1].toFixed(1) }}) mm — should be near the bull (0, 0).
          </p>
        </div>
      </div>

      <p v-else class="rounded-2xl bg-board-panel px-5 py-8 text-center text-slate-500">
        Capture a frame to begin marking the board points.
      </p>
    </template>
  </div>
</template>
