// SPDX-License-Identifier: GPL-3.0-or-later
import { onBeforeUnmount, onMounted, type Ref } from 'vue'

/** The reference canvas of the display (docs/design/first-run-and-display.md, 3.1). */
export const CANVAS_W = 960
export const CANVAS_H = 540

/** 1 reference px = min(width / 960, height / 540) px; the rest of the viewport is filled with --bg. */
export function displayScale(width: number, height: number): number {
  if (width <= 0 || height <= 0) return 1
  return Math.min(width / CANVAS_W, height / CANVAS_H)
}

/** Sets --s on the element and keeps it in step with the window size. */
export function useDisplayScale(el: Ref<HTMLElement | null>): void {
  const apply = () => {
    const e = el.value
    if (e) e.style.setProperty('--s', String(displayScale(window.innerWidth, window.innerHeight)))
  }
  onMounted(() => {
    apply()
    window.addEventListener('resize', apply)
  })
  onBeforeUnmount(() => window.removeEventListener('resize', apply))
}
