<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import type { JoinRequest } from '../../core/receiverSession'
import { cleanName, shortName } from '../../core/joinPrefs'
import { isArmed } from '../../core/joinGuard'

// Join request (spec 4.4), an alertdialog. One press of Yes allows. Input is ignored for 1 s after
// the dialog appears and after it switches to the next request. Initial focus: Yes.
const props = defineProps<{ request: JoinRequest; more: number }>()
const emit = defineEmits<{ allow: []; deny: []; block: [] }>()
const { t } = useI18n()
const root = ref<HTMLElement | null>(null)

// When this request became the one on screen (the guard counts from here).
const shownAt = ref(Date.now())
const now = ref(Date.now())
const armed = computed(() => isArmed(shownAt.value, now.value))
const tick = setInterval(() => (now.value = Date.now()), 100)
onBeforeUnmount(() => clearInterval(tick))

async function focusYes() {
  await nextTick()
  root.value?.querySelector<HTMLElement>('[data-testid="request-yes"]')?.focus()
}
onMounted(focusYes)
watch(
  () => props.request.id,
  () => {
    shownAt.value = Date.now()
    now.value = shownAt.value
    void focusYes()
  },
)

/** Name: control and invisible characters stripped, whitespace collapsed, cut to 18 + "…", in quotes. */
const full = computed(() => cleanName(props.request.name))
const shown = computed(() => (full.value ? t('common.quoted', { name: shortName(full.value, 18) }) : t('common.noName')))
const aria = computed(() => t('request.aria', { name: full.value ? t('common.quoted', { name: full.value }) : t('common.noName') }))
// "wants to connect": the protocol carries no device type, so the sentence has none (decision 13.3).
const sub = computed(() => t('request.subtitle'))

function guarded(fn: () => void) {
  return () => {
    if (armed.value) fn()
  }
}
</script>

<template>
  <div class="cd-modal" data-testid="request-modal">
    <div ref="root" class="cd-wz" role="alertdialog" aria-modal="true" :aria-label="aria" :data-armed="armed">
      <div class="cd-rh">
        <h2>{{ t('request.title') }}</h2>
        <span v-if="more > 0" class="cd-queue" data-testid="request-queue">{{ t('request.queue', { n: more }) }}</span>
      </div>
      <div class="cd-rq">
        <div>
          <h2 data-testid="request-name" :title="full">{{ shown }}</h2>
          <p data-testid="request-sub">{{ sub }}</p>
        </div>
        <div class="cd-chk">
          <span>{{ t('request.check') }}</span>
          <span class="mono" data-testid="request-check">{{ request.check }}</span>
        </div>
      </div>
      <div class="cd-acts">
        <button type="button" class="cd-tbtn p" data-row="0" data-testid="request-yes" @click="guarded(() => emit('allow'))()">{{ t('request.yes') }}</button>
        <button type="button" class="cd-tbtn" data-row="0" data-testid="request-no" @click="guarded(() => emit('deny'))()">{{ t('request.no') }}</button>
        <span class="gap"></span>
        <button type="button" class="cd-ghost" data-row="0" data-testid="request-block" @click="guarded(() => emit('block'))()">{{ t('request.noBlock') }}</button>
      </div>
    </div>
  </div>
</template>
