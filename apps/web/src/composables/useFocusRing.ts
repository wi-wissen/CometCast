// SPDX-License-Identifier: GPL-3.0-or-later
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { ringHidden } from '../core/focusRing'

/**
 * `hidden` is true after the first mouse or touch input and false again after the next key press (spec 7.2).
 * The ring is shown at load. The display's CSS hides the ring while `hidden` (class `cd-pointer`).
 */
export function useFocusRing() {
  const hidden = ref(false)
  const key = () => (hidden.value = ringHidden(hidden.value, 'key'))
  const pointer = () => (hidden.value = ringHidden(hidden.value, 'pointer'))
  // A real mouse move counts as mouse input; the zero-movement events that browsers send after a layout change do not.
  const move = (e: PointerEvent) => {
    if (e.pointerType === 'mouse' && (e.movementX !== 0 || e.movementY !== 0)) pointer()
  }
  onMounted(() => {
    window.addEventListener('keydown', key, true)
    window.addEventListener('pointerdown', pointer, true)
    window.addEventListener('pointermove', move, true)
    window.addEventListener('touchstart', pointer, { capture: true, passive: true })
  })
  onBeforeUnmount(() => {
    window.removeEventListener('keydown', key, true)
    window.removeEventListener('pointerdown', pointer, true)
    window.removeEventListener('pointermove', move, true)
    window.removeEventListener('touchstart', pointer, true)
  })
  return { hidden }
}
