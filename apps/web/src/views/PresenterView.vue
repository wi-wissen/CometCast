<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import StartPage from '../components/presenter/StartPage.vue'
import WaitingPage from '../components/presenter/WaitingPage.vue'
import WhatPage from '../components/presenter/WhatPage.vue'
import FilePage from '../components/presenter/FilePage.vue'
import LinkPage from '../components/presenter/LinkPage.vue'
import PresentingPage from '../components/presenter/PresentingPage.vue'
import ErrorPage from '../components/presenter/ErrorPage.vue'
import ByePage from '../components/presenter/ByePage.vue'
import { detectCapabilities } from '../core/capabilities'
import { isTestVideo } from '../core/capture'
import { currentPlatform } from '../core/platform'
import { errorKindFor, lockedWhileWaiting, type ErrorKind } from '../core/presenterErrors'
import { isValidRoomCode } from '../core/roomCode'
import { useDeniedCooldown } from '../composables/useCooldown'
import { useRelay } from '../composables/useRelay'
import { useSender } from '../composables/useSender'
import { appUrlFor } from '../config/stores'

// The presenter (sender) side (docs/design/first-run-and-display.md, 5 and 6): start page, waiting
// for approval, what to show, presenting, errors, disconnected.
const props = defineProps<{ qrCode: string | null }>()
const { t, te } = useI18n()
type Page = 'start' | 'waiting' | 'what' | 'file' | 'link' | 'presenting' | 'error' | 'bye'
const page = ref<Page>('start')
const errorKind = ref<ErrorKind>('code')
const errorCode = ref<string | null>(null)
const presKind = ref<'screen' | 'file' | 'link'>('screen')
const presPdf = ref(false)

const platform = currentPlatform()
const caps = detectCapabilities()
const relay = useRelay()
const dom = computed(() => relay.current.value.host)
const { state, start, stop, sendImage, sendDocument, sendLink, sendCmd, shareScreen, stopScreen } = useSender()
const cooldown = useDeniedCooldown(state)

const fromQr = ref(props.qrCode !== null)
const activeCode = ref(props.qrCode ?? '')
// The display's room name comes with the protocol (join_pending / joined `room_name`). Without one the
// pages show no room at all; the code is never shown in its place.
const room = computed(() => state.value.roomName)
const busy = computed(() => page.value === 'start' && ['capturing', 'connecting', 'joining', 'negotiating', 'sharing'].includes(state.value.phase))
const screenOk = computed(() => (!platform.isPhone && caps.screenShare) || isTestVideo())
const keepBar = computed(() => presKind.value === 'screen' && platform.isLaptop && ['Chrome', 'Edge'].includes(platform.browser))
const wasWaiting = ref(false)

// Not drawn: a status line for screen readers (and for the automated tests).
const statusText = computed(() => {
  const c = state.value.errorCode
  const err = state.value.phase === 'error' && c ? ' ' + (te(`errors.${c}`) ? t(`errors.${c}`) : t('errors.unknown')) : ''
  return t(`send.status.${state.value.phase}`) + err
})

function toError(kind: ErrorKind, code: string | null = null) {
  errorKind.value = kind
  errorCode.value = code
  page.value = 'error'
}

function connect(code: string) {
  if (!isValidRoomCode(code)) return toError('code')
  activeCode.value = code
  wasWaiting.value = false
  page.value = 'start'
  void start(code, { capture: false })
}

watch(
  state,
  (s) => {
    if (s.phase === 'waiting_approval') {
      wasWaiting.value = true
      if (page.value !== 'waiting') page.value = 'waiting'
    } else if (s.phase === 'sharing' && s.channelOpen) {
      if (page.value === 'start' || page.value === 'waiting') page.value = 'what'
    } else if (s.phase === 'error') {
      toError(errorKindFor(s.errorCode, { wasWaiting: wasWaiting.value }), s.errorCode)
    } else if (s.phase === 'peer_left') {
      toError('ended', 'peer_left')
    }
    // The browser's own "stop sharing" button ended the screen: back to "What to show".
    if (page.value === 'presenting' && presKind.value === 'screen' && !s.screen) page.value = 'what'
  },
  { flush: 'sync' },
)

