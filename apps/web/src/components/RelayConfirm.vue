<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { arrowDir, moveFocus } from '../core/dpad'
import { useDisplayScale } from '../composables/useDisplayScale'
import { useFocusRing } from '../composables/useFocusRing'
import { useRelayConfirmation } from '../composables/useRelay'
import { useTheme } from '../composables/useTheme'

// Blocking dialog for a relay that only a link (`?server=`) asked for. Nothing connects or is
// captured until the person answers. The safe answer (default relay) is focused first, so an Enter on load
// cancels. On `/display` it is the display's dialog card (spec 4.3 / 4.4 look, focus row [Cancel, Use this relay]);
// on the presenter pages it is a presenter card (spec 7.2).
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

watch(
  () => confirm.relay.value,
  async (r) => {
    if (r) {
      await nextTick()
      cancelBtn.value?.focus()
    }
  },
)

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
  <!-- Display: the display's tokens on a reference-px layer over the page (scrim 55 %). -->
  <div
    v-if="confirm.relay.value && display()"
    ref="root"
    class="cd cd-over"
    :class="{ 'cd-pointer': ringHidden }"
    :data-theme="theme"
    data-testid="relay-confirm-backdrop"
    @keydown="onKey"
  >
    <div class="cd-hud" style="z-index: 60">
      <div class="cd-modal">
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

  <!-- Presenter pages: a presenter card (heading 24/700, body 15, risk as small print, buttons as on error pages). -->
  <div v-else-if="confirm.relay.value" class="cp cp-over" data-testid="relay-confirm-backdrop" @keydown="onKey">
    <div class="cp-card" role="alertdialog" aria-modal="true" aria-labelledby="relay-confirm-title" aria-describedby="relay-confirm-body" data-testid="relay-confirm">
      <h1 id="relay-confirm-title" class="cp-h">{{ t('relay.confirm.title') }}</h1>
      <p id="relay-confirm-body" class="cp-p">{{ t('relay.confirm.body', { host: confirm.relay.value.host }) }}</p>
      <p class="cp-host mono" data-testid="relay-confirm-host">{{ confirm.relay.value.host }}</p>
      <p class="cp-small">{{ t('relay.confirm.risk') }}</p>
      <div class="cp-acts">
        <button ref="cancelBtn" type="button" class="cp-sbtn" data-testid="relay-confirm-cancel" @click="confirm.cancel()">
          {{ t('relay.confirm.cancel') }}
        </button>
        <button ref="useBtn" type="button" class="cp-pbtn" data-testid="relay-confirm-use" @click="confirm.accept()">
          {{ t('relay.confirm.use') }}
        </button>
      </div>
    </div>
  </div>
</template>
