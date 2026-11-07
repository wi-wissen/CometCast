<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed } from 'vue'
import { qrSvg } from '../../core/qr'

// QR code (4.2): modules are always the dark blue on the light quiet zone behind it.
// The box is empty (no modules) while there is no code; the caller then does not render this at all.
const props = defineProps<{ url: string; label: string }>()
const qr = computed(() => qrSvg(props.url))
const view = computed(() => `-2 -2 ${qr.value.size + 4} ${qr.value.size + 4}`)
</script>

<template>
  <svg :viewBox="view" shape-rendering="crispEdges" role="img" :aria-label="label">
    <path :d="qr.path" fill="var(--qrdot)" />
  </svg>
</template>
