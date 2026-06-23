import type {
  Calibration,
  CalibrationPoints,
  CalibrationResult,
  CameraInfo,
  GameDetail,
  GameSummary,
  SavedPlayer,
} from './protocol'

// Single place that all REST traffic flows through so base URL / headers / error
// handling stay consistent. Same-origin in prod; Vite proxies /api in dev.
const BASE = '/api'

/** REST failure carrying the HTTP status so callers can branch (e.g. 409). */
export class ApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message)
    this.name = 'ApiError'
  }
}

async function getJson<T>(path: string): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    headers: { Accept: 'application/json' },
  })
  if (!res.ok) {
    throw new ApiError(res.status, `GET ${path} failed: ${res.status} ${res.statusText}`)
  }
  return (await res.json()) as T
}

async function sendJson<T>(method: string, path: string, body: unknown): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method,
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: JSON.stringify(body),
  })
  if (!res.ok) {
    throw new ApiError(res.status, `${method} ${path} failed: ${res.status} ${res.statusText}`)
  }
  return (await res.json()) as T
}

async function sendNoContent(method: string, path: string): Promise<void> {
  const res = await fetch(`${BASE}${path}`, { method })
  if (!res.ok) {
    throw new ApiError(res.status, `${method} ${path} failed: ${res.status} ${res.statusText}`)
  }
}

async function getBlob(path: string): Promise<Blob> {
  const res = await fetch(`${BASE}${path}`)
  if (!res.ok) {
    throw new ApiError(res.status, `GET ${path} failed: ${res.status} ${res.statusText}`)
  }
  return res.blob()
}

export const api = {
  listGames(): Promise<GameSummary[]> {
    return getJson<GameSummary[]>('/games')
  },
  getGame(id: number): Promise<GameDetail> {
    return getJson<GameDetail>(`/games/${id}`)
  },
  health(): Promise<{ status: string }> {
    return getJson<{ status: string }>('/health')
  },

  // --- persistent players ("player table") ---
  listPlayers(): Promise<SavedPlayer[]> {
    return getJson<SavedPlayer[]>('/players')
  },
  createPlayer(name: string): Promise<SavedPlayer> {
    return sendJson<SavedPlayer>('POST', '/players', { name })
  },
  renamePlayer(id: string, name: string): Promise<SavedPlayer> {
    return sendJson<SavedPlayer>('PATCH', `/players/${id}`, { name })
  },
  deletePlayer(id: string): Promise<void> {
    return sendNoContent('DELETE', `/players/${id}`)
  },

  // --- camera calibration (one-time, fixed camera) ---
  listCameras(): Promise<CameraInfo[]> {
    return getJson<CameraInfo[]>('/cameras')
  },
  cameraSnapshot(camera: number): Promise<Blob> {
    return getBlob(`/camera/snapshot?camera=${camera}`)
  },
  getCalibration(): Promise<Calibration | null> {
    return getJson<Calibration | null>('/calibration')
  },
  saveCalibration(points: CalibrationPoints): Promise<CalibrationResult> {
    return sendJson<CalibrationResult>('POST', '/calibration', { points })
  },
}
