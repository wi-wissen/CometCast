<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, nextTick, onMounted, reactive, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import Icon from '../Icon.vue'
import { logoFor } from './logos'
import type { Theme } from '../../core/appearance'
import {
  cleanRoomName,
  initialWizard,
  NAME_SUGGESTION_KEYS,
  ROOM_NAME_MAX,
  wizardReduce,
  type DisplaySetup,
  type WizardAction,
} from '../../core/displaySetup'
import { useRelay } from '../../composables/useRelay'

// First-run setup (spec 4.1): welcome, room name, who may connect, address, done.
const props = defineProps<{ theme: Theme }>()
const emit = defineEmits<{ finish: [setup: DisplaySetup] }>()
const { t } = useI18n()
const relay = useRelay()

const w = reactive(initialWizard(t(NAME_SUGGESTION_KEYS[0])))
function act(a: WizardAction) {
  Object.assign(w, wizardReduce({ ...w }, a))
}
const root = ref<HTMLElement | null>(null)

// Address (step 3): detected, not typed. "Other address" cycles through the found options; with one
// option it opens a text field.
const choices = computed(() => relay.choices.value)
const showField = ref(false)
const fieldText = ref('')
const fieldError = ref('')
const isDefault = computed(() => relay.current.value.wsUrl === relay.default.value.wsUrl)
function otherAddress() {
  if (choices.value.length > 1) {
    const i = choices.value.findIndex((c) => c.wsUrl === relay.current.value.wsUrl)
    const next = choices.value[(i + 1) % choices.value.length]
    relay.set(next.wsUrl)
  } else {
    showField.value = !showField.value
    fieldError.value = ''
  }
}
function applyField() {
  const err = relay.set(fieldText.value)
  fieldError.value = err ? t(`relay.errors.${err}`) : ''
  if (!err) showField.value = false
}

function finish() {
  // Only an address the person chose here is kept. One that came from a link (?server=) is never
  // remembered: it has to be confirmed again (src/composables/useRelay.ts).
  const chosen = relay.current.value.source === 'user' ? relay.current.value.wsUrl : null
  emit('finish', { room: w.room || t(NAME_SUGGESTION_KEYS[0]), mode: w.mode, server: chosen })
}

// Initial focus of every step (table in 4.1).
const initialFocus: Record<number, string> = { 0: 'start', 1: 'next', 2: 'mode-selected', 3: 'ok', 4: 'finish' }
async function focusInitial() {
  await nextTick()
  const id = initialFocus[w.step]
  const el =
    id === 'mode-selected'
      ? root.value?.querySelector<HTMLElement>('[role=radio][aria-checked=true]')
      : root.value?.querySelector<HTMLElement>(`[data-testid="wz-${id}"]`)
  el?.focus()
}
onMounted(focusInitial)
watch(() => w.step, focusInitial)

/** Back (Escape / remote Back) goes one step back; on the first screen it does nothing. */
defineExpose({ back: () => act({ type: 'back' }) })
</script>

