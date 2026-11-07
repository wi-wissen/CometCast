<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { LOCALES, type Locale } from '../i18n'
import { useLocale } from '../composables/useLocale'

const { t } = useI18n()
const { locale, setLocale } = useLocale()
const base = import.meta.env.BASE_URL
</script>

<template>
  <header class="border-b border-line bg-surface">
    <div class="mx-auto flex max-w-3xl items-center justify-between gap-3 px-4 py-3">
      <a :href="base" class="flex items-center gap-2.5 rounded-cc font-semibold tracking-tight">
        <img :src="`${base}cometcast-logo-small.svg`" alt="" width="32" height="32" class="h-8 w-8" />
        <span>{{ t('app.name') }}</span>
      </a>
      <div role="group" :aria-label="t('lang.label')" class="flex overflow-hidden rounded-cc border border-line2">
        <button
          v-for="l in LOCALES"
          :key="l"
          type="button"
          :lang="l"
          :aria-label="t(`lang.${l}`)"
          :aria-pressed="locale === l"
          class="px-3 py-1 text-sm uppercase hover:bg-sunk aria-pressed:bg-accent aria-pressed:text-accent-fg"
          @click="setLocale(l as Locale)"
        >
          {{ l }}
        </button>
      </div>
    </div>
  </header>
</template>
