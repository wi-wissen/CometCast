// SPDX-License-Identifier: GPL-3.0-or-later
//
// Signalling client. Messages mirror crates/cc-proto and docs/protocol.md (version 2).

export const PROTO_VERSION = 2

/** Who may join a room (docs/protocol.md, "Join approval"). */
export type JoinMode = 'open' | 'confirm' | 'locked'
export const JOIN_MODES: readonly JoinMode[] = ['open', 'confirm', 'locked']
export const isJoinMode = (v: unknown): v is JoinMode => v === 'open' || v === 'confirm' || v === 'locked'

export type Role = 'sender' | 'receiver'

export type ClientMessage =
  | { type: 'hello'; proto_version: number; role: Role }
  // altcha: base64 of {challenge, solution}; only needed when the server answers altcha_required (docs/server.md)
  // room_name: the display's name for its room (the server cleans it, cuts it at 32 characters and relays it to senders)
  | { type: 'create_room'; altcha?: string; join_mode?: JoinMode; room_name?: string }
  // handover: single-use token of the iOS broadcast extension (native-apps.md 20.5); the web client
  // never sends one, the type only keeps the message shape in one place
  | { type: 'join_room'; code: string; name?: string; handover?: string }
  | { type: 'set_join_mode'; join_mode: JoinMode }
  // id absent + allow false = deny all waiting requests
  // block: with allow false and an id, also block the sender's address bucket for this room session
  | { type: 'join_decision'; id?: number; allow: boolean; block?: boolean }
  | { type: 'remove_peer'; block?: boolean }
  | { type: 'unblock'; id: number }
  | { type: 'end_room' }
  | { type: 'cancel_join' }
  | { type: 'offer'; sdp: string }
  | { type: 'answer'; sdp: string }
  | { type: 'ice_candidate'; candidate: string }
  // control.cmd uses serde's enum form: "stop" or {"go_to_page":{"page":3}}
  | { type: 'control'; cmd: unknown }

export type ServerMessage =
  | { type: 'hello'; proto_version: number }
  | { type: 'room_created'; code: string; join_mode: JoinMode }
  // room_name: optional, absent when the display set none (or the server is older)
  | { type: 'joined'; room_name?: string }
  // handover: the token the sender joined with (open rooms); the web display does not use it
  | { type: 'peer_joined'; name: string; handover?: string }
  | { type: 'join_pending'; timeout_secs: number; check: number; room_name?: string }
  | { type: 'join_request'; id: number; name: string; check: number; handover?: string }
  | { type: 'join_request_cancelled'; id: number; reason: string }
  | { type: 'join_mode_changed'; join_mode: JoinMode }
  | { type: 'blocked'; id: number; name: string }
  | { type: 'unblocked'; id: number }
  | { type: 'room_ended' }
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
  // Join approval (docs/protocol.md)
  'join_pending',
  'join_denied',
  'room_locked',
  'approval_timeout',
  'removed_by_host',
  'open_rooms_disabled',
  'no_such_request',
  'no_such_block',
  'block_list_full',
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
const isNum = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v)

/**
 * `room_name` is optional (protocol: additive). Absent, null, not a string or only white space all mean
 * "no room name"; the rest of the message stays valid. The server cleaned the text; it is shown as plain text.
 */
function roomNameField(o: Record<string, unknown>): { room_name?: string } {
  return isStr(o.room_name) && o.room_name.trim() !== '' ? { room_name: o.room_name } : {}
}

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
      return isStr(o.code) && isJoinMode(o.join_mode) ? { type: 'room_created', code: o.code, join_mode: o.join_mode } : null
    case 'joined':
      return { type: 'joined', ...roomNameField(o) }
    case 'peer_left':
    case 'room_ended':
      return { type: o.type }
    case 'blocked':
      return isNum(o.id) && isStr(o.name) ? { type: 'blocked', id: o.id, name: o.name } : null
    case 'unblocked':
      return isNum(o.id) ? { type: 'unblocked', id: o.id } : null
    case 'peer_joined':
      return isStr(o.name) ? { type: 'peer_joined', name: o.name, ...(isStr(o.handover) ? { handover: o.handover } : {}) } : null
    case 'join_pending':
      return isNum(o.timeout_secs) && isNum(o.check)
        ? { type: 'join_pending', timeout_secs: o.timeout_secs, check: o.check, ...roomNameField(o) }
        : null
    case 'join_request':
      return isNum(o.id) && isStr(o.name) && isNum(o.check)
        ? { type: 'join_request', id: o.id, name: o.name, check: o.check, ...(isStr(o.handover) ? { handover: o.handover } : {}) }
        : null
    case 'join_request_cancelled':
      return isNum(o.id) && isStr(o.reason) ? { type: 'join_request_cancelled', id: o.id, reason: o.reason } : null
    case 'join_mode_changed':
      return isJoinMode(o.join_mode) ? { type: 'join_mode_changed', join_mode: o.join_mode } : null
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
