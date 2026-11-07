<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { useI18n } from 'vue-i18n'

// Connection check code (see core/sas.ts): shown in the connected state, one line.
defineProps<{ code?: string }>()
const { t } = useI18n()
</script>

<template>
  <p v-if="code" class="cp-small" data-testid="sas-line">
    {{ t('sas.label') }}: <span class="mono" style="font-weight: 700; color: var(--p-fg)" data-testid="sas-code">{{ code }}</span>
    <span style="margin-left: 4px">{{ t('sas.explain') }}</span>
  </p>
</template>
