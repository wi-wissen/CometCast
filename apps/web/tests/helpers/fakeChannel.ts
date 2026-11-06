// SPDX-License-Identifier: GPL-3.0-or-later
import type { ControlChannel } from '../../src/core/control'

/** In-memory data channel. `pair()` connects two ends; `drainAfter` simulates a slow link. */
export class FakeChannel implements ControlChannel {
  label = 'control'
  readyState = 'open'
  bufferedAmount = 0
  bufferedAmountLowThreshold = 0
  binaryType = 'blob'
  onopen: ControlChannel['onopen'] = null
  onclose: ControlChannel['onclose'] = null
  onmessage: ControlChannel['onmessage'] = null
  onbufferedamountlow: ControlChannel['onbufferedamountlow'] = null
  sent: (string | ArrayBuffer)[] = []
  peer: FakeChannel | null = null
  /** If set, every send adds its size to bufferedAmount and draining happens via drain(). */
  simulateBuffer = false

  send(data: string | ArrayBuffer): void {
    this.sent.push(data)
    if (this.simulateBuffer) this.bufferedAmount += typeof data === 'string' ? data.length : data.byteLength
    const p = this.peer
    if (p) queueMicrotask(() => p.onmessage?.({ data }))
  }

  /** Empties the simulated send buffer and fires bufferedamountlow. */
  drain(): void {
    this.bufferedAmount = 0
    this.onbufferedamountlow?.({})
  }

  close(): void {
    this.readyState = 'closed'
    this.onclose?.({})
  }
}

export function pair(): [FakeChannel, FakeChannel] {
  const a = new FakeChannel()
  const b = new FakeChannel()
  a.peer = b
  b.peer = a
  return [a, b]
}
