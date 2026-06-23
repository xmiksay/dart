import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api, ApiError } from '@/api/client'
import type { CalibrationPoints, CameraInfo, PixelPoint } from '@/api/protocol'

/**
 * Server state for the one-time camera calibration flow. Camera endpoints
 * return 501 when the backend wasn't built with `--features capture`; that case
 * is surfaced via `captureUnavailable` so the view can degrade gracefully.
 */
export const useCalibrationStore = defineStore('calibration', () => {
  const cameras = ref<CameraInfo[]>([])
  const captureUnavailable = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)

  /** Object URL of the captured frame; revoked before being replaced. */
  const snapshotUrl = ref<string | null>(null)
  /** Existing saved calibration in original-image pixels, if any. */
  const savedPoints = ref<CalibrationPoints | null>(null)
  /** Board-mm centre returned by the last successful save (≈ bull). */
  const centre = ref<PixelPoint | null>(null)

  function isNotImplemented(err: unknown): boolean {
    return err instanceof ApiError && err.status === 501
  }

  /** Load camera list + any existing calibration. Safe to call repeatedly. */
  async function init() {
    loading.value = true
    error.value = null
    captureUnavailable.value = false
    try {
      cameras.value = await api.listCameras()
    } catch (err) {
      if (isNotImplemented(err)) {
        captureUnavailable.value = true
      } else {
        error.value = err instanceof Error ? err.message : 'failed to list cameras'
      }
    }
    try {
      const existing = await api.getCalibration()
      savedPoints.value = existing ? existing.points : null
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'failed to load calibration'
    } finally {
      loading.value = false
    }
  }

  async function capture(camera: number) {
    error.value = null
    try {
      const blob = await api.cameraSnapshot(camera)
      if (snapshotUrl.value) URL.revokeObjectURL(snapshotUrl.value)
      snapshotUrl.value = URL.createObjectURL(blob)
    } catch (err) {
      if (isNotImplemented(err)) {
        captureUnavailable.value = true
      } else {
        error.value = err instanceof Error ? err.message : 'failed to capture frame'
      }
    }
  }

  /** Persist the four canonical-order points (original-image pixels). */
  async function save(points: CalibrationPoints): Promise<boolean> {
    error.value = null
    centre.value = null
    try {
      const result = await api.saveCalibration(points)
      centre.value = result.centre
      savedPoints.value = points
      return true
    } catch (err) {
      if (err instanceof ApiError && err.status === 400) {
        error.value = 'rejected: the four points do not form a valid board quad'
      } else {
        error.value = err instanceof Error ? err.message : 'failed to save calibration'
      }
      return false
    }
  }

  function clearSnapshot() {
    if (snapshotUrl.value) URL.revokeObjectURL(snapshotUrl.value)
    snapshotUrl.value = null
  }

  return {
    cameras,
    captureUnavailable,
    loading,
    error,
    snapshotUrl,
    savedPoints,
    centre,
    init,
    capture,
    save,
    clearSnapshot,
  }
})
