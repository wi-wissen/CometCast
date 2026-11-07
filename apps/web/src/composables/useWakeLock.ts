// SPDX-License-Identifier: GPL-3.0-or-later
import { onBeforeUnmount, watch, type Ref } from 'vue'
import { ScreenWakeLock } from '../core/wakeLock'

/** Holds the screen awake while `active` is true (presenting) and lets go otherwise. */
export function useWakeLock(active: Ref<boolean>): ScreenWakeLock {
  const lock = new ScreenWakeLock()
  watch(active, (a) => lock.setWanted(a), { immediate: true })
  onBeforeUnmount(() => lock.dispose())
  return lock
}
