// SPDX-License-Identifier: GPL-3.0-or-later
//
// Signalling client. Messages mirror crates/cc-proto and docs/protocol.md (version 1).

export const PROTO_VERSION = 1

export type Role = 'sender' | 'receiver'

export type ClientMessage =
  | { type: 'hello'; proto_version: number; role: Role }
  // altcha: base64 of {challenge, solution}; only needed when the server answers altcha_required (docs/server.md)
  | { type: 'create_room'; altcha?: string }
  | { type: 'join_room'; code: string }
  | { type: 'offer'; sdp: string }
  | { type: 'answer'; sdp: string }
  | { type: 'ice_candidate'; candidate: string }
  // control.cmd uses serde's enum form: "stop" or {"go_to_page":{"page":3}}
  | { type: 'control'; cmd: unknown }

export type ServerMessage =
  | { type: 'hello'; proto_version: number }
  | { type: 'room_created'; code: string }
  | { type: 'joined' }
  | { type: 'peer_joined' }
  | { type: 'offer'; sdp: string }
  | { type: 'answer'; sdp: string }
  | { type: 'ice_candidate'; candidate: string }
  | { type: 'control'; cmd: unknown }
  | { type: 'peer_left' }
  | { type: 'error'; code: string; message: string }

/** Error codes the server documents. Unknown codes are still passed through as strings. */
export const SERVER_ERROR_CODES = [
  'bad_hello',
  'proto_mismatch',
  'wrong_role',
  'room_not_found',
  'room_full',
  'not_in_room',
  'bad_message',
  // Added with the server's M8 (docs/protocol.md). HTTP-side refusals of /altcha/challenge use
  // the same words (see altchaClient.ts), plus 'forbidden', 'server_full' and 'altcha_failed'.
  'altcha_required',
  'altcha_invalid',
  'rate_limited',
  'banned',
  'forbidden',
  'server_full',
  'altcha_failed',
  'altcha_too_hard',
  'altcha_expired',
] as const

export function encodeClientMessage(m: ClientMessage): string {
  return JSON.stringify(m)
}

const isStr = (v: unknown): v is string => typeof v === 'string'

/** Parses one server text frame. Returns null for anything that is not a valid server message. */
export function decodeServerMessage(text: unknown): ServerMessage | null {
  if (!isStr(text)) return null
  let v: unknown
  try {
    v = JSON.parse(text)
  } catch {
    return null
  }
  if (v === null || typeof v !== 'object') return null
  const o = v as Record<string, unknown>
  switch (o.type) {
    case 'hello':
      return typeof o.proto_version === 'number' ? { type: 'hello', proto_version: o.proto_version } : null
    case 'room_created':
      return isStr(o.code) ? { type: 'room_created', code: o.code } : null
    case 'joined':
    case 'peer_joined':
    case 'peer_left':
      return { type: o.type }
    case 'offer':
    case 'answer':
      return isStr(o.sdp) ? { type: o.type, sdp: o.sdp } : null
    case 'ice_candidate':
      return isStr(o.candidate) ? { type: 'ice_candidate', candidate: o.candidate } : null
    case 'control':
      return 'cmd' in o ? { type: 'control', cmd: o.cmd } : null
    case 'error':
      return isStr(o.code) ? { type: 'error', code: o.code, message: isStr(o.message) ? o.message : '' } : null
    default:
      return null
  }
}

/**
 * ws:// or wss:// URL for /ws on the page origin. `?server=host:port` overrides the host, but
 * only in development builds: in production a crafted link must not redirect the signalling.
 */
export function signalingUrl(
  loc: { protocol: string; host: string; search: string },
  allowOverride: boolean = import.meta.env.DEV,
): string {
  const scheme = loc.protocol === 'https:' ? 'wss:' : 'ws:'
  let host = loc.host
  const override = new URLSearchParams(loc.search).get('server')
  // Only a plain host[:port] is accepted, so the parameter cannot inject a path or a scheme.
  if (allowOverride && override && /^[A-Za-z0-9.-]+(:\d{1,5})?$/.test(override)) host = override
  return `${scheme}//${host}/ws`
}

/** The subset of the browser WebSocket that this client uses (lets tests inject a fake). */
export interface WebSocketLike {
  send(data: string): void
  close(code?: number, reason?: string): void
  onopen: ((ev: unknown) => void) | null
  onmessage: ((ev: { data: unknown }) => void) | null
  onclose: ((ev: { code?: number; reason?: string }) => void) | null
  onerror: ((ev: unknown) => void) | null
}

export type SocketFactory = (url: string) => WebSocketLike

export interface SignalingHandlers {
  /** Server accepted our hello. */
  onReady(): void
  onMessage(m: ServerMessage): void
  /** Connection ended without us calling close() (server closed it, or it never opened). */
  onClose(info: { code?: number; reason?: string }): void
}

export class SignalingClient {
  private ws: WebSocketLike | null = null
  private closedByUs = false

  constructor(
    private readonly url: string,
    private readonly role: Role,
    private readonly handlers: SignalingHandlers,
    private readonly factory: SocketFactory = (u) => new WebSocket(u) as unknown as WebSocketLike,
  ) {}

  open(): void {
    const ws = this.factory(this.url)
    this.ws = ws
    ws.onopen = () => this.send({ type: 'hello', proto_version: PROTO_VERSION, role: this.role })
    ws.onmessage = (ev) => {
      const m = decodeServerMessage(ev.data)
      if (!m) return // not a valid server message: ignored
      if (m.type === 'hello') {
        if (m.proto_version !== PROTO_VERSION) {
          this.handlers.onMessage({ type: 'error', code: 'proto_mismatch', message: 'protocol version differs' })
          this.close()
          return
        }
        this.handlers.onReady()
        return
      }
      this.handlers.onMessage(m)
    }
    ws.onclose = (ev) => {
      if (this.closedByUs) return
      this.ws = null
      this.handlers.onClose({ code: ev.code, reason: ev.reason })
    }
    // A failing socket is always followed by onclose, which reports it.
    ws.onerror = () => {}
  }

  send(m: ClientMessage): void {
    this.ws?.send(encodeClientMessage(m))
  }

  close(): void {
    this.closedByUs = true
    const ws = this.ws
    this.ws = null
    if (ws) {
      ws.onopen = ws.onmessage = ws.onclose = ws.onerror = null
      try {
        ws.close(1000)
      } catch {
        /* already closed */
      }
    }
  }
}
