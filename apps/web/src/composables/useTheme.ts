// SPDX-License-Identifier: GPL-3.0-or-later
import { computed, onBeforeUnmount, ref } from 'vue'
import { loadAppearance, resolveAppearance, saveAppearance, type Appearance } from '../core/appearance'

/** Appearance of the display, kept on this device; "auto" follows prefers-color-scheme. */
export function useTheme() {
  const appearance = ref<Appearance>(loadAppearance())
  const mq = typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: dark)') : null
  const systemDark = ref(mq ? mq.matches : true)
  const onChange = (e: MediaQueryListEvent) => (systemDark.value = e.matches)
  mq?.addEventListener?.('change', onChange)
  onBeforeUnmount(() => mq?.removeEventListener?.('change', onChange))
  const theme = computed(() => resolveAppearance(appearance.value, systemDark.value))
  function setAppearance(a: Appearance) {
    appearance.value = a
    saveAppearance(a)
  }
  return { appearance, theme, setAppearance }
}
