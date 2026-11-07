<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import Icon from '../Icon.vue'
import QrCode from './QrCode.vue'
import { logoFor } from './logos'
import type { Theme } from '../../core/appearance'
import type { JoinMode } from '../../core/signaling'
import { formatCode } from '../../core/codeFormat'
import { joinUrl } from '../../core/qr'

// Idle screen (spec 4.2): top row, address + code, QR code, footer with the settings button.
const props = defineProps<{
  theme: Theme
  room: string
  mode: JoinMode
  /** The join code, or null while the room is being created. */
  code: string | null
  /** host[:port] of the server this display uses. */
  dom: string
  offline: boolean
  dimmed: boolean
  /** http for a plain local relay in tests and on a LAN; the spec's QR address is https. */
  scheme: 'https' | 'http'
}>()
const emit = defineEmits<{ settings: [] }>()
const { t } = useI18n()

const locked = computed(() => props.mode === 'locked')
const icon = computed(() => (props.mode === 'open' ? 'open' : props.mode === 'confirm' ? 'approval' : 'locked'))
const modeText = computed(() => t(props.mode === 'open' ? 'mode.open' : props.mode === 'confirm' ? 'mode.approval' : 'mode.locked'))
const url = computed(() => (props.code ? joinUrl(props.dom, props.code, props.scheme) : ''))
const qrDim = computed(() => locked.value || props.offline)
const settingsBtn = ref<HTMLButtonElement | null>(null)
defineExpose({ focusSettings: () => settingsBtn.value?.focus() })
</script>

<template>
  <div class="cd-idle" data-testid="idle">
    <div class="cd-itop">
      <img :src="logoFor(theme)" alt="" />
      <p class="cd-room" data-testid="room-name">{{ room }}</p>
      <div class="cd-pill" data-testid="mode-pill" :data-mode="mode">
        <Icon :name="icon" />{{ modeText }}
      </div>
    </div>

    <div class="cd-imain">
      <div class="cd-isteps">
        <template v-if="!offline">
          <div class="cd-istep">
            <span class="cd-num">1</span>
            <div>
              <p class="cd-ilbl">{{ t('idle.step1') }}</p>
              <p class="cd-idom" data-testid="address">{{ dom }}</p>
            </div>
          </div>
          <div class="cd-istep">
            <span class="cd-num">2</span>
            <div>
              <p class="cd-ilbl">{{ t('idle.step2') }}</p>
              <p class="cd-icode mono" :class="{ dim: locked }" data-testid="room-code">{{ code ? formatCode(code) : ' ' }}</p>
            </div>
          </div>
          <p v-if="locked" class="cd-inote" data-testid="locked-note">{{ t('idle.locked') }}</p>
        </template>
        <div v-else class="cd-offline" data-testid="offline">
          <Icon name="wifiOff" />
          <p>
            <b>{{ t('idle.offline.title', { dom }) }}</b>{{ t('idle.offline.body') }}
          </p>
        </div>
      </div>
      <div class="cd-iqr">
        <div class="cd-qrbox" :class="{ dim: qrDim }" :style="{ visibility: code ? 'visible' : 'hidden' }" data-testid="qr">
          <QrCode v-if="code" :url="url" :label="`QR: ${url}`" :data-url="url" />
        </div>
        <p class="cd-qrcap" data-testid="qr-caption">{{ locked ? t('idle.qrLocked') : t('idle.qrCaption') }}</p>
      </div>
    </div>

    <div class="cd-ifoot">
      <span>{{ t('idle.appLine', { dom }) }}</span>
      <button ref="settingsBtn" type="button" class="cd-menubtn" data-row="0" data-testid="settings-open" @click="emit('settings')">
        <Icon name="gear" />{{ t('settings.open') }}
      </button>
    </div>
    <div class="cd-dim" :class="{ on: dimmed }" data-testid="dim" aria-hidden="true"></div>
  </div>
</template>
