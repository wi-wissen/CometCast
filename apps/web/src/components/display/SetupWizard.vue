<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, nextTick, onMounted, reactive, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import Icon from '../Icon.vue'
import { logoFor } from './logos'
import type { Theme } from '../../core/appearance'
import {
  addressSourceKey,
  chipSelected,
  cleanRoomName,
  initialWizard,
  nextAddressStep,
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

// Address (step 3): the relay the page uses. "Other address" goes through the relays of config.json, then opens a
// text field that replaces the card in place; OK / Enter confirms it (the card then says "Entered by hand."),
// Back cancels it.
const choices = computed(() => relay.choices.value)
const showField = ref(false)
const fieldText = ref('')
const fieldError = ref('')
const typed = ref(false) // the address in use was entered in the field
const sourceKey = computed(() =>
  addressSourceKey({
    typed: typed.value,
    source: relay.current.value.source,
    inChoices: choices.value.some((c) => c.wsUrl === relay.current.value.wsUrl),
  }),
)
function otherAddress() {
  const step = nextAddressStep(
    choices.value.map((c) => c.wsUrl),
    relay.current.value.wsUrl,
    typed.value,
  )
  if (step.kind === 'choice') {
    relay.set(step.ws)
    typed.value = false
  } else {
    showField.value = true
    fieldError.value = ''
    void nextTick(() => root.value?.querySelector<HTMLElement>('[data-testid="wz-addr"]')?.focus())
  }
}
function applyField() {
  const err = relay.set(fieldText.value)
  fieldError.value = err ? t(`relay.errors.${err}`) : ''
  if (!err) {
    typed.value = true
    showField.value = false
    fieldText.value = ''
    void focusInitial()
  }
}
function cancelField() {
  showField.value = false
  fieldError.value = ''
  // Back closes the field and the focus goes to "Other address" (spec 4.1).
  void nextTick(() => root.value?.querySelector<HTMLElement>('[data-testid="wz-change"]')?.focus())
}
function onAddressNext() {
  if (showField.value) applyField()
  else act({ type: 'next' })
}
function onAddressBack() {
  if (showField.value) cancelField()
  else act({ type: 'back' })
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

/** Back (Escape / remote Back) goes one step back; on the first screen it does nothing; an open address field closes first. */
defineExpose({ back: () => (showField.value ? cancelField() : act({ type: 'back' })) })
</script>

<template>
  <!-- Steps 1 to 4: the card's top edge is fixed (64) so that logo and step dots do not jump; the welcome card is centred. -->
  <div ref="root" class="cd-setup" :class="{ top: w.step > 0 }" data-testid="setup">
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
            :class="{ on: chipSelected(w, t(k)) }"
            data-row="0"
            :aria-pressed="chipSelected(w, t(k))"
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
            :value="w.custom"
            data-row="1"
            data-testid="wz-room"
            @input="act({ type: 'custom', value: ($event.target as HTMLInputElement).value })"
            @blur="w.custom !== '' && act({ type: 'custom', value: cleanRoomName(w.custom) })"
          />
        </div>
        <div class="cd-acts">
          <button type="button" class="cd-tbtn" data-row="2" data-testid="wz-back" @click="act({ type: 'back' })">{{ t('common.back') }}</button>
          <button type="button" class="cd-tbtn p" data-row="2" data-testid="wz-next" @click="act({ type: 'next' })">{{ t('common.next') }}</button>
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
        <div v-if="showField" class="cd-namefield">
          <label for="wz-addr">{{ t('setup.address.fieldLabel') }}</label>
          <input id="wz-addr" v-model="fieldText" type="text" autocomplete="off" spellcheck="false" data-row="0" data-testid="wz-addr" @keydown.enter.prevent="applyField" />
          <!-- An invalid address: the existing text, 16 --fg, led by the info icon in --acc (no red). -->
          <p v-if="fieldError" class="cd-warn" role="alert" data-testid="wz-addr-error"><Icon name="info" /><span>{{ fieldError }}</span></p>
        </div>
        <div v-else class="cd-srvcard">
          <Icon name="globe" />
          <div>
            <p class="d" data-testid="wz-domain">{{ relay.join.value.dom }}</p>
            <p class="s" data-testid="wz-source">{{ t(sourceKey) }}</p>
          </div>
        </div>
        <p>{{ t('setup.address.body') }}</p>
        <div class="cd-acts">
          <button type="button" class="cd-tbtn" :data-row="showField ? 1 : 0" data-testid="wz-back" @click="onAddressBack">{{ t('common.back') }}</button>
          <button v-if="!showField" type="button" class="cd-tbtn" data-row="0" data-testid="wz-change" @click="otherAddress">{{ t('setup.address.change') }}</button>
          <button type="button" class="cd-tbtn p" :data-row="showField ? 1 : 0" data-testid="wz-ok" @click="onAddressNext">{{ t('setup.address.ok') }}</button>
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
