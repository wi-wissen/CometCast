<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import Icon from '../Icon.vue'
import IdleScreen from './IdleScreen.vue'
import SettingsDialog from './SettingsDialog.vue'
import RequestDialog from './RequestDialog.vue'
import PresentingStage from './PresentingStage.vue'
import ToastLayer from './ToastLayer.vue'
import type { Appearance, Theme } from '../../core/appearance'
import { ControlsReveal } from '../../core/controlsReveal'
import { arrowDir, moveFocus } from '../../core/dpad'
import type { DisplaySetup } from '../../core/displaySetup'
import { IdleDim } from '../../core/idleDim'
import { cleanName, shortName } from '../../core/joinPrefs'
import type { ReceiverEvent } from '../../core/receiverSession'
import { useReceiver } from '../../composables/useReceiver'
import { useRelay } from '../../composables/useRelay'
import { useWakeLock } from '../../composables/useWakeLock'

// Everything the display does once it is set up: idle screen, settings, join requests, presenting.
const props = defineProps<{ setup: DisplaySetup; theme: Theme; appearance: Appearance }>()
const emit = defineEmits<{ appearance: [a: Appearance] }>()
const { t, te } = useI18n()
const relay = useRelay()

// --- toasts (4.6): one line, 3.2 s, a new one replaces the old one ------------------------------
const TOAST_MS = 3200
const toast = ref<string | null>(null)
let toastTimer: ReturnType<typeof setTimeout> | null = null
function showToast(text: string) {
  toast.value = text
  if (toastTimer) clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (toast.value = null), TOAST_MS)
}
function quoted(raw: string): string {
  const c = cleanName(raw)
  return c ? t('common.quoted', { name: shortName(c, 18) }) : t('common.noName')
}
function onEvent(e: ReceiverEvent) {
  switch (e.kind) {
    case 'lockedRejected':
      return showToast(t('toast.lockedRejected', { n: e.n }))
    case 'sessionEnded':
      return showToast(t('toast.sessionEnded'))
    default:
      return showToast(t(`toast.${e.kind}`, { name: quoted(e.name) }))
  }
}
onBeforeUnmount(() => toastTimer && clearTimeout(toastTimer))

const { state, start, setJoinMode, decide, endRoom, clearContent, nextPage, prevPage, renderPage } = useReceiver({
  joinMode: props.setup.mode,
  onEvent,
})

// --- what is on screen ---------------------------------------------------------------------------
const settingsOpen = ref(false)
const streamPlaying = ref(false)
const presenting = computed(() => state.value.content !== null || (state.value.stream !== null && streamPlaying.value))
const head = computed(() => state.value.requests[0] ?? null)
const offline = computed(() => state.value.phase === 'error')
const dom = computed(() => relay.current.value.host)
const scheme = computed<'https' | 'http'>(() => (relay.current.value.insecure ? 'http' : 'https'))
// Not drawn: a status line for screen readers (and for the automated tests). The spec's display shows
// nothing else at rest (docs/design/implementation-questions.md).
const statusText = computed(() => {
  const c = state.value.errorCode
  const err = state.value.phase === 'error' && c ? ' ' + (te(`errors.${c}`) ? t(`errors.${c}`) : t('errors.unknown')) : ''
  return t(`receive.status.${state.value.phase}`) + err
})
const byline = computed(() => t('present.byline', { name: quoted(state.value.peerName ?? ''), device: '' }).replace(/[\s·]+$/, ''))

useWakeLock(presenting)

// --- connection: starts at once; comes back by itself when the network returns --------------------
let retryTimer: ReturnType<typeof setTimeout> | null = null
const SLOW_RETRY = new Set(['banned', 'rate_limited', 'forbidden', 'server_full'])
watch(
  () => state.value.phase,
  (p) => {
    if (retryTimer) clearTimeout(retryTimer)
    retryTimer = null
    if (p === 'error') retryTimer = setTimeout(() => void start(), SLOW_RETRY.has(state.value.errorCode ?? '') ? 30_000 : 4000)
  },
)
function onOnline() {
  if (state.value.phase === 'error') void start()
}

// --- idle dimming (8.6) -----------------------------------------------------------------------------
const dimmed = ref(false)
const idleDim = new IdleDim((d) => (dimmed.value = d))
watch(presenting, (p) => idleDim.setEnabled(!p), { immediate: true })
// Joins, requests, connection changes and mode changes count as activity.
watch(
  () => [state.value.requests.length, state.value.peerName, state.value.joinMode, state.value.phase, state.value.code] as const,
  () => idleDim.activity(),
)

// --- presenting: name tag and End on input (4.5, 8.1) -----------------------------------------------
const controlsShown = ref(false)
const controls = new ControlsReveal((v) => (controlsShown.value = v))
const endBtn = ref<HTMLButtonElement | null>(null)
const idle = ref<InstanceType<typeof IdleScreen> | null>(null)
watch(presenting, (p) => {
  if (!p) controls.hideNow()
})
watch(controlsShown, (v) => {
  if (v) void nextTick(() => endBtn.value?.focus())
})
function endSession() {
  controls.hideNow()
  endRoom()
}

// --- focus --------------------------------------------------------------------------------------------
async function focusBase() {
  await nextTick()
  if (presenting.value || head.value) return
  if (settingsOpen.value) {
    document.querySelector<HTMLElement>(`[data-testid="settings-mode-${state.value.joinMode}"]`)?.focus()
  } else idle.value?.focusSettings()
}
onMounted(focusBase)
// A dialog closed or a presentation ended: the focus goes back to the Settings button (idle).
watch([settingsOpen, head, presenting], focusBase)
watch(presenting, (p) => {
  // Settings only exists over Idle.
  if (p) settingsOpen.value = false
})

