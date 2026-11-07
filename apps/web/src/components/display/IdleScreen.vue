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
import { anyAppLink } from '../../config/stores'
import type { ConnectionView, OfflineCause } from '../../core/connectionView'

// Idle screen (spec 4.2): top row, address + code, QR code, footer with the settings button.
const props = defineProps<{
  theme: Theme
  room: string
  mode: JoinMode
  /** The join code, or null while the room is being created (or the connection is gone). */
  code: string | null
  /** host[:port] of the server this display uses. */
  dom: string
  /** Idle, connecting: no code yet; idle, offline: the box (spec 4, 4.2). See core/connectionView.ts. */
  view: ConnectionView
  /** Why the box is shown: picks its body text (spec 4.2). */
  cause: OfflineCause
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
const offline = computed(() => props.view === 'offline')
// Empty (no modules) while connecting or offline, in the empty-box look at full opacity (spec 3.2): an old code must
// never be scannable. Only a QR code that exists (Locked) is drawn at 25 %.
const qrEmpty = computed(() => props.view !== 'online' || !props.code)
const qrDim = computed(() => locked.value && !qrEmpty.value)
const bodyKey = computed(() => (props.cause === 'busy' ? 'idle.offline.bodyBusy' : props.cause === 'outdated' ? 'idle.offline.bodyOutdated' : 'idle.offline.body'))
// The footer names the stores only when at least one link exists (src/config/stores.ts).
const footKey = anyAppLink() ? 'idle.appLine' : 'idle.appLineWeb'
// idle.appLine writes `{dom}/app`; where the join address already ends in /app it is shown once (decision N85).
const footDom = computed(() => props.dom.replace(/\/app$/, ''))
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
              <p class="cd-icode mono" :class="{ dim: locked }" data-testid="room-code">{{ code && view === 'online' ? formatCode(code) : ' ' }}</p>
            </div>
          </div>
          <p v-if="locked" class="cd-inote" data-testid="locked-note">{{ t('idle.locked') }}</p>
        </template>
        <div v-else class="cd-offline" data-testid="offline">
          <Icon name="wifiOff" />
          <p>
            <b>{{ t('idle.offline.title', { dom }) }}</b><span data-testid="offline-body">{{ t(bodyKey) }}</span>
          </p>
        </div>
      </div>
      <div class="cd-iqr">
        <div class="cd-qrbox" :class="{ dim: qrDim, empty: qrEmpty }" :data-empty="qrEmpty" data-testid="qr">
          <QrCode v-if="!qrEmpty" :url="url" :label="`QR: ${url}`" :data-url="url" />
        </div>
        <p class="cd-qrcap" :style="{ visibility: qrEmpty ? 'hidden' : 'visible' }" data-testid="qr-caption">{{ locked ? t('idle.qrLocked') : t('idle.qrCaption') }}</p>
      </div>
    </div>

    <div class="cd-ifoot">
      <span data-testid="idle-foot">{{ t(footKey, { dom: footDom }) }}</span>
      <button ref="settingsBtn" type="button" class="cd-menubtn" data-row="0" data-testid="settings-open" @click="emit('settings')">
        <Icon name="gear" />{{ t('settings.open') }}
      </button>
    </div>
    <div class="cd-dim" :class="{ on: dimmed }" data-testid="dim" aria-hidden="true"></div>
  </div>
</template>
