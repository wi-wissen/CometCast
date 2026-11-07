<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { splitSlot } from '../core/textSlot'

// Security code (core/sas.ts): one line of small print on the "Presenting" page only (spec 5.6):
// "Security code 171 852. The screen shows the same code under your name." The digits are mono 700.
const props = defineProps<{ code?: string }>()
const { t } = useI18n()
const parts = computed(() => splitSlot((m) => t('presenting.sas', { sas: m })))
</script>

<template>
  <p v-if="code" class="cp-small" data-testid="sas-line">
    {{ parts.before }}<span class="mono cp-sas" data-testid="sas-code">{{ props.code }}</span>{{ parts.after }}
  </p>
</template>
