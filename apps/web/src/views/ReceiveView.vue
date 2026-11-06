<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import AnnotationLayer from '../components/AnnotationLayer.vue'
import ContentOverlay from '../components/ContentOverlay.vue'
import ReceiverToolbar from '../components/ReceiverToolbar.vue'
import { AnnotationModel } from '../core/annotations'
import {
  initialToolbar,
  moveTo,
  setColor,
  setOpen,
  setSize,
  toggleBlack,
  toggleFreeze,
  toggleTool,
  type ColorId,
  type SizeIndex,
  type Tool,
} from '../core/toolbarState'
import { useReceiver } from '../composables/useReceiver'

const { t, te } = useI18n()
const { state, start, clearContent } = useReceiver()
const video = ref<HTMLVideoElement | null>(null)
const stage = ref<HTMLElement | null>(null)
const freezeCanvas = ref<HTMLCanvasElement | null>(null)

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
// The stage also exists without a screen stream (content-only senders: images, links).
const showStage = computed(() => (state.value.phase === 'receiving' || state.value.content !== null))
const immersive = computed(() => state.value.content !== null)

// --- Toolbar, annotations (local only), freeze, black screen, fullscreen -------------------------
const toolbar = ref(initialToolbar())
const model = new AnnotationModel()
const canUndo = ref(false)
const hasStrokes = ref(false)
function onModelChanged() {
  canUndo.value = model.canUndo
  hasStrokes.value = !model.isEmpty
}

const videoSize = ref(0)
function onVideoSize() {
  videoSize.value = video.value?.videoWidth ?? 0
}
const canFreeze = computed(() => showVideo.value && videoSize.value > 0)

function applyFreeze() {
  const v = video.value
  const c = freezeCanvas.value
  if (!toolbar.value.frozen) return
  if (!v || !c || v.videoWidth === 0) {
    toolbar.value = { ...toolbar.value, frozen: false }
    return
  }
  // Paint the current frame; the video element keeps playing the live stream underneath.
  c.width = v.videoWidth
  c.height = v.videoHeight
  c.getContext('2d')?.drawImage(v, 0, 0)
}
function onFreeze() {
  toolbar.value = toggleFreeze(toolbar.value)
  applyFreeze()
}
// A frozen frame of a finished stream would be misleading.
watch(showVideo, (v) => {
  if (!v && toolbar.value.frozen) toolbar.value = { ...toolbar.value, frozen: false }
  if (!v) videoSize.value = 0
})

const isFullscreen = ref(false)
const fullscreenAvailable = typeof document !== 'undefined' && document.fullscreenEnabled === true
function onFullscreenChange() {
  isFullscreen.value = document.fullscreenElement !== null && document.fullscreenElement === stage.value
}
function onFullscreen() {
  if (document.fullscreenElement) void document.exitFullscreen().catch(() => undefined)
  else void stage.value?.requestFullscreen().catch(() => undefined)
}
onMounted(() => document.addEventListener('fullscreenchange', onFullscreenChange))
onBeforeUnmount(() => document.removeEventListener('fullscreenchange', onFullscreenChange))

const transferPercent = computed(() => {
  const tr = state.value.transfer
  return tr ? Math.floor((tr.received / tr.size) * 100) : 0
})
const contentErrorText = computed(() => {
  const c = state.value.contentError
  return c ? (te(`receive.transferError.${c}`) ? t(`receive.transferError.${c}`) : t('receive.transferError.bad_message')) : ''
})
</script>

<template>
  <section class="flex flex-col gap-5">
    <h1 class="text-2xl font-semibold tracking-tight">{{ t('receive.title') }}</h1>
    <p v-if="!showStage" class="text-muted">{{ t('receive.intro') }}</p>

    <div v-if="!showStage" class="rounded-cc border border-line bg-surface p-5">
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

    <div
      v-show="showStage"
      ref="stage"
      data-testid="stage"
      :data-frozen="toolbar.frozen"
      :aria-label="t('receive.stageLabel')"
      role="region"
      class="isolate w-full overflow-hidden bg-black"
      :class="immersive ? 'fixed inset-0 z-40' : 'relative h-[75vh] rounded-cc'"
    >
      <video
        v-show="showVideo"
        ref="video"
        data-testid="remote-video"
        :aria-label="t('receive.videoLabel')"
        class="absolute inset-0 h-full w-full bg-black object-contain"
        autoplay
        muted
        playsinline
        @loadedmetadata="onVideoSize"
        @resize="onVideoSize"
        @playing="onVideoSize"
      />
      <canvas
        v-show="toolbar.frozen"
        ref="freezeCanvas"
        data-testid="freeze-canvas"
        aria-hidden="true"
        class="absolute inset-0 z-[1] h-full w-full bg-black object-contain"
      />
      <p v-if="!showVideo && !state.content" class="absolute inset-0 grid place-items-center px-6 text-center text-white/70">
        {{ t('receive.stageWaiting') }}
      </p>

      <ContentOverlay :content="state.content" @close="clearContent" />
      <AnnotationLayer :model="model" :toolbar="toolbar" :label="t('toolbar.canvasLabel')" @changed="onModelChanged" />

      <div
        v-if="toolbar.black"
        data-testid="black-screen"
        class="absolute inset-0 z-[22] grid place-items-center bg-black"
      >
        <p class="text-sm text-white/40">{{ t('toolbar.blackHint') }}</p>
      </div>

      <p
        v-if="toolbar.frozen"
        data-testid="frozen-badge"
        class="absolute bottom-3 left-3 z-[27] rounded-full bg-black/75 px-3 py-1 text-sm text-white"
      >
        {{ t('toolbar.frozenHint') }}
      </p>
      <p
        v-if="state.transfer"
        data-testid="transfer"
        role="status"
        class="absolute bottom-3 left-1/2 z-[27] -translate-x-1/2 rounded-full bg-black/75 px-3 py-1 text-sm text-white"
      >
        {{ t('receive.transfer', { name: state.transfer.name, percent: transferPercent }) }}
      </p>
      <p
        v-else-if="state.contentError"
        data-testid="content-error"
        role="alert"
        class="absolute bottom-3 left-1/2 z-[27] max-w-[90%] -translate-x-1/2 rounded-full bg-err px-3 py-1 text-sm text-white"
      >
        {{ contentErrorText }}
      </p>

      <ReceiverToolbar
        :state="toolbar"
        :can-undo="canUndo"
        :has-strokes="hasStrokes"
        :can-freeze="canFreeze"
        :fullscreen="isFullscreen"
        :fullscreen-available="fullscreenAvailable"
        @tool="(tool: Tool) => (toolbar = toggleTool(toolbar, tool))"
        @color="(c: ColorId) => (toolbar = setColor(toolbar, c))"
        @size="(s: SizeIndex) => (toolbar = setSize(toolbar, s))"
        @undo="model.undo()"
        @clear="model.clear()"
        @freeze="onFreeze"
        @black="toolbar = toggleBlack(toolbar)"
        @fullscreen="onFullscreen"
        @open="(o: boolean) => (toolbar = setOpen(toolbar, o))"
        @move="(x: number, y: number) => (toolbar = moveTo(toolbar, x, y))"
      />
    </div>
  </section>
</template>
