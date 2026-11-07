<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
// Plays one media link after the receiver's person clicked "Play". The element exists only while this
// component is mounted; on unmount it is stopped and its source is dropped so the connection closes.
// No credentials (crossorigin=anonymous), no referrer, no download button, no third-party player.
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { mediaErrorCode, type MediaErrorCode, type MediaKind } from '../core/media'

const props = defineProps<{ url: string; kind: MediaKind; host: string }>()
const emit = defineEmits<{ playing: []; error: [code: MediaErrorCode]; stop: [] }>()
const { t } = useI18n()
const el = ref<HTMLMediaElement | null>(null)
const stopBtn = ref<HTMLButtonElement | null>(null)

onMounted(async () => {
  await nextTick()
  stopBtn.value?.focus()
  // A click started this (user activation), so play() with sound is allowed; if the browser refuses
  // anyway, the controls stay and the person can press play.
  el.value?.play().catch(() => undefined)
})

function release() {
  const m = el.value
  if (!m) return
  m.pause()
  m.removeAttribute('src')
  m.load()
}
onBeforeUnmount(release)

function onError() {
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
    :aria-label="t('receive.media.label', { host })"
    @keydown="onKey"
  >
    <div class="flex items-center justify-between gap-3 px-3 py-2 text-sm">
      <span data-testid="media-host" class="truncate text-white/80">{{ t('receive.media.from', { host }) }}</span>
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
        referrerpolicy="no-referrer"
        controls
        controlslist="nodownload noremoteplayback"
        disablepictureinpicture
        playsinline
        preload="metadata"
        class="h-full w-full bg-black object-contain"
        @playing="emit('playing')"
        @error="onError"
      />
      <audio
        v-else
        ref="el"
        data-testid="media-element"
        :src="url"
        crossorigin="anonymous"
        referrerpolicy="no-referrer"
        controls
        controlslist="nodownload noremoteplayback"
        preload="metadata"
        class="w-[min(32rem,90%)]"
        @playing="emit('playing')"
        @error="onError"
      />
    </div>
  </div>
</template>
