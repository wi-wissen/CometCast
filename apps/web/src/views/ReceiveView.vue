<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useReceiver } from '../composables/useReceiver'

const { t, te } = useI18n()
const { state, start } = useReceiver()
const video = ref<HTMLVideoElement | null>(null)

onMounted(start)

watch(
  () => state.value.stream,
  (s) => {
    const el = video.value
    if (!el) return
    el.srcObject = s
    if (s) el.play().catch(() => {
      // Autoplay can be blocked; the element is muted, so this is normally allowed.
    })
  },
)

const errorText = computed(() => {
  const c = state.value.errorCode
  return c ? (te(`errors.${c}`) ? t(`errors.${c}`) : t('errors.unknown')) : ''
})
const showCode = computed(() => state.value.code !== null && state.value.phase !== 'error')
const showVideo = computed(() => state.value.stream !== null && state.value.phase === 'receiving')
</script>

<template>
  <section class="flex flex-col gap-5">
    <h1 class="text-2xl font-semibold tracking-tight">{{ t('receive.title') }}</h1>
    <p v-if="!showVideo" class="text-muted">{{ t('receive.intro') }}</p>

    <div v-if="!showVideo" class="rounded-cc border border-line bg-surface p-5">
      <div class="text-xs font-semibold uppercase tracking-wider text-muted">{{ t('receive.codeLabel') }}</div>
      <div
        data-testid="room-code"
        class="mt-2 font-mono text-6xl font-semibold tracking-[0.25em]"
        :class="showCode ? 'text-fg' : 'text-muted'"
      >
        {{ showCode ? state.code : t('receive.codePlaceholder') }}
      </div>
      <p v-if="showCode" class="mt-2 text-sm text-muted">{{ t('receive.codeHelp') }}</p>
    </div>

    <p
      data-testid="status"
      :data-phase="state.phase"
      role="status"
      aria-live="polite"
      class="rounded-cc border px-3 py-2"
      :class="state.phase === 'error' ? 'border-err text-err' : 'border-line bg-surface'"
    >
      {{ t(`receive.status.${state.phase}`) }}
      <template v-if="state.phase === 'error'"> {{ errorText }}</template>
    </p>

    <div v-if="state.phase === 'error' || state.phase === 'idle'">
      <button type="button" class="rounded-cc bg-accent px-4 py-2 font-medium text-accent-fg" @click="start">
        {{ t('receive.retry') }}
      </button>
    </div>

    <video
      v-show="showVideo"
      ref="video"
      data-testid="remote-video"
      :aria-label="t('receive.videoLabel')"
      class="max-h-[80vh] w-full rounded-cc bg-black"
      autoplay
      muted
      playsinline
    />
  </section>
</template>
