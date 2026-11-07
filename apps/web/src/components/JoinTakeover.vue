<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import JoinRequestCard from './JoinRequestCard.vue'
import type { JoinRequest } from '../core/receiverSession'

// An incoming join request takes over the display: a large dialog on the idle page (code page),
// a prominent banner at the top while somebody presents. Yes / No / No and block. When it appears
// the focus goes to "No" (the safe choice) and the buttons ignore input for a moment (see
// joinGuard.ts). Escape or "Later" hides it without deciding; the request stays in the join menu.
const props = defineProps<{ request: JoinRequest | null; more: number; banner: boolean }>()
const emit = defineEmits<{ allow: []; deny: []; block: []; denyAll: []; later: [] }>()
const { t } = useI18n()

const root = ref<HTMLElement | null>(null)
const now = ref(Date.now())
let timer: ReturnType<typeof setInterval> | null = null
let returnFocus: HTMLElement | null = null

watch(
  () => props.request?.id ?? null,
  async (id, old) => {
    if (id !== null && old == null) {
      // Appeared: remember where the focus was, and move it to "No".
      returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    }
    if (id !== null) {
      now.value = Date.now()
      if (!timer) timer = setInterval(() => (now.value = Date.now()), 200)
      await nextTick()
      root.value?.querySelector<HTMLElement>('[data-deny]')?.focus()
    } else {
      if (timer) clearInterval(timer)
      timer = null
      await nextTick()
      if (returnFocus && document.contains(returnFocus)) returnFocus.focus()
      returnFocus = null
    }
  },
  { immediate: true },
)
onBeforeUnmount(() => timer && clearInterval(timer))

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    e.preventDefault()
    e.stopPropagation()
    emit('later')
  }
}
</script>

<template>
  <div
    v-if="request"
    ref="root"
    data-testid="join-takeover"
    :data-banner="banner"
    :role="banner ? 'alert' : 'alertdialog'"
    :aria-modal="banner ? undefined : 'true'"
    :aria-label="t('join.takeoverTitle')"
    class="fixed z-[60]"
    :class="banner ? 'inset-x-0 top-0 p-3' : 'inset-0 grid place-items-center overflow-auto bg-black/75 p-4'"
    @keydown="onKey"
  >
    <div class="w-full rounded-cc border border-warn bg-surface shadow-2xl" :class="banner ? 'mx-auto max-w-3xl p-3' : 'max-w-2xl p-6'">
      <h2 class="mb-2 font-semibold" :class="banner ? 'text-lg' : 'text-3xl'">{{ t('join.takeoverTitle') }}</h2>
      <JoinRequestCard
        :request="request"
        :now="now"
        :big="!banner"
        @allow="emit('allow')"
        @deny="emit('deny')"
        @block="emit('block')"
      />
      <div class="mt-3 flex flex-wrap items-center gap-3">
        <span v-if="more > 0" data-testid="takeover-more" class="text-base">{{ t('join.more', { n: more }) }}</span>
        <button
          v-if="more > 0"
          type="button"
          data-testid="deny-all"
          class="min-h-11 rounded-cc border border-line2 px-3 py-1 focus-visible:outline-4 focus-visible:outline-offset-2 focus-visible:outline-fg"
          @click="emit('denyAll')"
        >
          {{ t('join.denyAll') }}
        </button>
        <button
          type="button"
          data-testid="later"
          class="min-h-11 rounded-cc border border-line2 px-3 py-1 focus-visible:outline-4 focus-visible:outline-offset-2 focus-visible:outline-fg"
          @click="emit('later')"
        >
          {{ t('join.later') }}
        </button>
      </div>
    </div>
  </div>
</template>
