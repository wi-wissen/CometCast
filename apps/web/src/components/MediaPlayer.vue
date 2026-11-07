<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
// Plays one media link after the receiver's person clicked "Play". The element exists only while this
// component is mounted; on unmount it is stopped and its source is dropped so the connection closes.
// No credentials (crossorigin=anonymous), no referrer (page meta tag), no download button, no third-party player.
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { MEDIA_LOAD_TIMEOUT_MS, mediaErrorCode, shortHost, type MediaErrorCode, type MediaKind } from '../core/media'

const props = defineProps<{ url: string; kind: MediaKind; host: string }>()
const emit = defineEmits<{ playing: []; error: [code: MediaErrorCode]; stop: [] }>()
const { t } = useI18n()
const el = ref<HTMLMediaElement | null>(null)
const stopBtn = ref<HTMLButtonElement | null>(null)

let timer: ReturnType<typeof setTimeout> | undefined
function clearTimer() {
  if (timer !== undefined) clearTimeout(timer)
  timer = undefined
}

onMounted(() => {
  stopBtn.value?.focus()
  // Loading timeout: if nothing can play in time, the element is released and the notice says so.
  timer = setTimeout(() => emit('error', 'network'), MEDIA_LOAD_TIMEOUT_MS)
  // The click on "Play" is the user activation. Vue mounts this component in the same tick as the click's
  // state change, so play() runs a few milliseconds after the click, inside Chrome's activation window
  // (time-based, about 5 s). UNCERTAIN: Safari ties activation more strictly to the handler; untested.
  // A refusal (NotAllowedError) is shown as a notice; other rejections arrive as an `error` event.
  el.value?.play().catch((e: unknown) => {
    if (e instanceof DOMException && e.name === 'NotAllowedError') {
      clearTimer()
      emit('error', 'blocked')
    }
  })
})

function release() {
  const m = el.value
  if (!m) return
  m.pause()
  m.removeAttribute('src')
  m.load()
}
onBeforeUnmount(() => {
  clearTimer()
  release()
})

function onReady() {
  clearTimer()
  emit('playing')
}
function onError() {
  clearTimer()
  emit('error', mediaErrorCode(el.value?.error?.code))
}
function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    emit('stop')
    e.stopPropagation()
  }
}
</script>

<template>
  <div
    data-testid="media-player"
    :data-kind="kind"
    class="absolute inset-0 z-[26] flex flex-col bg-black text-white"
    role="region"
    :aria-label="t('receive.media.label', { host: shortHost(host) })"
    @keydown="onKey"
  >
    <div class="flex items-center justify-between gap-3 px-3 py-2 text-sm">
      <span data-testid="media-host" :title="host" class="min-w-0 truncate text-white/80">{{ t('receive.media.from', { host: shortHost(host) }) }}</span>
      <button
        ref="stopBtn"
        type="button"
        data-testid="media-stop"
        class="rounded-cc bg-white/15 px-3 py-1.5 font-medium hover:bg-white/25"
        @click="emit('stop')"
      >
        {{ t('receive.media.stop') }}
      </button>
    </div>
    <div class="grid min-h-0 flex-1 place-items-center">
      <video
        v-if="kind !== 'audio'"
        ref="el"
        data-testid="media-element"
        :src="url"
        crossorigin="anonymous"
        controls
        controlslist="nodownload noremoteplayback"
        disablepictureinpicture
        playsinline
        preload="metadata"
        class="h-full w-full bg-black object-contain"
        @playing="onReady"
        @loadeddata="clearTimer"
        @error="onError"
      />
      <audio
        v-else
        ref="el"
        data-testid="media-element"
        :src="url"
        crossorigin="anonymous"
        controls
        controlslist="nodownload noremoteplayback"
        preload="metadata"
        class="w-[min(32rem,90%)]"
        @playing="onReady"
        @loadeddata="clearTimer"
        @error="onError"
      />
    </div>
  </div>
</template>
