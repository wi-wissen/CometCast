<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { useI18n } from 'vue-i18n'

// Connection check code (see core/sas.ts): shown in the connected state, one line.
defineProps<{ code?: string }>()
const { t } = useI18n()
</script>

<template>
  <p v-if="code" class="text-sm text-muted" data-testid="sas-line">
    {{ t('sas.label') }}: <span class="font-mono font-semibold text-fg" data-testid="sas-code">{{ code }}</span>
    <span class="ml-1">{{ t('sas.explain') }}</span>
  </p>
</template>
