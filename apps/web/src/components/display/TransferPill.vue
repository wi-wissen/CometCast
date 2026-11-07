<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { shortenMiddle } from '../../core/fitName'

// The transfer pill (spec 3.5, decision 13.63): one line, at most 340 wide, never cut except the file name, which is
// shortened in the middle (start and extension stay). The same pill carries `receive.document.loading`.
const props = defineProps<{ messageKey: 'receive.transfer' | 'receive.document.loading'; name: string; percent?: number }>()
const { t } = useI18n()
// 340 wide, minus 2 x 2 border and 2 x 14 padding.
const AVAILABLE = 340 - 4 - 28
const span = ref<HTMLElement | null>(null)
const keep = ref(Array.from(props.name).length)
const shown = () => shortenMiddle(props.name, keep.value)
const text = ref('')
function render() {
  text.value = t(props.messageKey, { name: shown(), percent: props.percent ?? 0 })
}
render()

async function fit() {
  const len = Array.from(props.name).length
  keep.value = len
  render()
  await nextTick()
  const el = span.value
  if (!el || el.offsetWidth <= AVAILABLE) return
  // Binary search for the longest name that fits.
  let lo = 4
  let hi = len - 1
  while (lo < hi) {
    const mid = Math.ceil((lo + hi) / 2)
    keep.value = mid
    render()
    await nextTick()
    if (el.offsetWidth <= AVAILABLE) lo = mid
    else hi = mid - 1
  }
  keep.value = lo
  render()
}
onMounted(() => void fit())
watch(() => [props.name, props.percent, props.messageKey], () => void fit())
</script>

<template>
  <p data-testid="transfer" role="status" class="cd-tag cd-transfer"><span ref="span" data-testid="transfer-text">{{ text }}</span></p>
</template>
