<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import Icon from './Icon.vue'
import { logoFor } from './display/logos'
import { arrowDir, moveFocus } from '../core/dpad'
import { useDisplayScale } from '../composables/useDisplayScale'
import { useFocusRing } from '../composables/useFocusRing'
import { useRelayConfirmation } from '../composables/useRelay'
import { useTheme } from '../composables/useTheme'

// A relay that only a link (`?server=`) asked for is confirmed before anything else is shown or connects
// (docs/design/first-run-and-display.md, 7.2, decision 13.37). Nothing of the page that would use the relay is drawn
// behind it (App.vue holds the page back), and the host appears nowhere else until it is confirmed.
//   /display        a 600 reference-px dialog card on the scaled canvas over plain --bg, with only the logo.
//   presenter pages its own page in the error-page layout.
// Cancel (secondary) is focused first and comes first, so Enter on load cancels; Back / Escape = Cancel.
const props = defineProps<{ variant?: 'display' | 'presenter' }>()
const { t } = useI18n()
const confirm = useRelayConfirmation()
const { theme } = useTheme()
const root = ref<HTMLElement | null>(null)
const cancelBtn = ref<HTMLButtonElement | null>(null)
const useBtn = ref<HTMLButtonElement | null>(null)
const display = () => props.variant === 'display'
useDisplayScale(root)
const { hidden: ringHidden } = useFocusRing()

async function focusCancel() {
  await nextTick()
  cancelBtn.value?.focus()
}
watch(() => confirm.relay.value, (r) => r && void focusCancel())
onMounted(() => confirm.relay.value && void focusCancel())

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    e.preventDefault()
    confirm.cancel()
  } else if (display() && arrowDir(e.key) && root.value) {
    e.preventDefault()
    moveFocus(root.value, arrowDir(e.key)!)
  } else if (e.key === 'Tab') {
    // Keep the focus inside the dialog (two buttons).
    const first = cancelBtn.value
    const last = useBtn.value
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault()
      last?.focus()
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault()
      first?.focus()
    }
  }
}
</script>

<template>
  <!-- Display: the display's tokens on the 960 x 540 canvas: the logo at the idle screen's top-left, the 600 card centred. -->
  <div
    v-if="confirm.relay.value && display()"
    ref="root"
    class="cd cd-relay"
    :class="{ 'cd-pointer': ringHidden }"
    :data-theme="theme"
    data-testid="relay-confirm-backdrop"
    @keydown="onKey"
  >
    <div class="cd-canvas">
      <img class="cd-relaylogo" :src="logoFor(theme)" alt="" />
      <div class="cd-relaycenter">
        <div class="cd-wz" role="alertdialog" aria-modal="true" aria-labelledby="relay-confirm-title" aria-describedby="relay-confirm-body" data-testid="relay-confirm">
          <div class="cd-rh"><h2 id="relay-confirm-title">{{ t('relay.confirm.title') }}</h2></div>
          <p id="relay-confirm-body" class="cd-body">{{ t('relay.confirm.body', { host: confirm.relay.value.host }) }}</p>
          <p class="cd-addr mono" data-testid="relay-confirm-host">{{ confirm.relay.value.host }}</p>
          <p class="cd-body">{{ t('relay.confirm.risk') }}</p>
          <div class="cd-acts cd-actsl">
            <button ref="cancelBtn" type="button" class="cd-tbtn" data-row="0" data-testid="relay-confirm-cancel" @click="confirm.cancel()">
              {{ t('relay.confirm.cancel') }}
            </button>
            <button ref="useBtn" type="button" class="cd-tbtn p" data-row="0" data-testid="relay-confirm-use" @click="confirm.accept()">
              {{ t('relay.confirm.use') }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>

  <!-- Presenter pages: a page in the error-page layout (icon disc, heading, body, host line, risk as small print, actions). -->
  <div v-else-if="confirm.relay.value" class="cp" data-testid="relay-confirm-backdrop" @keydown="onKey">
    <main class="cp-page cp-relaypage" role="alertdialog" aria-modal="true" aria-labelledby="relay-confirm-title" aria-describedby="relay-confirm-body" data-testid="relay-confirm">
      <div class="cp-ico warn"><Icon name="info" /></div>
      <h1 id="relay-confirm-title" class="cp-h">{{ t('relay.confirm.title') }}</h1>
      <p id="relay-confirm-body" class="cp-p">{{ t('relay.confirm.body', { host: confirm.relay.value.host }) }}</p>
      <p class="cp-host mono" data-testid="relay-confirm-host">{{ confirm.relay.value.host }}</p>
      <p class="cp-small">{{ t('relay.confirm.risk') }}</p>
      <div class="cp-spacer"></div>
      <div class="cp-acts">
        <button ref="cancelBtn" type="button" class="cp-sbtn" data-testid="relay-confirm-cancel" @click="confirm.cancel()">
          {{ t('relay.confirm.cancel') }}
        </button>
        <button ref="useBtn" type="button" class="cp-pbtn" data-testid="relay-confirm-use" @click="confirm.accept()">
          {{ t('relay.confirm.use') }}
        </button>
      </div>
    </main>
  </div>
</template>
