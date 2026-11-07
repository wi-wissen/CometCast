<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { isArmed } from '../core/joinGuard'
import { shortName } from '../core/joinPrefs'
import { JOIN_REQUEST_SECS, type JoinRequest } from '../core/receiverSession'

// One join request: the sender's name in quotes (untrusted text, shown as plain text), the check
// number, the time left, and Yes / No / No and block. The buttons ignore input for a short time
// after the request appeared (see joinGuard.ts). `big` is the takeover layout.
const props = defineProps<{ request: JoinRequest; now: number; big?: boolean }>()
const emit = defineEmits<{ allow: []; deny: []; block: [] }>()
const { t } = useI18n()

const armed = computed(() => isArmed(props.request.receivedAt, props.now))
const left = computed(() => Math.max(0, JOIN_REQUEST_SECS - Math.floor((props.now - props.request.receivedAt) / 1000)))
const btn = computed(() => (props.big ? 'min-h-16 px-6 py-3 text-2xl' : 'min-h-12 px-4 py-2 text-lg'))
const focusRing = 'focus-visible:outline-4 focus-visible:outline-offset-2 focus-visible:outline-fg'

function act(fn: () => void) {
  // Not armed yet: the press is ignored.
  if (armed.value) fn()
}
</script>

<template>
  <article
    data-testid="join-request"
    :data-check="request.check"
    :data-armed="armed"
    class="flex flex-col gap-2 rounded-cc border border-line2 bg-surface p-3"
    :class="big ? 'gap-4 p-6' : ''"
  >
    <p class="text-sm text-muted">{{ t('join.nameBoxLabel') }}</p>
    <p class="flex flex-wrap items-center gap-3">
      <span
        data-testid="request-name"
        :title="request.name"
        class="max-w-full break-words rounded-cc border border-dashed border-line2 px-3 py-1 font-semibold"
        :class="big ? 'text-3xl' : ''"
        >&ldquo;{{ shortName(request.name) }}&rdquo;</span
      >
      <span :class="big ? 'text-2xl' : 'text-base'">{{ t('join.wantsToPresent') }}</span>
    </p>
    <p>
      <strong class="font-mono" :class="big ? 'text-6xl' : 'text-2xl'" data-testid="request-check">{{ request.check }}</strong>
      <span class="ml-3 text-sm text-muted">{{ t('join.checkHelp') }}</span>
    </p>
    <div class="h-1.5 overflow-hidden rounded-full bg-sunk" aria-hidden="true">
      <div class="h-full bg-warn" :style="{ width: `${(left / JOIN_REQUEST_SECS) * 100}%` }" />
    </div>
    <p class="text-xs text-muted">{{ t('join.secondsLeft', { s: left }) }}</p>
    <div class="flex flex-wrap gap-3">
      <!-- No first: it is the safe default for focus. -->
      <button
        type="button"
        data-testid="deny"
        data-deny
        :aria-disabled="!armed"
        class="rounded-cc border border-line2 bg-surface font-medium"
        :class="[btn, focusRing, armed ? '' : 'opacity-60']"
        @click="act(() => emit('deny'))"
      >
        {{ t('join.deny') }}
      </button>
      <button
        type="button"
        data-testid="allow"
        data-allow
        :aria-disabled="!armed"
        :aria-label="`${t('join.allow')}: ${shortName(request.name)}`"
        class="rounded-cc bg-accent font-medium text-accent-fg"
        :class="[btn, focusRing, armed ? '' : 'opacity-60']"
        @click="act(() => emit('allow'))"
      >
        {{ t('join.allow') }}
      </button>
      <button
        type="button"
        data-testid="deny-block"
        :aria-disabled="!armed"
        class="rounded-cc border border-line2 bg-surface font-medium"
        :class="[btn, focusRing, armed ? '' : 'opacity-60']"
        @click="act(() => emit('block'))"
      >
        {{ t('join.denyBlock') }}
      </button>
    </div>
  </article>
</template>
