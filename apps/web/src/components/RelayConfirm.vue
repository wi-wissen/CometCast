<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRelayConfirmation } from '../composables/useRelay'

// Blocking dialog for a relay that only a link (`?server=`) asked for. Nothing connects or is
// captured until the person answers. The safe answer (default relay) is focused first.
const { t } = useI18n()
const confirm = useRelayConfirmation()
const cancelBtn = ref<HTMLButtonElement | null>(null)
const useBtn = ref<HTMLButtonElement | null>(null)

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
  <div
    v-if="confirm.relay.value"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 px-4"
    data-testid="relay-confirm-backdrop"
    @keydown="onKey"
  >
    <div
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="relay-confirm-title"
      aria-describedby="relay-confirm-body"
      class="w-full max-w-lg rounded-cc border border-warn bg-surface p-5 shadow-xl"
      data-testid="relay-confirm"
    >
      <h2 id="relay-confirm-title" class="text-lg font-semibold">{{ t('relay.confirm.title') }}</h2>
      <p id="relay-confirm-body" class="mt-2">
        {{ t('relay.confirm.body', { host: confirm.relay.value.host }) }}
      </p>
      <p class="mt-2 rounded-cc bg-sunk px-3 py-2 font-mono text-lg" data-testid="relay-confirm-host">{{ confirm.relay.value.host }}</p>
      <p class="mt-2 text-sm text-muted">{{ t('relay.confirm.risk') }}</p>
      <div class="mt-4 flex flex-wrap justify-end gap-2">
        <button
          ref="cancelBtn"
          type="button"
          data-testid="relay-confirm-cancel"
          class="rounded-cc border border-line2 bg-surface px-4 py-2 font-medium"
          @click="confirm.cancel()"
        >
          {{ t('relay.confirm.cancel') }}
        </button>
        <button
          ref="useBtn"
          type="button"
          data-testid="relay-confirm-use"
          class="rounded-cc bg-accent px-4 py-2 font-medium text-accent-fg"
          @click="confirm.accept()"
        >
          {{ t('relay.confirm.use') }}
        </button>
      </div>
    </div>
  </div>
</template>
