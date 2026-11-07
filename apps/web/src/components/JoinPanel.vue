<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import HoldButton from './HoldButton.vue'
import { shortName } from '../core/joinPrefs'
import { JOIN_REQUEST_SECS, type JoinNotice, type JoinRequest } from '../core/receiverSession'
import { JOIN_MODES, type JoinMode } from '../core/signaling'

// The host's side of join approval: who may join (mode), who asks to join (requests), who is
// connected. Rules (docs/protocol.md): a new request never takes the focus; "Allow" needs a
// deliberate hold; "Deny" is one press; after a decision the focus goes to the next request's
// "Deny", never to "Allow". While somebody presents (`compact`) the panel is a small chip and a
// pill with the number of waiting requests and opens on demand.
const props = defineProps<{
  mode: JoinMode
  requests: JoinRequest[]
  peerName: string | null
  notice: JoinNotice | null
  compact: boolean
}>()
const emit = defineEmits<{
  mode: [mode: JoinMode]
  allow: [id: number]
  deny: [id: number]
  denyAll: []
  remove: []
  dismiss: []
}>()
const { t } = useI18n()

const MAX_SHOWN = 3
const root = ref<HTMLElement | null>(null)
const expanded = ref(false)
const showPanel = computed(() => !props.compact || expanded.value)
const shown = computed(() => props.requests.slice(0, MAX_SHOWN))
const more = computed(() => Math.max(0, props.requests.length - MAX_SHOWN))

// Countdown (one tick per second, only while somebody waits).
const now = ref(Date.now())
let timer: ReturnType<typeof setInterval> | null = null
watch(
  () => props.requests.length > 0,
  (waiting) => {
    if (waiting && !timer) timer = setInterval(() => (now.value = Date.now()), 1000)
    if (!waiting && timer) {
      clearInterval(timer)
      timer = null
    }
    if (waiting) now.value = Date.now()
  },
  { immediate: true },
)
onBeforeUnmount(() => timer && clearInterval(timer))
const secondsLeft = (r: JoinRequest) => Math.max(0, JOIN_REQUEST_SECS - Math.floor((now.value - r.receivedAt) / 1000))

// A new request is announced to screen readers, but never moves the focus.
const announce = ref('')
watch(
  () => props.requests.map((r) => r.id),
  (ids, old) => {
    const fresh = props.requests.find((r) => !(old ?? []).includes(r.id))
    if (fresh) announce.value = t('join.announce', { name: shortName(fresh.name), check: fresh.check })
  },
)

async function focusAfterDecision() {
  await nextTick()
  const el = root.value
  if (!el) return
  const next = el.querySelector<HTMLElement>('[data-deny]')
  if (next) next.focus()
  else el.querySelector<HTMLElement>('[role=radio][aria-checked=true]')?.focus()
}
function decideAllow(id: number) {
  emit('allow', id)
  void focusAfterDecision()
}
function decideDeny(id: number) {
  emit('deny', id)
  void focusAfterDecision()
}
function denyAll() {
  emit('denyAll')
  void focusAfterDecision()
}

// Radio group: arrow keys move the focus only; Enter, Space or a click selects. (Moving with a
// remote control must not change the mode by accident.)
function onRadioKey(e: KeyboardEvent, index: number) {
  const key = e.key
  const delta = key === 'ArrowRight' || key === 'ArrowDown' ? 1 : key === 'ArrowLeft' || key === 'ArrowUp' ? -1 : 0
  if (!delta) return
  e.preventDefault()
  const buttons = root.value?.querySelectorAll<HTMLElement>('[role=radio]')
  buttons?.[(index + delta + JOIN_MODES.length) % JOIN_MODES.length]?.focus()
}

function onEscape(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.compact && expanded.value) {
    e.stopPropagation()
    expanded.value = false
  }
}
</script>

