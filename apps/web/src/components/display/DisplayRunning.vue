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
import { connectionView, offlineCause, retryDelayMs } from '../../core/connectionView'
import { ControlsReveal } from '../../core/controlsReveal'
import { arrowDir, moveFocus } from '../../core/dpad'
import type { DisplaySetup } from '../../core/displaySetup'
import { IdleDim } from '../../core/idleDim'
import { cleanName, shortName } from '../../core/joinPrefs'
import type { ReceiverEvent } from '../../core/receiverSession'
import { useReceiver } from '../../composables/useReceiver'
import { splitSlot } from '../../core/textSlot'
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
      // Cannot happen through the screen (a request closes Settings, spec 4.3); no toast for it.
      return
    case 'sessionEnded':
      return showToast(t('toast.sessionEnded'))
    case 'openDisabled':
      return showToast(t('toast.openDisabled'))
    default:
      return showToast(t(`toast.${e.kind}`, { name: quoted(e.name) }))
  }
}
onBeforeUnmount(() => toastTimer && clearTimeout(toastTimer))

const { state, start, setJoinMode, decide, endRoom, clearContent, renderPage } = useReceiver({
  joinMode: props.setup.mode,
  // Presenters see this name on their waiting, "What to show" and "Presenting" pages (protocol `room_name`).
  roomName: props.setup.room,
  onEvent,
})

// A file or link that could not be shown is told as a toast (the idle screen has no other place for it).
// Watched together with its counter: the same problem twice in a row (two broken PDFs) is told twice.
watch(
  () => [state.value.contentError, state.value.contentErrorSeq] as const,
  ([c]) => {
    if (c) showToast(te(`receive.transferError.${c}`) ? t(`receive.transferError.${c}`) : t('receive.transferError.bad_message'))
  },
)

// --- what is on screen ---------------------------------------------------------------------------
const settingsOpen = ref(false)
const streamPlaying = ref(false)
const presenting = computed(() => state.value.content !== null || (state.value.stream !== null && streamPlaying.value))
const head = computed(() => state.value.requests[0] ?? null)
// A dialog over the presentation (join request, link card): name tag, End, toolbar, badge and pill are under its scrim and inert (decision 13.68).
const linkOpen = computed(() => state.value.content?.kind === 'link')
const dialogOpen = computed(() => head.value !== null || linkOpen.value)
// `{dom}` is the join address, where a browser finds the web app (decision N85), not always the relay's host.
const dom = computed(() => relay.join.value.dom)
const scheme = computed<'https' | 'http'>(() => relay.join.value.scheme)
// Not drawn: a status line for screen readers (and for the automated tests). The spec's display shows
// nothing else at rest (docs/design/implementation-questions.md).
const statusText = computed(() => {
  const c = state.value.errorCode
  const err = state.value.phase === 'error' && c ? ' ' + (te(`errors.${c}`) ? t(`errors.${c}`) : t('errors.unknown')) : ''
  return t(`receive.status.${state.value.phase}`) + err
})
// "“Lena” is showing": the protocol carries no device type, so the sentence has none (spec 4.5).
const byline = computed(() => t('present.byline', { name: quoted(state.value.peerName ?? '') }))
const sasParts = computed(() => splitSlot((m) => t('present.sas', { sas: m })))

useWakeLock(presenting)

