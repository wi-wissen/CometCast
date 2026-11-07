<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import RelayConfirm from './components/RelayConfirm.vue'
import DisplayView from './views/DisplayView.vue'
import PresenterView from './views/PresenterView.vue'
import { useLocale } from './composables/useLocale'
import { parseRoute } from './core/router'

// Two pages, chosen by path (see core/router.ts): the display at /display, everything else is the
// presenter's start page (with the join code when the path is a code, as in the QR code).
const route = parseRoute(location.pathname, import.meta.env.BASE_URL)
useLocale()
</script>

<template>
  <DisplayView v-if="route.name === 'display'" />
  <PresenterView v-else :qr-code="route.code" />
  <RelayConfirm :variant="route.name === 'display' ? 'display' : 'presenter'" />
</template>
