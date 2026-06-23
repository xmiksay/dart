import type { ClientCommand, ServerEvent } from './protocol'

export type WsStatus = 'connecting' | 'open' | 'closed'

interface WsHandlers {
  onEvent: (event: ServerEvent) => void
  onStatus: (status: WsStatus) => void
}

/** Resolve the /ws URL for the current origin (Vite proxies it in dev). */
function wsUrl(): string {
  const proto = window.location.protocol === 'https:' ? 'wss' : 'ws'
  return `${proto}://${window.location.host}/ws`
}

/**
 * Thin WebSocket client for the live-play channel. Owns reconnection and JSON
 * (de)serialization; the store supplies handlers and sends typed commands.
 */
export class GameSocket {
  private socket: WebSocket | null = null
  private handlers: WsHandlers
  private reconnectTimer: number | null = null
  private closedByUser = false

  constructor(handlers: WsHandlers) {
    this.handlers = handlers
  }

  connect(): void {
    this.closedByUser = false
    this.open()
  }

  private open(): void {
    this.handlers.onStatus('connecting')
    const socket = new WebSocket(wsUrl())
    this.socket = socket

    socket.onopen = () => this.handlers.onStatus('open')

    socket.onmessage = (ev) => {
      try {
        const parsed = JSON.parse(ev.data as string) as ServerEvent
        this.handlers.onEvent(parsed)
      } catch (err) {
        console.error('[ws] bad message', err)
      }
    }

    socket.onclose = () => {
      this.handlers.onStatus('closed')
      if (!this.closedByUser) this.scheduleReconnect()
    }

    socket.onerror = () => socket.close()
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer !== null) return
    this.reconnectTimer = window.setTimeout(() => {
      this.reconnectTimer = null
      this.open()
    }, 1500)
  }

  send(command: ClientCommand): void {
    if (this.socket?.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify(command))
    } else {
      console.warn('[ws] dropped command, socket not open', command.type)
    }
  }

  close(): void {
    this.closedByUser = true
    if (this.reconnectTimer !== null) {
      window.clearTimeout(this.reconnectTimer)
      this.reconnectTimer = null
    }
    this.socket?.close()
    this.socket = null
  }
}
