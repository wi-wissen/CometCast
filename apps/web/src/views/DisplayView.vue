<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import SetupWizard from '../components/display/SetupWizard.vue'
import DisplayRunning from '../components/display/DisplayRunning.vue'
import { arrowDir, moveFocus } from '../core/dpad'
import { loadSetup, saveSetup, type DisplaySetup } from '../core/displaySetup'
import { useDisplayScale } from '../composables/useDisplayScale'
import { useRelay } from '../composables/useRelay'
import { useTheme } from '../composables/useTheme'

// The display (receiver) page at {dom}/display (docs/design/first-run-and-display.md, 4 and 7.2):
// the setup wizard on first run, then the idle screen, presenting and the dialogs.
const root = ref<HTMLElement | null>(null)
useDisplayScale(root)
const { appearance, theme, setAppearance } = useTheme()
const relay = useRelay()
const setup = ref<DisplaySetup | null>(loadSetup())
// The server chosen in the setup applies again after a restart (a ?server= link still wins).
if (setup.value?.server && !new URLSearchParams(location.search).has('server')) relay.set(setup.value.server)

const wizard = ref<InstanceType<typeof SetupWizard> | null>(null)
function finish(s: DisplaySetup) {
  saveSetup(s)
  setup.value = s
}

// Setup keys: arrows move the focus, Escape / Back goes one step back (text fields keep their own arrows).
function onKey(e: KeyboardEvent) {
  if (setup.value || e.ctrlKey || e.metaKey || e.altKey) return
  const dir = arrowDir(e.key)
  const inText = e.target instanceof HTMLInputElement && e.target.type === 'text'
  if (dir && !(inText && (dir === 'left' || dir === 'right')) && root.value) {
    e.preventDefault()
    moveFocus(root.value, dir)
  } else if (e.key === 'Escape') {
    e.preventDefault()
    wizard.value?.back()
  }
}
onMounted(() => {
  window.addEventListener('keydown', onKey)
  document.documentElement.classList.add('cd-page')
})
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey)
  document.documentElement.classList.remove('cd-page')
})
</script>

<template>
  <div ref="root" class="cd" :data-theme="theme" data-testid="display">
    <div v-if="!setup" class="cd-canvas">
      <SetupWizard ref="wizard" :theme="theme" @finish="finish" />
    </div>
    <DisplayRunning v-else :setup="setup" :theme="theme" :appearance="appearance" @appearance="setAppearance" />
  </div>
</template>
