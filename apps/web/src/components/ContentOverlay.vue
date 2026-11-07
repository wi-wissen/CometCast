<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
// Content sent by the presenter: an image or a PDF page (the picture only, nothing drawn over it: no close
// button, no page buttons; it goes away when the presenter stops or leaves) or a link. The display never opens a
// page by itself: the link card is a dialog in the style of the join request (spec 4.5). Open (primary, initial
// focus) and Dismiss (secondary), input ignored for 1 s, Back / Escape = Dismiss, focus row [Open, Dismiss].
// A media link shows its player inside the same card.
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import type { ReceivedContent } from '../core/receiverSession'
import type { PageBitmap } from '../core/pdfRenderer'
import { arrowDir, moveFocus } from '../core/dpad'
import { isArmed } from '../core/joinGuard'
import DocumentView from './DocumentView.vue'
import MediaPlayer from './MediaPlayer.vue'
import { canPlayHlsNatively, classifyMediaUrl, initialPlayer, isMixedContent, playerReduce, shortHost, urlHostIsLocalOrIp, type MediaErrorCode, type PlayerEvent } from '../core/media'

const props = defineProps<{ content: ReceivedContent | null; renderPage: (page: number, width: number) => Promise<PageBitmap> }>()
const emit = defineEmits<{ close: [] }>()
const { t } = useI18n()
const openBtn = ref<HTMLAnchorElement | null>(null)
const card = ref<HTMLElement | null>(null)
const playBtn = ref<HTMLButtonElement | null>(null)

// Media links: a link whose path ends in a media extension gets a "Play" button. One player at a time;
// new content or a close always returns to the card, which removes (and releases) the element.
const player = ref(initialPlayer)
function dispatch(e: PlayerEvent) {
  player.value = playerReduce(player.value, e)
}
const mediaKind = computed(() => (props.content?.kind === 'link' ? classifyMediaUrl(props.content.url) : null))
const hlsOk = computed(() => mediaKind.value !== 'hls' || canPlayHlsNatively())
const mixed = computed(() => props.content?.kind === 'link' && mediaKind.value !== null && isMixedContent(typeof location === 'undefined' ? '' : location.protocol, props.content.url))
const canPlay = computed(() => mediaKind.value !== null && hlsOk.value && !mixed.value)
const hostWarn = computed(() => canPlay.value && props.content?.kind === 'link' && urlHostIsLocalOrIp(props.content.url))
const showPlayer = computed(() => props.content?.kind === 'link' && mediaKind.value !== null && (player.value.phase === 'loading' || player.value.phase === 'playing'))
const mediaError = computed(() => (player.value.phase === 'error' ? player.value.code : null))
function onMediaError(code: MediaErrorCode) {
  dispatch({ type: 'error', code })
  void nextTick(() => playBtn.value?.focus())
}
function onMediaStop() {
  // Stop closes the whole link content, like Dismiss.
  dispatch({ type: 'stop' })
  emit('close')
}

// --- the 1 s guard (the same rule as the join request, core/joinGuard.ts) --------------------------------
const shownAt = ref(Date.now())
const now = ref(Date.now())
const armed = computed(() => isArmed(shownAt.value, now.value))
const tick = setInterval(() => (now.value = Date.now()), 100)
onBeforeUnmount(() => clearInterval(tick))

watch(
  () => props.content,
  () => dispatch({ type: 'stop' }),
)
watch(
  () => props.content,
  async (c, old) => {
    if (c?.kind !== 'link') return
    // A new link starts the guard again; the focus goes to Open (the guard protects against a stray press).
    if (old?.kind !== 'link' || old.url !== c.url) {
      shownAt.value = Date.now()
      now.value = shownAt.value
    }
    await nextTick()
    openBtn.value?.focus()
  },
  { immediate: true },
)

function guarded(fn: () => void) {
  return () => {
    if (armed.value) fn()
  }
}
function onOpen(e: MouseEvent) {
  if (!armed.value) {
    e.preventDefault()
    return
  }
  // Remove the card only after the browser has started the navigation of the clicked anchor.
  setTimeout(() => emit('close'), 0)
}

