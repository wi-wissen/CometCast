<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
// One line at the top, centred (spec 4.6). A new toast replaces the old one. Never takes focus. `low`: the
// annotation toolbar is visible, so the toast sits 8 below it instead of on top of it.
defineProps<{ text: string; low?: boolean }>()
</script>

<template>
  <div class="cd-toast" :class="{ low }" role="status" aria-live="polite" data-testid="toast">{{ text }}</div>
</template>
