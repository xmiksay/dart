<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import type { PixelPoint } from '@/api/protocol'

/**
 * Shows a captured frame and lets the operator place ordered markers on it.
 * Clicks are reported in ORIGINAL image pixels (not displayed pixels): the
 * `<img>` is scaled to fit, so each click is converted via the rendered rect.
 * Markers passed back in (`points`) are stored in original pixels and rendered
 * by reversing that scale, so saved calibrations overlay correctly on redo.
 */
const props = defineProps<{
  src: string
  /** Already-placed points in original-image pixels. */
  points: PixelPoint[]
  /** Whether more clicks are accepted (false once all 4 are placed). */
  accepting: boolean
}>()

const emit = defineEmits<{ place: [point: PixelPoint] }>()

const imgEl = ref<HTMLImageElement | null>(null)
// Natural size, captured on load so display->original scaling is available.
const natural = ref({ w: 0, h: 0 })
// Rendered size of the (object-contain) image inside its box.
const rendered = ref({ w: 0, h: 0 })

function onLoad() {
  const el = imgEl.value
  if (!el) return
  natural.value = { w: el.naturalWidth, h: el.naturalHeight }
  measure()
}

function measure() {
  const el = imgEl.value
  if (!el) return
  const rect = el.getBoundingClientRect()
  rendered.value = { w: rect.width, h: rect.height }
}

function onClick(ev: MouseEvent) {
  if (!props.accepting) return
  const el = imgEl.value
  if (!el) return
  const rect = el.getBoundingClientRect()
  // click position relative to the rendered image, scaled into original pixels.
  const srcX = ((ev.clientX - rect.left) * el.naturalWidth) / rect.width
  const srcY = ((ev.clientY - rect.top) * el.naturalHeight) / rect.height
  emit('place', [srcX, srcY])
}

// Keep marker placement aligned when the layout/viewport changes.
onMounted(() => window.addEventListener('resize', measure))
onBeforeUnmount(() => window.removeEventListener('resize', measure))

/** Original-pixel point -> displayed CSS pixels for marker placement. */
function toDisplay(p: PixelPoint): { left: string; top: string } {
  if (!natural.value.w || !natural.value.h) return { left: '0', top: '0' }
  const x = (p[0] / natural.value.w) * rendered.value.w
  const y = (p[1] / natural.value.h) * rendered.value.h
  return { left: `${x}px`, top: `${y}px` }
}
</script>

<template>
  <div class="relative inline-block">
    <img
      ref="imgEl"
      :src="src"
      alt="captured camera frame"
      class="block max-h-[70vh] max-w-full select-none rounded-xl"
      :class="accepting ? 'cursor-crosshair' : 'cursor-default'"
      draggable="false"
      @load="onLoad"
      @click="onClick"
    />

    <span
      v-for="(p, i) in points"
      :key="i"
      class="pointer-events-none absolute flex h-8 w-8 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full border-2 border-board-bg bg-accent text-base font-black text-board-bg shadow-lg"
      :style="toDisplay(p)"
    >
      {{ i + 1 }}
    </span>
  </div>
</template>
