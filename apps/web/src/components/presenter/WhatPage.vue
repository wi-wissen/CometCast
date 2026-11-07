<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import Icon from '../Icon.vue'

// What do you want to show (spec 5.5): screen, file, link.
const props = defineProps<{
  /** The display's room name; absent when it set none (the header then says only "Connected", never the code). */
  room?: string
  /** Screen sharing is possible in this browser (a laptop browser). */
  screenOk: boolean
}>()
const emit = defineEmits<{ screen: []; file: []; link: []; disconnect: [] }>()
const { t } = useI18n()
const screenSub = computed(() => t(props.screenOk ? 'what.screenLaptop' : 'what.screenPhoneBrowser'))
</script>

<template>
  <main class="cp-page" data-testid="what-page">
    <div class="cp-row">
      <div class="cp-ico ok" style="width: 40px; height: 40px"><Icon name="check" /></div>
      <span class="cp-lbl" data-testid="connected-to">{{ room ? t('what.connectedTo', { room }) : t('what.connected') }}</span>
    </div>
    <h1 class="cp-h">{{ t('what.title') }}</h1>
    <div class="cp-opts">
      <button type="button" class="cp-opt" :class="{ lim: !screenOk }" data-testid="what-screen" @click="emit('screen')">
        <Icon name="screen" /><span>{{ t('what.screen') }}<small>{{ screenSub }}</small></span>
      </button>
      <button type="button" class="cp-opt" data-testid="what-file" @click="emit('file')">
        <Icon name="file" /><span>{{ t('what.file') }}<small>{{ t('what.fileSub') }}</small></span>
      </button>
      <button type="button" class="cp-opt" data-testid="what-link" @click="emit('link')">
        <Icon name="link" /><span>{{ t('what.link') }}<small>{{ t('what.linkSub') }}</small></span>
      </button>
    </div>
    <div class="cp-spacer"></div>
    <button type="button" class="cp-sbtn" data-testid="disconnect" @click="emit('disconnect')">{{ t('what.disconnect') }}</button>
  </main>
</template>
