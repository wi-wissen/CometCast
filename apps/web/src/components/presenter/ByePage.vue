<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import Icon from '../Icon.vue'

// Disconnected (spec 5.7): grey tick disc, title, body, "Connect again".
const emit = defineEmits<{ again: [] }>()
const { t } = useI18n()
</script>

<template>
  <main class="cp-page" data-testid="bye-page">
    <div class="cp-ico mut"><Icon name="check" /></div>
    <h1 class="cp-h">{{ t('bye.title') }}</h1>
    <p class="cp-p">{{ t('bye.body') }}</p>
    <div class="cp-spacer"></div>
    <button type="button" class="cp-pbtn" data-testid="bye-again" @click="emit('again')">{{ t('bye.again') }}</button>
  </main>
</template>
