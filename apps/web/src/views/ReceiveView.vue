<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
</script>

<template>
  <section class="flex flex-col gap-5">
    <h1 class="text-2xl font-semibold tracking-tight">{{ t('receive.title') }}</h1>
    <p class="text-muted">{{ t('receive.intro') }}</p>
    <div class="rounded-cc border border-dashed border-line2 bg-surface p-4">
      <div class="text-xs font-semibold uppercase tracking-wider text-muted">{{ t('receive.codeLabel') }}</div>
      <div class="mt-1 font-mono text-2xl tracking-[0.3em] text-muted" aria-live="polite">{{ t('receive.codePlaceholder') }}</div>
    </div>
    <div>
      <h2 class="mb-2 font-semibold">{{ t('receive.willTitle') }}</h2>
      <ul class="list-disc space-y-1 pl-5 text-muted">
        <li>{{ t('receive.will1') }}</li>
        <li>{{ t('receive.will2') }}</li>
        <li>{{ t('receive.will3') }}</li>
      </ul>
    </div>
  </section>
</template>