<template>
  <div ref="root" data-testid="join-panel" :data-mode="mode" @keydown="onEscape">
    <p class="sr-only" role="status" aria-live="polite" data-testid="join-announce">{{ announce }}</p>

    <!-- Compact: chip and pill -->
    <div
      v-if="compact && !expanded"
      class="fixed right-3 top-3 z-[45] flex items-center gap-2"
      data-testid="join-chip-row"
    >
      <button
        type="button"
        data-testid="join-pill"
        v-if="requests.length"
        class="min-h-11 rounded-full bg-warn px-4 py-2 text-base font-semibold text-black"
        :aria-label="t('join.open')"
        @click="expanded = true"
      >
        {{ t('join.pill', { n: requests.length }) }}
      </button>
      <button
        type="button"
        data-testid="join-chip"
        class="min-h-11 rounded-full border border-line2 bg-surface/90 px-4 py-2 text-base"
        :aria-label="t('join.open')"
        @click="expanded = true"
      >
        {{ t('join.chip', { mode: t(`join.mode.${mode}.label`) }) }}
      </button>
    </div>

    <section
      v-if="showPanel"
      :aria-label="t('join.panelTitle')"
      class="flex flex-col gap-4 rounded-cc border border-line bg-surface p-4"
      :class="compact ? 'fixed right-3 top-3 z-50 max-h-[90vh] w-[min(26rem,calc(100vw-1.5rem))] overflow-auto shadow-xl' : ''"
    >
      <div class="flex items-center justify-between gap-2">
        <h2 class="text-sm font-semibold uppercase tracking-wider text-muted">{{ t('join.panelTitle') }}</h2>
        <button
          v-if="compact"
          type="button"
          class="min-h-11 rounded-cc border border-line2 px-3 py-1"
          @click="expanded = false"
        >
          {{ t('join.close') }}
        </button>
      </div>

      <p
        v-if="notice"
        role="status"
        data-testid="join-notice"
        class="flex items-start justify-between gap-2 rounded-cc border border-warn bg-warn-soft px-3 py-2 text-sm"
      >
        <span>{{ t(`join.notice.${notice.kind}`, { count: notice.kind === 'locked_discarded' ? notice.count : 0 }) }}</span>
        <button type="button" class="min-h-9 shrink-0 underline" @click="emit('dismiss')">{{ t('join.dismiss') }}</button>
      </p>

      <!-- Mode: one press each -->
      <div role="radiogroup" :aria-label="t('join.modeLabel')" class="grid grid-cols-3 gap-2">
        <button
          v-for="(m, i) in JOIN_MODES"
          :key="m"
          type="button"
          role="radio"
          :aria-checked="mode === m"
          :data-testid="`mode-${m}`"
          class="min-h-14 rounded-cc border px-2 py-2 text-lg font-medium focus-visible:outline-4 focus-visible:outline-offset-2 focus-visible:outline-fg"
          :class="mode === m ? 'border-accent bg-accent text-accent-fg' : 'border-line2 bg-surface'"
          @click="emit('mode', m)"
          @keydown="onRadioKey($event, i)"
        >
          {{ t(`join.mode.${m}.label`) }}
        </button>
      </div>
      <p class="text-sm text-muted" data-testid="mode-help">{{ t(`join.mode.${mode}.help`) }}</p>
      <button
        v-if="mode !== 'locked'"
        type="button"
        data-testid="lock-now"
        class="min-h-12 self-start rounded-cc border border-line2 px-4 py-2 font-medium focus-visible:outline-4 focus-visible:outline-offset-2 focus-visible:outline-fg"
        @click="emit('mode', 'locked')"
      >
        {{ t('join.lockNow') }}
      </button>

      <!-- Requests -->
      <div class="flex flex-col gap-3" data-testid="join-requests">
        <div class="flex items-center justify-between gap-2">
          <h3 class="font-semibold">{{ t('join.requestsTitle') }}</h3>
          <button
            v-if="requests.length >= 2"
            type="button"
            data-testid="deny-all"
            class="min-h-11 rounded-cc border border-line2 px-3 py-1 focus-visible:outline-4 focus-visible:outline-offset-2 focus-visible:outline-fg"
            @click="denyAll"
          >
            {{ t('join.denyAll') }}
          </button>
        </div>
        <p v-if="requests.length === 0" class="text-sm text-muted">{{ t('join.none') }}</p>
        <article
          v-for="r in shown"
          :key="r.id"
          data-testid="join-request"
          :data-check="r.check"
          class="flex flex-col gap-2 rounded-cc border border-line2 p-3"
        >
          <p class="text-sm text-muted">{{ t('join.nameBoxLabel') }}</p>
          <p class="flex flex-wrap items-center gap-2">
            <span
              data-testid="request-name"
              :title="r.name"
              class="max-w-full break-words rounded-cc border border-dashed border-line2 px-2 py-1 font-medium"
              >&ldquo;{{ shortName(r.name) }}&rdquo;</span
            >
            <span class="text-base">{{ t('join.wantsToPresent') }}</span>
          </p>
          <p class="text-base">
            <strong class="font-mono text-2xl" data-testid="request-check">{{ r.check }}</strong>
            <span class="ml-2 text-sm text-muted">{{ t('join.check', { check: r.check }) }}. {{ t('join.checkHelp') }}</span>
          </p>
          <div class="h-1.5 overflow-hidden rounded-full bg-sunk" aria-hidden="true">
            <div class="h-full bg-warn" :style="{ width: `${(secondsLeft(r) / JOIN_REQUEST_SECS) * 100}%` }" />
          </div>
          <p class="text-xs text-muted">{{ t('join.secondsLeft', { s: secondsLeft(r) }) }}</p>
          <div class="flex flex-wrap items-start gap-3">
            <button
              type="button"
              data-deny
              data-testid="deny"
              class="min-h-12 min-w-28 rounded-cc border border-line2 bg-surface px-4 py-2 text-lg font-medium focus-visible:outline-4 focus-visible:outline-offset-2 focus-visible:outline-fg"
              @click="decideDeny(r.id)"
            >
              {{ t('join.deny') }}
            </button>
            <HoldButton :label="t('join.allow')" :aria-label="`${t('join.allow')}: ${shortName(r.name)} (${t('join.allowHint')})`" @confirm="decideAllow(r.id)" />
          </div>
        </article>
        <p v-if="requests.length" class="text-xs text-muted">{{ t('join.allowHelp', { key: 'OK / Enter' }) }}</p>
        <p v-if="more > 0" class="text-sm text-muted" data-testid="join-more">{{ t('join.more', { n: more }) }}</p>
      </div>

      <!-- Connected -->
      <div class="flex flex-col gap-2" data-testid="join-connected">
        <h3 class="font-semibold">{{ t('join.connectedTitle') }}</h3>
        <p v-if="!peerName" class="text-sm text-muted">{{ t('join.noneConnected') }}</p>
        <div v-else class="flex flex-wrap items-center justify-between gap-2">
          <span data-testid="peer-name" class="max-w-full break-words rounded-cc border border-dashed border-line2 px-2 py-1 font-medium"
            >&ldquo;{{ shortName(peerName, 40) }}&rdquo;</span
          >
          <button
            type="button"
            data-testid="remove-peer"
            :aria-label="t('join.removeLabel', { name: shortName(peerName) })"
            class="min-h-12 rounded-cc border border-line2 px-4 py-2 font-medium focus-visible:outline-4 focus-visible:outline-offset-2 focus-visible:outline-fg"
            @click="emit('remove')"
          >
            {{ t('join.remove') }}
          </button>
        </div>
      </div>
    </section>
  </div>
</template>
