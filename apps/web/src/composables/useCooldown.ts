// SPDX-License-Identifier: GPL-3.0-or-later
import { computed, onBeforeUnmount, ref, type ShallowRef } from 'vue'
import type { SenderState } from '../core/senderSession'

/** Seconds a sender waits before it may ask again after the host denied it. */
export const DENIED_COOLDOWN_SECS = 15

/** Seconds left until "ask again" is allowed (0 = now). Only a denial causes a wait. */
export function useDeniedCooldown(state: ShallowRef<SenderState>) {
  const now = ref(Date.now())
  const timer = setInterval(() => (now.value = Date.now()), 500)
  onBeforeUnmount(() => clearInterval(timer))
  return computed(() => {
    const s = state.value
    if (s.phase !== 'error' || s.errorCode !== 'join_denied' || s.errorAt === undefined) return 0
    // `now` is refreshed every 500 ms and can be a moment older than the error: never count backwards.
    return Math.max(0, DENIED_COOLDOWN_SECS - Math.max(0, Math.floor((now.value - s.errorAt) / 1000)))
  })
}
