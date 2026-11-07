<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'
import { useI18n } from 'vue-i18n'

// A button that only acts after it was held for `holdMs` (pointer, touch, or Enter/Space on a
// keyboard or remote control). A short press never acts; it shows a hint. This keeps an accidental
// click, or a "Select" press that was meant for something else, from allowing anybody.
const props = withDefaults(defineProps<{ label: string; holdMs?: number; ariaLabel?: string }>(), { holdMs: 1000 })
const emit = defineEmits<{ confirm: [] }>()
const { t } = useI18n()

const progress = ref(0) // 0..1
const holding = ref(false)
const hint = ref(false)
let started = 0
let timer: ReturnType<typeof setInterval> | null = null
let hintTimer: ReturnType<typeof setTimeout> | null = null

function stopTimer() {
  if (timer) clearInterval(timer)
  timer = null
}

function begin() {
  if (holding.value) return
  holding.value = true
  hint.value = false
  started = Date.now()
  progress.value = 0
  timer = setInterval(() => {
    progress.value = Math.min(1, (Date.now() - started) / props.holdMs)
    if (progress.value >= 1) {
      stopTimer()
      holding.value = false
      progress.value = 0
      emit('confirm')
    }
  }, 40)
}

function end() {
  if (!holding.value) return
  stopTimer()
  holding.value = false
  progress.value = 0
  // Released too early: only a hint.
  hint.value = true
  if (hintTimer) clearTimeout(hintTimer)
  hintTimer = setTimeout(() => (hint.value = false), 2500)
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault()
    if (!e.repeat) begin()
  }
}
function onKeyup(e: KeyboardEvent) {
  if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault()
    end()
  }
}
onBeforeUnmount(() => {
  stopTimer()
  if (hintTimer) clearTimeout(hintTimer)
})
</script>

<template>
  <span class="relative inline-flex flex-col">
    <button
      type="button"
      data-allow
      :aria-label="ariaLabel"
      class="relative min-h-12 min-w-28 select-none overflow-hidden rounded-cc bg-accent px-4 py-2 text-lg font-medium text-accent-fg focus-visible:outline-4 focus-visible:outline-offset-2 focus-visible:outline-fg"
      @pointerdown="begin"
      @pointerup="end"
      @pointerleave="end"
      @pointercancel="end"
      @contextmenu.prevent
      @keydown="onKeydown"
      @keyup="onKeyup"
      @blur="end"
      @click.prevent
    >
      <span
        aria-hidden="true"
        data-testid="hold-bar"
        class="absolute inset-y-0 left-0 bg-white/35"
        :style="{ width: `${Math.round(progress * 100)}%` }"
      />
      <span class="relative">{{ holding ? t('join.allowHolding') : label }}</span>
    </button>
    <span v-if="hint" role="status" data-testid="hold-hint" class="mt-1 text-sm text-muted">{{ t('join.allowHint') }}</span>
  </span>
</template>
