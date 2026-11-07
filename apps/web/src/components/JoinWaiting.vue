<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch, type ShallowRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { useDeniedCooldown } from '../composables/useCooldown'
import type { SenderState } from '../core/senderSession'

// Shown to a sender while the host decides (with the check number that the display shows on the
// request), and a short note after a denial (the same request may be repeated after a pause).
const props = defineProps<{ state: SenderState }>()
const emit = defineEmits<{ cancel: [] }>()
const { t } = useI18n()

const stateRef = computed(() => props.state) as unknown as ShallowRef<SenderState>
const cooldown = useDeniedCooldown(stateRef)

const waiting = computed(() => props.state.phase === 'waiting_approval')
const denied = computed(() => props.state.phase === 'error' && props.state.errorCode === 'join_denied')
const startedAt = ref(Date.now())
watch(waiting, (w) => {
  if (w) startedAt.value = Date.now()
})
const now = ref(Date.now())
const timer = setInterval(() => (now.value = Date.now()), 1000)
onBeforeUnmount(() => clearInterval(timer))
const left = computed(() => Math.max(0, (props.state.timeoutSecs ?? 60) - Math.floor((now.value - startedAt.value) / 1000)))
</script>

<template>
  <section
    v-if="waiting"
    data-testid="join-waiting"
    class="flex flex-col gap-2 rounded-cc border border-warn bg-warn-soft p-4"
  >
    <h2 class="font-semibold">{{ t('send.waiting.title') }}</h2>
    <p>{{ t('send.waiting.text') }}</p>
    <p>
      <span class="text-sm">{{ t('send.waiting.check') }}</span>
      <strong data-testid="waiting-check" class="ml-2 font-mono text-4xl">{{ state.check }}</strong>
    </p>
    <p class="text-sm text-muted">{{ t('send.waiting.checkHelp') }}</p>
    <p class="text-sm text-muted">{{ t('send.waiting.timeLeft', { s: left }) }}</p>
    <div>
      <button
        type="button"
        data-testid="cancel-join"
        class="min-h-11 rounded-cc border border-line2 bg-surface px-4 py-2 font-medium"
        @click="emit('cancel')"
      >
        {{ t('send.waiting.cancel') }}
      </button>
    </div>
  </section>
  <p v-else-if="denied && cooldown > 0" data-testid="denied-cooldown" role="status" class="text-sm text-muted">
    {{ t('send.waiting.retryIn', { s: cooldown }) }}
  </p>
</template>