async function onScreen() {
  if (!screenOk.value) return toError('noScreen')
  const r = await shareScreen()
  if (r === 'ok') {
    presKind.value = 'screen'
    page.value = 'presenting'
  } else if (r === 'failed') toError('noScreen', 'capture_failed')
  // Cancelling the system dialog returns to this page without an error.
}
function turnPage(cmd: 'next_page' | 'prev_page') {
  try {
    sendCmd(cmd)
  } catch {
    /* channel closed; the state already says so */
  }
}
async function onStop() {
  if (presKind.value === 'screen') await stopScreen()
  else {
    try {
      sendCmd('stop')
    } catch {
      /* channel closed; the state already says so */
    }
  }
  page.value = 'what'
}
function started(kind: 'file' | 'link', pdf = false) {
  presKind.value = kind
  presPdf.value = pdf
  page.value = 'presenting'
}
function withdraw() {
  stop()
  page.value = 'start'
}
function disconnect() {
  stop()
  page.value = 'bye'
}
function retype() {
  fromQr.value = false
  activeCode.value = ''
  page.value = 'start'
}
function getApp() {
  const url = appUrlFor(platform.store)
  if (url) window.location.assign(url)
}
function reload() {
  window.location.reload()
}
function retry() {
  connect(activeCode.value)
}

// "No internet" retries by itself when the network comes back (and now and then).
let retryTimer: ReturnType<typeof setInterval> | null = null
function autoRetry() {
  if (page.value === 'error' && errorKind.value === 'noInternet' && navigator.onLine) retry()
}
onMounted(() => {
  window.addEventListener('online', autoRetry)
  retryTimer = setInterval(autoRetry, 10_000)
})
onBeforeUnmount(() => {
  window.removeEventListener('online', autoRetry)
  if (retryTimer) clearInterval(retryTimer)
})
</script>

<template>
  <div class="cp" data-testid="presenter" :data-page="page">
    <p class="sr-only" role="status" aria-live="polite" data-testid="status" :data-phase="state.phase" :data-ready="state.channelOpen">{{ statusText }}</p>
    <StartPage v-if="page === 'start'" :qr-code="fromQr ? activeCode : null" :dom="dom" :busy="busy" @connect="connect" />
    <WaitingPage v-else-if="page === 'waiting'" :room="room" :check="state.check" :timeout-secs="state.timeoutSecs" @withdraw="withdraw" />
    <WhatPage
      v-else-if="page === 'what'"
      :room="room"
      :screen-ok="screenOk"
      @screen="onScreen"
      @file="page = 'file'"
      @link="page = 'link'"
      @disconnect="disconnect"
    />
    <FilePage
      v-else-if="page === 'file'"
      :send="(f, p) => (f.type === 'application/pdf' || /\.pdf$/i.test(f.name) ? sendDocument(f, p) : sendImage(f, p))"
      @back="page = 'what'"
      @started="started('file', $event)"
    />
    <LinkPage v-else-if="page === 'link'" :send="sendLink" @back="page = 'what'" @started="started('link')" />
    <PresentingPage
      v-else-if="page === 'presenting'"
      :kind="presKind"
      :room="room"
      :keep-bar="keepBar"
      :sas="state.sas"
      :pdf="presKind === 'file' && presPdf"
      @stop="onStop"
      @page="turnPage"
    />
    <ErrorPage
      v-else-if="page === 'error'"
      :kind="errorKind"
      :locked-while="lockedWhileWaiting(errorCode)"
      :cooldown="cooldown"
      :code="errorCode"
      :app-offer="Boolean(appUrlFor(platform.store))"
      @retype="retype"
      @retry="retry"
      @show-file="page = 'file'"
      @show-link="page = 'link'"
      @get-app="getApp"
      @ask="retry"
      @reload="reload"
    />
    <ByePage v-else @again="retype" />
  </div>
</template>
