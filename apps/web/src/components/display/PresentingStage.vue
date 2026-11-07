<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import AnnotationLayer from '../AnnotationLayer.vue'
import ContentOverlay from '../ContentOverlay.vue'
import ReceiverToolbar from '../ReceiverToolbar.vue'
import { AnnotationModel } from '../../core/annotations'
import type { ReceiverState } from '../../core/receiverSession'
import type { PageBitmap } from '../../core/pdfRenderer'
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
} from '../../core/toolbarState'

// The picture: full bleed on black, letterboxed in its own aspect ratio (spec 4.5). The received
// screen, images, links and PDF pages are shown here. `playing` tells the display that the first
// frame of a presentation is on screen (the screen is then kept awake).
const props = defineProps<{
  state: ReceiverState
  renderPage: (page: number, width: number) => Promise<PageBitmap>
}>()
const emit = defineEmits<{ close: []; prev: []; next: []; playing: [on: boolean] }>()
const { t, te } = useI18n()

const video = ref<HTMLVideoElement | null>(null)
const stage = ref<HTMLElement | null>(null)
const freezeCanvas = ref<HTMLCanvasElement | null>(null)

watch(
  () => props.state.stream,
  (s) => {
    const el = video.value
    if (!el) return
    el.srcObject = s
    if (s)
      el.play().catch(() => {
        // Autoplay can be blocked; the element is muted, so this is normally allowed.
      })
    else emit('playing', false)
  },
)

const showVideo = computed(() => props.state.stream !== null && props.state.phase === 'receiving')
const videoPlaying = ref(false)
function onPlaying() {
  videoPlaying.value = true
  onVideoSize()
  emit('playing', true)
}
watch(showVideo, (v) => {
  if (!v) {
    videoPlaying.value = false
    emit('playing', false)
  }
})

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

// New content or a departed sender invalidates annotations made on top of the old content.
const announce = ref('')
watch(
  () => props.state.content,
  (c, old) => {
    if (c?.kind === 'document' && old?.kind === 'document' && c.id === old.id) {
      // Same transfer: a page turn (annotations belong to a page, so they go) or the page count arriving.
      if (c.page !== old.page) model.reset()
    } else if (c) {
      model.reset()
      const key = c.kind === 'link' ? 'receive.announce.link' : c.kind === 'document' ? 'receive.announce.document' : 'receive.announce.image'
      const text = t(key, { name: c.kind === 'link' ? c.host : c.name })
      announce.value = old ? `${t('receive.announce.replaced')} ${text}` : text
    } else if (old) {
      announce.value = t('receive.announce.closed')
    }
  },
)
watch(
  () => props.state.phase,
  (p) => {
    if (p === 'peer_left') model.reset()
  },
)
// The presenter stopped showing the screen but stays connected.
watch(
  () => props.state.stream,
  (s) => {
    if (!s) model.reset()
  },
)

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
  const tr = props.state.transfer
  return tr ? Math.floor((tr.received / tr.size) * 100) : 0
})
const contentErrorText = computed(() => {
  const c = props.state.contentError
  return c ? (te(`receive.transferError.${c}`) ? t(`receive.transferError.${c}`) : t('receive.transferError.bad_message')) : ''
})
defineExpose({ stage })
</script>

<template>
  <div
    ref="stage"
    data-testid="stage"
    :data-frozen="toolbar.frozen"
    :aria-label="t('receive.stageLabel')"
    role="region"
    class="cd-show isolate"
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
      @playing="onPlaying"
    />
    <canvas
      v-show="toolbar.frozen"
      ref="freezeCanvas"
      data-testid="freeze-canvas"
      aria-hidden="true"
      class="absolute inset-0 z-[1] h-full w-full bg-black object-contain"
    />

    <ContentOverlay
      :content="state.content"
      :render-page="renderPage"
      @close="emit('close')"
      @prev="emit('prev')"
      @next="emit('next')"
    />
    <p class="sr-only" role="status" aria-live="polite" data-testid="announce">{{ announce }}</p>
    <AnnotationLayer :inert="state.content?.kind === 'link'" :model="model" :toolbar="toolbar" :label="t('toolbar.canvasLabel')" @changed="onModelChanged" />

    <div v-if="toolbar.black" data-testid="black-screen" class="absolute inset-0 z-[22] grid place-items-center bg-black">
      <p class="text-sm text-white/40">{{ t('toolbar.blackHint') }}</p>
    </div>

    <p
      v-if="toolbar.frozen"
      data-testid="frozen-badge"
      class="absolute left-3 top-3 z-[27] rounded-full bg-black/75 px-3 py-1 text-sm text-white"
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
      :inert="state.content?.kind === 'link'"
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
</template>
