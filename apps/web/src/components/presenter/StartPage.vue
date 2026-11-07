<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import Brand from './Brand.vue'
import Icon from '../Icon.vue'
import { currentPlatform } from '../../core/platform'
import { cleanName, loadDisplayName, MAX_NAME_CHARS, saveDisplayName } from '../../core/joinPrefs'
import { isValidRoomCode, normalizeRoomCode } from '../../core/roomCode'
import { formatCode } from '../../core/codeFormat'
import { DOWNLOAD_URL_DESKTOP, STORE_URL_APP_STORE, STORE_URL_GOOGLE_PLAY } from '../../config/stores'

// The page at {dom} and {dom}/{CODE} (spec 5.2): code, name, browser or app.
const props = defineProps<{
  /** Code that came with the link (QR code, camera chip); null when the person types it. */
  qrCode: string | null
  dom: string
  busy: boolean
  /** Shown instead of nothing after a failed code check (inline error). */
  codeError?: boolean
}>()
const emit = defineEmits<{ connect: [code: string] }>()
const { t } = useI18n()
const platform = currentPlatform()

const code = ref(props.qrCode ?? '')
const empty = ref(false)
const name = ref(loadDisplayName())
const downloadStarted = ref(false)
const codeInput = ref<HTMLInputElement | null>(null)

const device = computed(() => t(`device.${platform.device}`))
const detected = computed(() => t('start.detected', { device: device.value, browser: platform.browser }))
const appLabel = computed(() =>
  platform.store === 'google-play'
    ? t('start.appAndroid')
    : platform.store === 'app-store'
      ? t('start.appIos')
      : t('start.appDesktop', { os: platform.os }),
)

function onCode(e: Event) {
  const el = e.target as HTMLInputElement
  code.value = normalizeRoomCode(el.value)
  el.value = formatCode(code.value)
  empty.value = false
}
function onName(e: Event) {
  const el = e.target as HTMLInputElement
  name.value = cleanName(el.value)
  saveDisplayName(name.value)
}
function connect() {
  // Validation happens on submit only (spec 5.2).
  if (code.value.length === 0) {
    empty.value = true
    void nextTick(() => codeInput.value?.focus())
    return
  }
  emit('connect', code.value)
}
function getApp() {
  const url = platform.store === 'google-play' ? STORE_URL_GOOGLE_PLAY : platform.store === 'app-store' ? STORE_URL_APP_STORE : DOWNLOAD_URL_DESKTOP
  // Placeholders (empty) do not navigate (src/config/stores.ts).
  if (url) window.location.assign(url)
  if (!platform.store) downloadStarted.value = true
}
onMounted(() => {
  if (!props.qrCode) codeInput.value?.focus()
})
</script>

<template>
  <main class="cp-page" data-testid="start-page">
    <form class="cp-land" novalidate @submit.prevent="connect">
      <div class="ca">
        <Brand />
        <h1 class="cp-h">{{ t('start.titleGeneric') }}</h1>
        <div v-if="qrCode" class="cp-codechip" data-testid="code-chip">
          <span class="mono" data-testid="start-code-chip">{{ formatCode(qrCode) }}</span>
          <span class="cp-okt"><Icon name="check" />{{ t('start.fromQr') }}</span>
        </div>
        <div v-else class="cp-field">
          <label class="cp-lbl" for="start-code">{{ t('start.codeLabel') }}</label>
          <input
            id="start-code"
            ref="codeInput"
            class="codein"
            :class="{ bad: empty || codeError }"
            type="text"
            maxlength="11"
            autocomplete="off"
            autocapitalize="characters"
            spellcheck="false"
            placeholder="XXX XXX"
            :value="formatCode(code)"
            :aria-invalid="empty || codeError"
            aria-describedby="start-code-err"
            data-testid="start-code"
            @input="onCode"
          />
          <p v-if="empty" id="start-code-err" class="cp-errt" role="alert" data-testid="start-code-error">{{ t('start.codeEmpty') }}</p>
        </div>
        <div class="cp-field">
          <label class="cp-lbl" for="start-name">{{ t('start.nameLabel') }}</label>
          <input id="start-name" type="text" :maxlength="MAX_NAME_CHARS" autocomplete="nickname" :value="name" data-testid="start-name" @input="onName" />
        </div>
      </div>
      <div class="cb">
        <div class="cp-choice">
          <button type="submit" class="cp-pbtn" :disabled="busy" :aria-busy="busy" data-testid="start-browser">
            <Icon name="globe" />{{ t('start.browser') }}
          </button>
          <p class="cp-small">{{ platform.isPhone ? t('start.browserPhone') : t('start.browserLaptop') }}</p>
        </div>
        <div class="cp-or">{{ t('start.or') }}</div>
        <div class="cp-choice">
          <button type="button" class="cp-sbtn" data-testid="start-app" @click="getApp">
            <Icon name="download" />{{ appLabel }}
          </button>
          <p class="cp-small">{{ platform.isPhone ? t('start.appPhone') : t('start.appLaptop') }}</p>
          <p v-if="downloadStarted" class="cp-dl" role="status" data-testid="download-note">{{ t('start.downloadStarted') }}</p>
        </div>
        <p class="cp-det" data-testid="detected"><Icon name="info" />{{ detected }}</p>
        <p v-if="platform.isLaptop" class="cp-small">{{ t('start.beDisplay', { dom }) }}</p>
      </div>
    </form>
  </main>
</template>
