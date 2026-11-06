<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import NavRow from '../components/NavRow.vue'
import type { ViewName } from '../types'

const { t } = useI18n()
const emit = defineEmits<{ go: [view: ViewName] }>()
</script>

<template>
  <section>
    <h1 class="mb-4 text-2xl font-semibold tracking-tight">{{ t('home.title') }}</h1>
    <div class="overflow-hidden rounded-cc border border-line bg-surface">
      <NavRow :title="t('home.sendScreen.title')" :desc="t('home.sendScreen.desc')" @click="emit('go', 'send')" />
      <NavRow :title="t('home.sendContent.title')" :desc="t('home.sendContent.desc')" @click="emit('go', 'sendContent')" />
      <NavRow :title="t('home.receive.title')" :desc="t('home.receive.desc')" @click="emit('go', 'receive')" />
    </div>
  </section>
</template>
