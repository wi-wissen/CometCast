<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
// One line at the top, centred (spec 4.6). A new toast replaces the old one. Never takes focus.
defineProps<{ text: string }>()
</script>

<template>
  <div class="cd-toast" role="status" aria-live="polite" data-testid="toast">{{ text }}</div>
</template>
