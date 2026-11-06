// SPDX-License-Identifier: GPL-3.0-or-later
import type { WebSocketLike } from '../../src/core/signaling'

export class FakeSocket implements WebSocketLike {
  sent: any[] = []
  closed = false
  onopen: any = null
  onmessage: any = null
  onclose: any = null
  onerror: any = null
  send(d: string) {
    this.sent.push(JSON.parse(d))
  }
  close() {
    this.closed = true
  }
  open() {
    this.onopen?.({})
  }
  recv(m: object) {
    this.onmessage?.({ data: JSON.stringify(m) })
  }
  drop() {
    this.onclose?.({ code: 1006 })
  }
}
