<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { detectCapabilities } from '../core/capabilities'
import { isTestVideo } from '../core/capture'
import { useSender } from '../composables/useSender'
import { isValidRoomCode, normalizeRoomCode, ROOM_CODE_LENGTH } from '../core/roomCode'

const { t, te } = useI18n()
const caps = detectCapabilities()

const code = ref('')
const touched = ref(false)
const valid = computed(() => isValidRoomCode(code.value))
const { state, start, stop } = useSender()
// TEST-ONLY: ?testVideo=1 replaces the screen with a canvas (see core/capture.ts).
const canShare = computed(() => caps.screenShare || isTestVideo())
const active = computed(() => ['capturing', 'connecting', 'joining', 'negotiating', 'sharing'].includes(state.value.phase))
const errorText = computed(() => {
  const c = state.value.errorCode
  return c ? (te(`errors.${c}`) ? t(`errors.${c}`) : t('errors.unknown')) : ''
})

function onSubmit() {
  if (valid.value && canShare.value && !active.value) void start(code.value)
}

function onInput(e: Event) {
  const el = e.target as HTMLInputElement
  code.value = normalizeRoomCode(el.value)
  el.value = code.value
  touched.value = true
}
</script>

<template>
  <section class="flex flex-col gap-5">
    <h1 class="text-2xl font-semibold tracking-tight">{{ t('send.title') }}</h1>

    <form class="flex flex-col gap-2" @submit.prevent="onSubmit">
      <label for="room-code" class="text-sm font-medium">{{ t('send.codeLabel') }}</label>
      <input
        id="room-code"
        :value="code"
        :disabled="active"
        type="text"
        inputmode="text"
        autocomplete="off"
        autocapitalize="characters"
        spellcheck="false"
        :maxlength="ROOM_CODE_LENGTH"
        :aria-invalid="touched && !valid"
        aria-describedby="room-code-hint room-code-status"
        class="w-full max-w-xs rounded-cc border border-line2 bg-surface px-3 py-2 font-mono text-xl uppercase tracking-[0.3em]"
        @input="onInput"
      />
      <p id="room-code-hint" class="text-sm text-muted">{{ t('send.codeHint') }}</p>
      <p id="room-code-status" class="text-sm" :class="valid ? 'text-ok' : 'text-err'" role="status">
        <template v-if="valid">{{ t('send.codeValid') }}</template>
        <template v-else-if="touched">{{ t('send.codeInvalid') }}</template>
      </p>
      <div class="flex flex-wrap items-center gap-3">
        <button
          type="submit"
          :disabled="!valid || !canShare || active"
          class="rounded-cc bg-accent px-4 py-2 font-medium text-accent-fg disabled:cursor-not-allowed disabled:opacity-50"
        >
          {{ t(state.phase === 'error' ? 'send.tryAgain' : 'send.connect') }}
        </button>
        <button
          v-if="active"
          type="button"
          data-testid="stop"
          class="rounded-cc border border-line2 bg-surface px-4 py-2 font-medium"
          @click="stop"
        >
          {{ t('send.stop') }}
        </button>
      </div>
    </form>

    <p
      data-testid="status"
      :data-phase="state.phase"
      role="status"
      aria-live="polite"
      class="rounded-cc border px-3 py-2"
      :class="state.phase === 'error' ? 'border-err text-err' : 'border-line bg-surface'"
    >
      {{ t(`send.status.${state.phase}`) }}
      <template v-if="state.phase === 'error'"> {{ errorText }}</template>
    </p>

    <div
      class="rounded-cc border p-4"
      :class="caps.screenShare ? 'border-line bg-surface' : 'border-warn bg-warn-soft'"
    >
      <h2 class="mb-1 font-semibold">{{ t('send.capTitle') }}</h2>
      <p v-if="caps.screenShare">{{ t('send.capOk') }}</p>
      <template v-else>
        <p>{{ t('send.capUnavailable') }}</p>
        <ul class="mt-2 list-disc pl-5 text-sm text-muted">
          <li v-if="!caps.secureContext">{{ t('send.capNoSecure') }}</li>
          <li v-if="!caps.getDisplayMedia">{{ t('send.capNoDisplay') }}</li>
        </ul>
      </template>
    </div>
  </section>
</template>
