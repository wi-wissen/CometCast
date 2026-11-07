<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import JoinRequestCard from './JoinRequestCard.vue'
import { shortName } from '../core/joinPrefs'
import type { BlockedEntry, JoinNotice, JoinRequest } from '../core/receiverSession'
import { JOIN_MODES, type JoinMode } from '../core/signaling'

// The host's menu: who may join (mode), requests that are not on the takeover, who is connected
// (end the presentation, with or without blocking), the block list, and "End session". On the code
// page it is a column next to the code. While somebody presents (`compact`) it is a small chip that
// appears when something moves and fades out, and opens into a panel on demand.
const props = defineProps<{
  mode: JoinMode
  requests: JoinRequest[]
  peerName: string | null
  blocks: BlockedEntry[]
  notice: JoinNotice | null
  compact: boolean
  /** The request that the takeover shows right now (not repeated in the list). */
  takeoverId?: number | null
  /** Compact layout: the chip is shown (it fades out when nothing happens). */
  chipVisible?: boolean
}>()
const emit = defineEmits<{
  mode: [mode: JoinMode]
  allow: [id: number]
  deny: [id: number]
  block: [id: number]
  remove: [block: boolean]
  unblock: [id: number]
  endSession: []
  dismiss: []
}>()
const { t } = useI18n()

const MAX_SHOWN = 3
const root = ref<HTMLElement | null>(null)
const expanded = ref(false)
const showPanel = computed(() => !props.compact || expanded.value)
const listed = computed(() => props.requests.filter((r) => r.id !== props.takeoverId))
const shown = computed(() => listed.value.slice(0, MAX_SHOWN))
const more = computed(() => Math.max(0, listed.value.length - MAX_SHOWN))

const now = ref(Date.now())
let timer: ReturnType<typeof setInterval> | null = null
watch(
  () => listed.value.length > 0,
  (waiting) => {
    if (waiting && !timer) timer = setInterval(() => (now.value = Date.now()), 200)
    if (!waiting && timer) {
      clearInterval(timer)
      timer = null
    }
    if (waiting) now.value = Date.now()
  },
  { immediate: true },
)
onBeforeUnmount(() => timer && clearInterval(timer))

