<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { ref } from 'vue'
import RelayConfirm from './components/RelayConfirm.vue'
import DisplayView from './views/DisplayView.vue'
import PresenterView from './views/PresenterView.vue'
import { useLocale } from './composables/useLocale'
import { ensureRelayTrusted, relayNeedsConfirmation } from './composables/useRelay'
import { parseRoute } from './core/router'

// Two pages, chosen by path (see core/router.ts): the display at /display, everything else is the
// presenter's start page (with the join code when the path is a code, as in the QR code).
const route = parseRoute(location.pathname, import.meta.env.BASE_URL)
useLocale()
// A relay that only a link asked for is confirmed before anything else is shown or connects (spec 7.2): until the
// person has answered, only the confirmation is drawn, so that the unconfirmed host appears nowhere else.
const confirming = ref(relayNeedsConfirmation())
if (confirming.value) void ensureRelayTrusted().then(() => (confirming.value = false))
</script>

<template>
  <RelayConfirm v-if="confirming" :variant="route.name === 'display' ? 'display' : 'presenter'" />
  <DisplayView v-else-if="route.name === 'display'" />
  <PresenterView v-else :qr-code="route.code" />
</template>