<template>
  <div ref="root" class="cd-setup" data-testid="setup">
    <div class="cd-wz">
      <div class="cd-wztop">
        <img :src="logoFor(props.theme)" alt="" />
        <span v-if="w.step > 0" class="cd-dots" data-testid="wz-steps">
          <i v-for="n in 4" :key="n" :class="{ on: n <= w.step }"></i>&#160;{{ t('common.stepOf', { n: w.step }) }}
        </span>
      </div>

      <template v-if="w.step === 0">
        <h2>{{ t('setup.welcome.title') }}</h2>
        <p>{{ t('setup.welcome.body') }}</p>
        <div class="cd-acts">
          <button type="button" class="cd-tbtn p" data-row="0" data-testid="wz-start" @click="act({ type: 'next' })">
            {{ t('setup.welcome.start') }}
          </button>
        </div>
      </template>

      <template v-else-if="w.step === 1">
        <h2>{{ t('setup.name.title') }}</h2>
        <p>{{ t('setup.name.body') }}</p>
        <div class="cd-chips">
          <button
            v-for="k in NAME_SUGGESTION_KEYS"
            :key="k"
            type="button"
            class="cd-chip"
            :class="{ on: w.room === t(k) }"
            data-row="0"
            :aria-pressed="w.room === t(k)"
            @click="act({ type: 'room', value: t(k) })"
          >
            {{ t(k) }}
          </button>
        </div>
        <div class="cd-namefield">
          <label for="wz-room">{{ t('setup.name.custom') }}</label>
          <input
            id="wz-room"
            type="text"
            autocomplete="off"
            :maxlength="ROOM_NAME_MAX"
            :value="w.room"
            data-testid="wz-room"
            @input="act({ type: 'room', value: ($event.target as HTMLInputElement).value })"
            @blur="w.room = cleanRoomName(w.room)"
          />
        </div>
        <div class="cd-acts">
          <button type="button" class="cd-tbtn" data-row="1" data-testid="wz-back" @click="act({ type: 'back' })">{{ t('common.back') }}</button>
          <button type="button" class="cd-tbtn p" data-row="1" data-testid="wz-next" @click="act({ type: 'next' })">{{ t('common.next') }}</button>
        </div>
      </template>

      <template v-else-if="w.step === 2">
        <h2>{{ t('setup.mode.title') }}</h2>
        <div class="cd-mcards" role="radiogroup" :aria-label="t('setup.mode.title')">
          <button
            type="button"
            role="radio"
            class="cd-mcard"
            :class="{ on: w.mode === 'open' }"
            :aria-checked="w.mode === 'open'"
            data-row="0"
            data-testid="wz-mode-open"
            @click="act({ type: 'mode', value: 'open' })"
          >
            <em>{{ t('setup.mode.fewPeople') }}</em>
            <b><Icon name="open" />{{ t('mode.open') }}</b>
            <span>{{ t('setup.mode.openText') }}</span>
          </button>
          <button
            type="button"
            role="radio"
            class="cd-mcard"
            :class="{ on: w.mode === 'confirm' }"
            :aria-checked="w.mode === 'confirm'"
            data-row="0"
            data-testid="wz-mode-confirm"
            @click="act({ type: 'mode', value: 'confirm' })"
          >
            <em>{{ t('setup.mode.manyPeople') }}</em>
            <b><Icon name="approval" />{{ t('mode.approval') }}</b>
            <span>{{ t('setup.mode.approvalText') }}</span>
          </button>
        </div>
        <p>{{ t('setup.mode.later') }}</p>
        <div class="cd-acts">
          <button type="button" class="cd-tbtn" data-row="1" data-testid="wz-back" @click="act({ type: 'back' })">{{ t('common.back') }}</button>
          <button type="button" class="cd-tbtn p" data-row="1" data-testid="wz-next" @click="act({ type: 'next' })">{{ t('common.next') }}</button>
        </div>
      </template>

      <template v-else-if="w.step === 3">
        <h2>{{ t('setup.address.title') }}</h2>
        <div class="cd-srvcard">
          <Icon name="globe" />
          <div>
            <p class="d" data-testid="wz-domain">{{ relay.current.value.host }}</p>
            <p class="s">{{ t(isDefault ? 'setup.address.sourceDefault' : 'setup.address.sourceOwn') }}</p>
          </div>
        </div>
        <div v-if="showField" class="cd-namefield">
          <label for="wz-addr">{{ t('setup.address.change') }}</label>
          <input id="wz-addr" v-model="fieldText" type="text" autocomplete="off" spellcheck="false" data-testid="wz-addr" @keydown.enter.prevent="applyField" />
          <p v-if="fieldError" data-testid="wz-addr-error">{{ fieldError }}</p>
        </div>
        <p>{{ t('setup.address.body') }}</p>
        <div class="cd-acts">
          <button type="button" class="cd-tbtn" data-row="0" data-testid="wz-back" @click="act({ type: 'back' })">{{ t('common.back') }}</button>
          <button type="button" class="cd-tbtn" data-row="0" data-testid="wz-change" @click="otherAddress">{{ t('setup.address.change') }}</button>
          <button type="button" class="cd-tbtn p" data-row="0" data-testid="wz-ok" @click="act({ type: 'next' })">{{ t('setup.address.ok') }}</button>
        </div>
      </template>

      <template v-else>
        <h2>{{ t('setup.done.title') }}</h2>
        <p>{{ t('setup.done.bodyPointer') }}</p>
        <div class="cd-acts">
          <button type="button" class="cd-tbtn p" data-row="0" data-testid="wz-finish" @click="finish">{{ t('setup.done.finish') }}</button>
        </div>
      </template>
    </div>
  </div>
</template>