async function focusAfterDecision() {
  await nextTick()
  const el = root.value
  if (!el) return
  const next = el.querySelector<HTMLElement>('[data-deny]')
  if (next) next.focus()
  else el.querySelector<HTMLElement>('[role=radio][aria-checked=true]')?.focus()
}
const decide = (fn: () => void) => {
  fn()
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

// "End session" needs a second press within a few seconds (it drops everybody).
const confirmEnd = ref(false)
let endTimer: ReturnType<typeof setTimeout> | null = null
function onEnd() {
  if (!confirmEnd.value) {
    confirmEnd.value = true
    endTimer = setTimeout(() => (confirmEnd.value = false), 4000)
    return
  }
  if (endTimer) clearTimeout(endTimer)
  confirmEnd.value = false
  emit('endSession')
}
onBeforeUnmount(() => endTimer && clearTimeout(endTimer))

const ring = 'focus-visible:outline-4 focus-visible:outline-offset-2 focus-visible:outline-fg'
</script>

<template>
  <div ref="root" data-testid="join-panel" :data-mode="mode" @keydown="onEscape">
    <!-- Compact: a chip that appears on activity and fades out -->
    <div
      v-if="compact && !expanded"
      class="fixed right-3 top-3 z-[45] flex items-center gap-2 transition-opacity duration-500"
      :class="chipVisible ? 'opacity-100' : 'pointer-events-none opacity-0'"
      :inert="!chipVisible"
      data-testid="join-chip-row"
      :data-visible="chipVisible"
    >
      <button
        type="button"
        data-testid="join-chip"
        class="min-h-11 rounded-full border border-line2 bg-surface/90 px-4 py-2 text-base"
        :class="ring"
        :aria-label="t('join.open')"
        @click="expanded = true"
      >
        {{ t('join.chip', { mode: t(`join.mode.${mode}.label`) }) }}
        <template v-if="requests.length"> &middot; {{ t('join.pill', { n: requests.length }) }}</template>
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
        <button v-if="compact" type="button" class="min-h-11 rounded-cc border border-line2 px-3 py-1" :class="ring" @click="expanded = false">
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
          class="min-h-14 rounded-cc border px-2 py-2 text-lg font-medium"
          :class="[ring, mode === m ? 'border-accent bg-accent text-accent-fg' : 'border-line2 bg-surface']"
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
        class="min-h-12 self-start rounded-cc border border-line2 px-4 py-2 font-medium"
        :class="ring"
        @click="emit('mode', 'locked')"
      >
        {{ t('join.lockNow') }}
      </button>

      <!-- Requests that are not on the takeover (hidden with "Later") -->
      <div class="flex flex-col gap-3" data-testid="join-requests">
        <h3 class="font-semibold">{{ t('join.requestsTitle') }}</h3>
        <p v-if="requests.length === 0" class="text-sm text-muted">{{ t('join.none') }}</p>
        <JoinRequestCard
          v-for="r in shown"
          :key="r.id"
          :request="r"
          :now="now"
          @allow="decide(() => emit('allow', r.id))"
          @deny="decide(() => emit('deny', r.id))"
          @block="decide(() => emit('block', r.id))"
        />
        <p v-if="more > 0" class="text-sm text-muted" data-testid="join-more">{{ t('join.more', { n: more }) }}</p>
      </div>

      <!-- Connected -->
      <div class="flex flex-col gap-2" data-testid="join-connected">
        <h3 class="font-semibold">{{ t('join.connectedTitle') }}</h3>
        <p v-if="!peerName" class="text-sm text-muted">{{ t('join.noneConnected') }}</p>
        <template v-else>
          <span data-testid="peer-name" class="max-w-full self-start break-words rounded-cc border border-dashed border-line2 px-2 py-1 font-medium"
            >&ldquo;{{ shortName(peerName, 40) }}&rdquo;</span
          >
          <div class="flex flex-wrap gap-2">
            <button
              type="button"
              data-testid="remove-peer"
              :aria-label="t('join.removeLabel', { name: shortName(peerName) })"
              class="min-h-12 rounded-cc border border-line2 px-4 py-2 font-medium"
              :class="ring"
              @click="emit('remove', false)"
            >
              {{ t('join.endPresentation') }}
            </button>
            <button
              type="button"
              data-testid="remove-block"
              class="min-h-12 rounded-cc border border-line2 px-4 py-2 font-medium"
              :class="ring"
              @click="emit('remove', true)"
            >
              {{ t('join.endAndBlock') }}
            </button>
          </div>
        </template>
      </div>

      <!-- Block list -->
      <div class="flex flex-col gap-2" data-testid="join-blocks">
        <h3 class="font-semibold">{{ t('join.blockedTitle') }}</h3>
        <p v-if="blocks.length === 0" class="text-sm text-muted">{{ t('join.noneBlocked') }}</p>
        <ul v-else class="flex flex-col gap-2">
          <li v-for="b in blocks" :key="b.id" data-testid="blocked-entry" class="flex flex-wrap items-center justify-between gap-2">
            <span class="max-w-full break-words rounded-cc border border-dashed border-line2 px-2 py-1">&ldquo;{{ shortName(b.name) }}&rdquo;</span>
            <button
              type="button"
              data-testid="unblock"
              class="min-h-11 rounded-cc border border-line2 px-3 py-1"
              :class="ring"
              @click="emit('unblock', b.id)"
            >
              {{ t('join.unblock') }}
            </button>
          </li>
        </ul>
        <p class="text-xs text-muted">{{ t('join.blockedHelp') }}</p>
      </div>

      <!-- End the room session -->
      <div class="flex flex-col gap-1 border-t border-line pt-3">
        <button
          type="button"
          data-testid="end-session"
          class="min-h-12 self-start rounded-cc border border-err px-4 py-2 font-medium text-err"
          :class="ring"
          @click="onEnd"
        >
          {{ confirmEnd ? t('join.endSessionConfirm') : t('join.endSession') }}
        </button>
        <p class="text-xs text-muted">{{ t('join.endSessionHelp') }}</p>
      </div>
    </section>
  </div>
</template>
