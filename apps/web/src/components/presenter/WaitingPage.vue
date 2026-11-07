<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { useI18n } from 'vue-i18n'

// Waiting for approval (spec 5.4): room label, heading, check number, a 6 px progress bar that counts
// down (default 60 s) and the way out.
// `room` is the display's room name from join_pending; without one the row shows the spinner only (never the code).
const props = defineProps<{ room?: string; check: number | undefined; timeoutSecs: number | undefined }>()
const emit = defineEmits<{ withdraw: [] }>()
const { t } = useI18n()
const total = computed(() => props.timeoutSecs ?? 60)
const startedAt = Date.now()
const now = ref(Date.now())
const timer = setInterval(() => (now.value = Date.now()), 250)
onBeforeUnmount(() => clearInterval(timer))
const left = computed(() => Math.max(0, Math.ceil(total.value - (now.value - startedAt) / 1000)))
const pct = computed(() => Math.max(0, Math.min(100, (left.value / total.value) * 100)))
</script>

<template>
  <main class="cp-page" data-testid="waiting-page">
    <div class="cp-row"><div class="cp-spin" aria-hidden="true"></div><span v-if="room" class="cp-lbl" data-testid="waiting-room">{{ room }}</span></div>
    <h1 class="cp-h">{{ t('wait.title') }}</h1>
    <div class="cp-waitcard">
      <span class="cp-lbl">{{ t('wait.checkLabel') }}</span>
      <span class="big mono" data-testid="waiting-check">{{ check }}</span>
      <p class="cp-small" style="text-align: center">{{ t('wait.checkHint') }}</p>
    </div>
    <div class="cp-field">
      <div class="cp-pbar" role="progressbar" :aria-valuemin="0" :aria-valuemax="total" :aria-valuenow="left"><span :style="{ width: pct + '%' }"></span></div>
      <p class="cp-small" data-testid="waiting-left">{{ t('wait.expires', { n: left }) }}</p>
    </div>
    <div class="cp-spacer"></div>
    <button type="button" class="cp-sbtn" data-testid="withdraw" @click="emit('withdraw')">{{ t('wait.withdraw') }}</button>
  </main>
</template>