// --- connection: starts at once; comes back by itself when the network returns --------------------
// What the idle screen shows (core/connectionView.ts): the code, "connecting" (empty code and QR box) or, once the
// retry at 3 s has failed (8 s without any code at start), the offline box. Short drops never flash the box.
let retryTimer: ReturnType<typeof setTimeout> | null = null
const lostAt = ref(Date.now()) // since when the display has no code
const failed = ref(false) // an error happened since it last had a code
const failedAttempts = ref(0) // attempts that ended in an error since it last had a code
const lastError = ref<string | null>(null)
const clock = ref(Date.now())
const hasCode = (code: string | null, phase: string) => code !== null && phase !== 'error'
watch(
  [() => state.value.code, () => state.value.phase],
  ([code, phase], [prevCode, prevPhase]) => {
    if (hasCode(code, phase)) {
      failed.value = false
      failedAttempts.value = 0
      lastError.value = null
    } else if (prevCode !== undefined && hasCode(prevCode, prevPhase as string)) lostAt.value = Date.now()
    if (phase === 'error' && prevPhase !== 'error') {
      failed.value = true
      failedAttempts.value += 1
      lastError.value = state.value.errorCode
    }
    if (retryTimer) clearTimeout(retryTimer)
    retryTimer = null
    if (phase === 'error') {
      // 1 s, 2 s, 3 s after the loss, then every 4 s; busy refusals after 30 s; an outdated page never (spec 4.2).
      const delay = retryDelayMs(state.value.errorCode, failedAttempts.value, Date.now() - lostAt.value)
      if (delay !== null) retryTimer = setTimeout(() => void start(), delay)
    }
  },
)
const clockTimer = setInterval(() => (clock.value = Date.now()), 500)
onBeforeUnmount(() => clearInterval(clockTimer))
const view = computed(() =>
  connectionView({
    code: state.value.code,
    phase: state.value.phase,
    failed: failed.value,
    failedAttempts: failedAttempts.value,
    cause: offlineCause(lastError.value),
    withoutCodeMs: clock.value - lostAt.value,
  }),
)
const cause = computed(() => offlineCause(lastError.value))
function onOnline() {
  // A page that is out of date needs a reload, not another attempt.
  if (state.value.phase === 'error' && retryDelayMs(state.value.errorCode) !== null) void start()
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
// The pointer hides together with the controls (7.2).
function setPointerHidden(hidden: boolean) {
  document.querySelector('[data-testid="display"]')?.classList.toggle('cd-hide-pointer', hidden)
}
watch([presenting, controlsShown], ([p, c]) => setPointerHidden(p && !c), { immediate: true, flush: 'post' })
onBeforeUnmount(() => setPointerHidden(false))
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
// One dialog at a time: a join request closes Settings (changes already made stay). The request dialog puts
// the focus on Yes; after the last request the Settings button gets it back (focusBase).
watch(head, (h) => {
  if (h) settingsOpen.value = false
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
    // Keys for the picture's own controls (annotation toolbar, link card, document pages) are theirs.
    const own = e.defaultPrevented || (e.target instanceof Element && e.target !== document.body && e.target.closest('[data-testid="stage"]') !== null)
    if (own) {
      controls.poke()
      return
    }
    const wasHidden = controls.poke()
    if (wasHidden) {
      // The first input only reveals (and puts the focus on End); it never activates anything.
      e.preventDefault()
      void nextTick(() => endBtn.value?.focus())
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
      :view="view"
      :cause="cause"
      :dimmed="dimmed"
      :scheme="scheme"
      @settings="settingsOpen = true"
    />
  </div>

  <PresentingStage
    v-show="presenting"
    :state="state"
    :render-page="renderPage"
    :controls-shown="controlsShown"
    :request-open="head !== null"
    @close="clearContent"
    @input="controls.poke()"
    @playing="streamPlaying = $event"
  />

  <p class="sr-only" role="status" aria-live="polite" data-testid="status" :data-phase="state.phase">{{ statusText }}</p>

  <div class="cd-hud" :style="{ zIndex: 30 }">
    <template v-if="presenting">
      <div class="cd-pv cd-byline" :class="{ hid: !controlsShown, under: linkOpen }" :inert="dialogOpen ? true : undefined" data-testid="byline" :aria-hidden="!controlsShown">
        <span data-testid="byline-name">{{ byline }}</span>
        <small v-if="state.sas" data-testid="sas-line">{{ sasParts.before }}<span class="mono" data-testid="sas-code">{{ state.sas }}</span>{{ sasParts.after }}</small>
      </div>
      <div class="cd-pv cd-endbtn" :class="{ hid: !controlsShown, under: linkOpen }" :inert="dialogOpen ? true : undefined">
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
    <!-- While the annotation toolbar is visible the toast sits 8 below it, never on top of it (spec 3.5). -->
    <ToastLayer v-if="toast" :text="toast" :low="presenting && controlsShown" />
  </div>
</template>
