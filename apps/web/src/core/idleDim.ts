// SPDX-License-Identifier: GPL-3.0-or-later
//
// Idle dimming (8.6): after 7 minutes on Idle without activity the idle screen fades to 40 %
// brightness; any input, join request or connection restores it at once. This timer never holds a
// wake lock. The delay is one constant. For automated tests it can be shortened ONLY at build time
// (VITE_E2E_DIM_MS in the build environment, see apps/web/e2e/display.spec.mjs); a production
// build has no such value and nothing in the URL or the page can change it.

export const IDLE_DIM_MS = 7 * 60 * 1000

/** Build-time override for end-to-end builds; undefined in every normal build. */
export function dimDelayMs(): number {
  const raw = (import.meta.env as Record<string, string | undefined>).VITE_E2E_DIM_MS
  const n = raw === undefined ? NaN : Number(raw)
  return Number.isFinite(n) && n > 0 ? n : IDLE_DIM_MS
}

export class IdleDim {
  private timer: ReturnType<typeof setTimeout> | null = null
  private enabled = false
  private dimmed = false

  constructor(
    private readonly onDim: (dimmed: boolean) => void,
    private readonly delay: number = dimDelayMs(),
  ) {}

  get isDimmed(): boolean {
    return this.dimmed
  }

  /** Idle (and nothing else going on) = enabled. Leaving idle restores the brightness. */
  setEnabled(enabled: boolean): void {
    if (enabled === this.enabled) return
    this.enabled = enabled
    if (enabled) this.arm()
    else {
      this.clear()
      this.set(false)
    }
  }

  /** Any input, join, request or mode change. */
  activity(): void {
    if (!this.enabled) return
    this.set(false)
    this.arm()
  }

  dispose(): void {
    this.clear()
  }

  private arm(): void {
    this.clear()
    this.timer = setTimeout(() => this.set(true), this.delay)
  }
  private clear(): void {
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
  }
  private set(d: boolean): void {
    if (d === this.dimmed) return
    this.dimmed = d
    this.onDim(d)
  }
}
