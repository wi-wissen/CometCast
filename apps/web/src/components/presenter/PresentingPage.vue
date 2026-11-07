<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import Icon, { type IconName } from '../Icon.vue'
import SasLine from '../SasLine.vue'

// Presenting, presenter side (spec 5.6): icon disc, heading, body, "Stop showing". When the
// browser's own sharing bar is shown at the bottom, 76 px stay free under the button.
const props = defineProps<{ kind: 'screen' | 'file' | 'link'; room: string; keepBar: boolean; sas?: string; pdf?: boolean }>()
const emit = defineEmits<{ stop: []; page: [cmd: 'next_page' | 'prev_page'] }>()
const { t } = useI18n()
const ICON: Record<string, IconName> = { screen: 'screen', file: 'file', link: 'link' }
</script>

<template>
  <main class="cp-page" :class="{ keepbar: keepBar }" data-testid="presenting-page">
    <div class="cp-ico acc"><Icon :name="ICON[props.kind]" /></div>
    <h1 class="cp-h" data-testid="presenting-title">{{ t(`presenting.${kind}`) }}</h1>
    <p class="cp-p">{{ t('presenting.where', { room }) }}</p>
    <SasLine :code="sas" />
    <!-- A shown PDF is turned page by page from here (existing behaviour; the spec has no design for it). -->
    <div v-if="pdf" class="cp-acts" role="group" :aria-label="t('sendContent.document.pagesHelp')">
      <button type="button" class="cp-sbtn" data-testid="send-prev-page" @click="emit('page', 'prev_page')">{{ t('sendContent.document.prev') }}</button>
      <button type="button" class="cp-sbtn" data-testid="send-next-page" @click="emit('page', 'next_page')">{{ t('sendContent.document.next') }}</button>
    </div>
    <div class="cp-spacer"></div>
    <button type="button" class="cp-pbtn" data-testid="presenting-stop" @click="emit('stop')">{{ t('presenting.stop') }}</button>
  </main>
</template>
