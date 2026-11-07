<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import AppHeader from './components/AppHeader.vue'
import RelayField from './components/RelayField.vue'
import HomeView from './views/HomeView.vue'
import SendView from './views/SendView.vue'
import SendContentView from './views/SendContentView.vue'
import ReceiveView from './views/ReceiveView.vue'
import type { ViewName } from './types'

const { t } = useI18n()
// Three views switched by state; no router needed for v0.1.
const view = ref<ViewName>('home')
</script>

<template>
  <div class="flex min-h-screen flex-col">
    <AppHeader />
    <main class="mx-auto w-full max-w-3xl flex-1 px-4 py-6">
      <p class="mb-5 rounded-cc border border-line bg-accent-soft px-3 py-2 text-sm" role="note">
        {{ t('app.preview') }}
      </p>
      <RelayField v-if="view === 'home'" class="mb-5" :editable="true" />
      <div v-else class="mb-4 flex flex-wrap items-start justify-between gap-x-4 gap-y-2">
        <button type="button" class="rounded-cc text-sm text-accent underline-offset-2 hover:underline" @click="view = 'home'">
          &larr; {{ t('nav.back') }}
        </button>
        <RelayField :editable="false" />
      </div>
      <HomeView v-if="view === 'home'" @go="view = $event" />
      <SendView v-else-if="view === 'send'" />
      <SendContentView v-else-if="view === 'sendContent'" />
      <ReceiveView v-else />
    </main>
  </div>
</template>
