// SPDX-License-Identifier: GPL-3.0-or-later
//
// Keeps the display device's screen awake ONLY while someone presents (D5, 8.7): acquired when a
// presentation shows its first frame, released when it stops, when the session ends or when the
// page goes to the background; asked for again when the page becomes visible while still wanted.
// Uses the Screen Wake Lock API; every call is wrapped (the API can be missing or refuse).

export interface WakeLockSentinelLike {
  release(): Promise<void>
  addEventListener?(type: 'release', cb: () => void): void
}
export interface WakeLockEnv {
  wakeLock?: { request(type: 'screen'): Promise<WakeLockSentinelLike> }
}
export interface VisibilityEnv {
  visibilityState: string
  addEventListener(type: 'visibilitychange', cb: () => void): void
  removeEventListener(type: 'visibilitychange', cb: () => void): void
}

export class ScreenWakeLock {
  private wanted = false
  private sentinel: WakeLockSentinelLike | null = null
  private requesting = false
  private readonly onVisibility = () => {
    // The browser releases the lock itself when the page is hidden; ask again when it is back.
    if (this.doc.visibilityState === 'visible') void this.acquire()
    else void this.drop()
  }

  constructor(
    private readonly nav: WakeLockEnv = typeof navigator === 'undefined' ? {} : (navigator as unknown as WakeLockEnv),
    private readonly doc: VisibilityEnv = document,
  ) {
    this.doc.addEventListener('visibilitychange', this.onVisibility)
  }

  /** True while a lock is held. */
  get held(): boolean {
    return this.sentinel !== null
  }

  /** Called with `true` while someone presents and `false` otherwise. */
  setWanted(wanted: boolean): void {
    if (wanted === this.wanted) return
    this.wanted = wanted
    if (wanted) void this.acquire()
    else void this.drop()
  }

  dispose(): void {
    this.wanted = false
    this.doc.removeEventListener('visibilitychange', this.onVisibility)
    void this.drop()
  }

  private async acquire(): Promise<void> {
    if (!this.wanted || this.sentinel || this.requesting || this.doc.visibilityState !== 'visible') return
    const api = this.nav.wakeLock
    if (!api) return
    this.requesting = true
    try {
      const s = await api.request('screen')
      if (!this.wanted) {
        // Stopped while the request was running.
        await s.release().catch(() => undefined)
        return
      }
      this.sentinel = s
      s.addEventListener?.('release', () => {
        if (this.sentinel === s) this.sentinel = null
      })
    } catch {
      /* refused (battery saver, not allowed): the display simply may sleep */
    } finally {
      this.requesting = false
    }
  }

  private async drop(): Promise<void> {
    const s = this.sentinel
    this.sentinel = null
    if (s) await s.release().catch(() => undefined)
  }
}