/** Back / Escape = Dismiss; arrows move between Open and Dismiss; Tab stays inside the card. */
function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.content?.kind === 'link') {
    e.preventDefault()
    e.stopPropagation()
    guardedDismiss()
    return
  }
  const dir = arrowDir(e.key)
  if (dir && card.value) {
    e.preventDefault()
    moveFocus(card.value, dir)
    return
  }
  if (e.key === ' ' && e.target instanceof HTMLAnchorElement) {
    e.preventDefault()
    e.target.click()
  } else if (e.key === 'Tab') trapTab(e)
}
function guardedDismiss() {
  if (armed.value) emit('close')
}
function trapTab(e: KeyboardEvent) {
  const els = card.value?.querySelectorAll<HTMLElement>('a[href], button:not(:disabled)')
  const first = els?.[0]
  const last = els?.[els.length - 1]
  if (!first || !last) return
  if (e.shiftKey && document.activeElement === first) {
    last.focus()
    e.preventDefault()
  } else if (!e.shiftKey && document.activeElement === last) {
    first.focus()
    e.preventDefault()
  }
}
</script>

<template>
  <!-- Image and PDF page sit below the annotation layer (z-10 < layer z-20); the link card is a reference-px dialog above it. -->
  <template v-if="content">
    <DocumentView v-if="content.kind === 'document'" :content="content" :render="renderPage" />
    <div v-else-if="content.kind === 'image'" class="absolute inset-0 z-10 bg-black">
      <img
        :src="content.url"
        :alt="t('receive.content.imageAlt', { name: content.name })"
        data-testid="received-image"
        class="h-full w-full object-contain"
      />
    </div>
    <div v-else-if="content.kind === 'link'" class="cd-hud" style="z-index: 26">
      <div class="cd-modal" data-testid="link-modal" @keydown="onKey">
        <div
          ref="card"
          class="cd-wz cd-link"
          role="alertdialog"
          aria-modal="true"
          aria-labelledby="link-card-title"
          aria-describedby="link-card-desc"
          :data-armed="armed"
          data-testid="link-card"
        >
          <div class="cd-rh">
            <h2 id="link-card-title">{{ t('receive.link.title', { host: content.host }) }}</h2>
          </div>
          <p data-testid="link-url" class="cd-addr mono">{{ content.url }}</p>
          <MediaPlayer
            v-if="showPlayer && mediaKind"
            :url="content.url"
            :kind="mediaKind"
            :host="content.host"
            @playing="dispatch({ type: 'playing' })"
            @error="onMediaError"
            @stop="onMediaStop"
          />
          <template v-else>
            <p id="link-card-desc" class="cd-body">{{ t('receive.link.hint') }}</p>
            <p v-if="canPlay" data-testid="media-hint" class="cd-body">{{ t('receive.media.hint', { host: content.host }) }}</p>
            <p v-else-if="mixed" data-testid="media-mixed" class="cd-body">{{ t('receive.media.mixed') }}</p>
            <p v-else-if="mediaKind === 'hls'" data-testid="media-hls-unsupported" class="cd-body">{{ t('receive.media.hlsUnsupported') }}</p>
            <p v-if="hostWarn" data-testid="media-host-warning" class="cd-body cd-strong">{{ t('receive.media.localWarning') }}</p>
            <p v-if="mediaError" data-testid="media-error" role="alert" class="cd-body cd-strong">
              {{ t(`receive.media.error.${mediaError}`, { host: content.host }) }}
            </p>
            <div class="cd-acts cd-actsl">
              <button
                v-if="canPlay"
                ref="playBtn"
                type="button"
                class="cd-tbtn"
                data-row="0"
                data-testid="media-play"
                :title="content.host"
                @click="guarded(() => dispatch({ type: 'play' }))()"
              >
                {{ mediaError ? t('receive.media.retry') : t('receive.media.play', { host: shortHost(content.host) }) }}
              </button>
              <a
                ref="openBtn"
                :href="content.url"
                target="_blank"
                rel="noopener noreferrer"
                class="cd-tbtn p cd-anchor"
                data-row="0"
                data-testid="link-open"
                @click="onOpen"
              >
                {{ t('receive.link.open') }}
              </a>
              <button type="button" class="cd-tbtn" data-row="0" data-testid="link-dismiss" @click="guardedDismiss">
                {{ t('receive.link.dismiss') }}
              </button>
            </div>
          </template>
        </div>
      </div>
    </div>
  </template>
</template>
