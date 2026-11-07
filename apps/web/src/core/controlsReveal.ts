// SPDX-License-Identifier: GPL-3.0-or-later
//
// While presenting only the picture is shown (4.5). Input reveals the name tag and the End button
// for 5 s after the last input (400 ms fade is CSS). Mouse movement re-triggers at most once per
// 400 ms. The first input only reveals; it never activates anything.

export const CONTROLS_HIDE_MS = 5000
export const MOUSE_THROTTLE_MS = 400

export class ControlsReveal {
  private timer: ReturnType<typeof setTimeout> | null = null
  private lastMouse = -Infinity
  private shown = false

  constructor(
    private readonly onChange: (shown: boolean) => void,
    private readonly now: () => number = Date.now,
    private readonly hideMs: number = CONTROLS_HIDE_MS,
  ) {}

  get visible(): boolean {
    return this.shown
  }

  /** Key, click, tap or touch. Returns true when the controls were hidden before (the input only reveals). */
  poke(): boolean {
    const wasHidden = !this.shown
    this.show()
    return wasHidden
  }

  /** Mouse movement, throttled. Returns true when it revealed. */
  mouseMove(): boolean {
    const t = this.now()
    if (t - this.lastMouse < MOUSE_THROTTLE_MS) return false
    this.lastMouse = t
    return this.poke()
  }

  /** Keeps the controls up (for example while End has the focus after a key). */
  hideNow(): void {
    this.clear()
    this.set(false)
  }

  dispose(): void {
    this.clear()
  }

  private show(): void {
    this.set(true)
    this.clear()
    this.timer = setTimeout(() => this.set(false), this.hideMs)
  }
  private clear(): void {
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
  }
  private set(v: boolean): void {
    if (v === this.shown) return
    this.shown = v
    this.onChange(v)
  }
}