function chooseMode(m: Parameters<typeof setJoinMode>[0]) {
  setJoinMode(m)
  idleDim.activity()
}

// --- keyboard, mouse, touch (7.2) -------------------------------------------------------------------------
function modalRoot(): HTMLElement {
  return (document.querySelector('[data-testid="request-modal"]') ??
    document.querySelector('[data-testid="settings-modal"]') ??
    document.querySelector('[data-testid="display"]')) as HTMLElement
}
function onKey(e: KeyboardEvent) {
  if (e.ctrlKey || e.metaKey || e.altKey) return
  const dir = arrowDir(e.key)
  const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement
  // Dimmed idle screen: the first key press only restores the brightness.
  if (dimmed.value && !presenting.value) {
    idleDim.activity()
    e.preventDefault()
    return
  }
  idleDim.activity()
  if (head.value) {
    // Join request: arrows move the focus; Back and Menu do nothing (an answer is required).
    if (dir) {
      e.preventDefault()
      moveFocus(modalRoot(), dir)
    } else if (e.key === 'Escape' || e.key.toLowerCase() === 'm') e.preventDefault()
    return
  }
  if (presenting.value) {
    const wasHidden = controls.poke()
    if (wasHidden) {
      // The first input only reveals; it never activates anything.
      e.preventDefault()
      return
    }
    if (dir) {
      e.preventDefault()
      endBtn.value?.focus()
    } else if (e.key === 'Escape') {
      e.preventDefault()
      controls.hideNow()
    }
    return
  }
  if (settingsOpen.value) {
    if (dir) {
      e.preventDefault()
      moveFocus(modalRoot(), dir)
    } else if (e.key === 'Escape' || e.key.toLowerCase() === 'm') {
      e.preventDefault()
      settingsOpen.value = false
    }
    return
  }
  // Idle.
  if (dir && !(typing && (dir === 'left' || dir === 'right'))) {
    e.preventDefault()
    moveFocus(modalRoot(), dir)
  } else if (e.key.toLowerCase() === 'm' && !typing) {
    e.preventDefault()
    settingsOpen.value = true
  }
}
function onPointerMove() {
  idleDim.activity()
  if (presenting.value) controls.mouseMove()
}
function onPointerDown() {
  idleDim.activity()
  if (presenting.value) controls.poke()
  // The page asks for fullscreen on the first click; a refusal is ignored (optional).
  if (!document.fullscreenElement && document.fullscreenEnabled && !requestedFullscreen) {
    requestedFullscreen = true
    void document.documentElement.requestFullscreen?.().catch(() => undefined)
  }
}
let requestedFullscreen = false
onMounted(() => {
  window.addEventListener('keydown', onKey)
  window.addEventListener('pointermove', onPointerMove, { passive: true })
  window.addEventListener('pointerdown', onPointerDown, { passive: true })
  window.addEventListener('touchstart', onPointerDown, { passive: true })
  window.addEventListener('online', onOnline)
  void start()
})
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey)
  window.removeEventListener('pointermove', onPointerMove)
  window.removeEventListener('pointerdown', onPointerDown)
  window.removeEventListener('touchstart', onPointerDown)
  window.removeEventListener('online', onOnline)
  if (retryTimer) clearTimeout(retryTimer)
  controls.dispose()
  idleDim.dispose()
})
</script>

<template>
  <div class="cd-canvas" v-show="!presenting">
    <IdleScreen
      ref="idle"
      :theme="theme"
      :room="setup.room"
      :mode="state.joinMode"
      :code="state.code"
      :dom="dom"
      :offline="offline"
      :dimmed="dimmed"
      :scheme="scheme"
      @settings="settingsOpen = true"
    />
  </div>

  <PresentingStage
    v-show="presenting"
    :state="state"
    :render-page="renderPage"
    @close="clearContent"
    @prev="prevPage"
    @next="nextPage"
    @playing="streamPlaying = $event"
  />

  <p class="sr-only" role="status" aria-live="polite" data-testid="status" :data-phase="state.phase">{{ statusText }}</p>

  <div class="cd-hud" :style="{ zIndex: 30 }">
    <template v-if="presenting">
      <div class="cd-pv cd-byline" :class="{ hid: !controlsShown }" data-testid="byline" :aria-hidden="!controlsShown">
        {{ byline }}
        <small v-if="state.sas" data-testid="sas-line">{{ t('sas.label') }}: <span class="mono" data-testid="sas-code">{{ state.sas }}</span></small>
      </div>
      <div class="cd-pv cd-endbtn" :class="{ hid: !controlsShown }">
        <button
          ref="endBtn"
          type="button"
          class="cd-ob"
          data-testid="end"
          :tabindex="controlsShown ? 0 : -1"
          :aria-hidden="!controlsShown"
          @click="endSession"
        >
          <Icon name="stop" />{{ t('present.end') }}
        </button>
      </div>
    </template>
    <SettingsDialog
      v-if="settingsOpen && !presenting"
      :mode="state.joinMode"
      :appearance="appearance"
      @mode="chooseMode"
      @appearance="emit('appearance', $event)"
      @close="settingsOpen = false"
    />
    <RequestDialog
      v-if="head"
      :request="head"
      :more="state.requests.length - 1"
      @allow="decide(head.id, true)"
      @deny="decide(head.id, false)"
      @block="decide(head.id, false, true)"
    />
    <ToastLayer v-if="toast" :text="toast" />
  </div>
</template>
