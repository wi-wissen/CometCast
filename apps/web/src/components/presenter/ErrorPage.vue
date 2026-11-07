<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import Icon, { type IconName } from '../Icon.vue'
import type { ErrorKind } from '../../core/presenterErrors'

// Error page (spec 6): icon disc, heading, a card with "Why" and "What now", buttons at the bottom
// (primary first; laptop in a row, phone stacked).
const props = defineProps<{
  kind: ErrorKind
  /** "Locked" while waiting: the second "why" variant. */
  lockedWhile?: boolean
  /** Seconds until "ask again" is allowed after a denial. */
  cooldown: number
  /** Server error code of a failure without a page of its own. */
  code?: string | null
  /** The app can be offered (its store link exists, src/config/stores.ts); "Get the app" is drawn only then, and the texts promise an app only then. */
  appOffer?: boolean
  /** `displayOff` only: the relay's list of STUN/TURN servers failed, so the "why" says the server could not help (decision 13.45). */
  whyRelay?: boolean
}>()
const emit = defineEmits<{ retype: []; retry: []; showFile: []; showLink: []; getApp: []; ask: []; reload: [] }>()
const { t, te } = useI18n()

type Action = { id: string; label: string; primary: boolean; disabled?: boolean; event: 'retype' | 'retry' | 'showFile' | 'showLink' | 'getApp' | 'ask' | 'reload' }
const look: Record<ErrorKind, { icon: IconName; tone: 'err' | 'warn' | 'acc' | 'mut' }> = {
  code: { icon: 'x', tone: 'err' },
  expired: { icon: 'clock', tone: 'warn' },
  otherServer: { icon: 'info', tone: 'acc' },
  noInternet: { icon: 'x', tone: 'warn' },
  noScreen: { icon: 'info', tone: 'acc' },
  displayOff: { icon: 'x', tone: 'warn' },
  denied: { icon: 'x', tone: 'err' },
  locked: { icon: 'locked', tone: 'mut' },
  timeout: { icon: 'clock', tone: 'warn' },
  ended: { icon: 'info', tone: 'mut' },
  occupied: { icon: 'info', tone: 'mut' },
  busy: { icon: 'clock', tone: 'warn' },
  outdated: { icon: 'info', tone: 'mut' },
  other: { icon: 'info', tone: 'mut' },
}
const genericText = computed(() => {
  const c = props.code
  return c && te(`errors.${c}`) ? t(`errors.${c}`) : t('errors.unknown')
})
const title = computed(() => t(`error.${props.kind}.title`))
const why = computed(() => {
  if (props.kind === 'other') return genericText.value // the existing text of the failure (`errors.<code>`)
  if (props.kind === 'locked' && props.lockedWhile) return t('error.locked.whyWhile')
  if (props.kind === 'displayOff' && props.whyRelay) return t('error.displayOff.whyRelay')
  return t(`error.${props.kind}.why`)
})
const now = computed(() => {
  // Without an app to get, the page does not promise one (spec 5.5, decision 13.42).
  if (props.kind === 'noScreen' && !props.appOffer) return t('error.noScreen.nowNoApp')
  return t(`error.${props.kind}.now`)
})
const A = (id: Action['event'], key: string, primary = false, disabled = false): Action => ({ id, label: t(key), primary, disabled, event: id })
const actions = computed<Action[]>(() => {
  switch (props.kind) {
    case 'code':
    case 'expired':
    case 'ended':
      return [A('retype', 'error.action.retype', true)]
    case 'otherServer':
    case 'noInternet':
    case 'displayOff':
    case 'locked':
    case 'occupied':
    case 'busy':
    case 'other':
      return [A('retry', 'error.action.retry', true)]
    case 'outdated':
      return [A('reload', 'error.action.reload', true)]
    case 'noScreen':
      return [
        A('showFile', 'error.action.showFile', true),
        A('showLink', 'error.action.showLink'),
        ...(props.appOffer ? [A('getApp', 'error.action.getApp')] : []),
      ]
    case 'denied':
      return [
        {
          id: 'ask',
          label: props.cooldown > 0 ? t('error.action.askIn', { n: props.cooldown }) : t('error.action.ask'),
          primary: true,
          disabled: props.cooldown > 0,
          event: 'ask',
        },
      ]
    case 'timeout':
      return [A('ask', 'error.action.ask', true)]
  }
})
</script>

<template>
  <main class="cp-page" :data-kind="kind" data-testid="error-page">
    <div class="cp-ico" :class="look[kind].tone"><Icon :name="look[kind].icon" /></div>
    <h1 class="cp-h" data-testid="error-title">{{ title }}</h1>
    <div class="cp-exp">
      <dl>
        <div><dt>{{ t('error.why') }}</dt><dd data-testid="error-why">{{ why }}</dd></div>
        <div><dt>{{ t('error.now') }}</dt><dd data-testid="error-now">{{ now }}</dd></div>
      </dl>
    </div>
    <div class="cp-spacer"></div>
    <div class="cp-acts">
      <button
        v-for="a in actions"
        :key="a.id"
        type="button"
        :class="a.primary ? 'cp-pbtn' : 'cp-sbtn'"
        :disabled="a.disabled"
        :data-testid="`error-${a.id}`"
        @click="(emit as (e: string) => void)(a.event)"
      >
        {{ a.label }}
      </button>
    </div>
  </main>
</template>
