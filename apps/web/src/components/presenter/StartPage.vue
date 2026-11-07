<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import Brand from './Brand.vue'
import Icon from '../Icon.vue'
import { currentPlatform } from '../../core/platform'
import { cleanName, loadDisplayName, MAX_NAME_CHARS, saveDisplayName } from '../../core/joinPrefs'
import { CODE_FIELD_MAX, codeFieldText, codeFromField } from '../../core/roomCode'
import { formatCode } from '../../core/codeFormat'
import { appUrlFor } from '../../config/stores'
import { splitSlot } from '../../core/textSlot'
import { legalLinks, licencesUrl, sourceUrl } from '../../core/legal'
import type { LegalUrls } from '../../core/relay'

// The page at {dom} and {dom}/{CODE} (spec 5.2): code, name, browser or app.
const props = defineProps<{
  /** Code that came with the link (QR code, camera chip); null when the person types it. */
  qrCode: string | null
  dom: string
  busy: boolean
  /** Shown instead of nothing after a failed code check (inline error). */
  codeError?: boolean
  /** privacy_url / imprint_url of the operator's config.json (decision N83). */
  legal?: LegalUrls
}>()
// `connect` carries the text of the field without spaces and dashes; the page that receives it checks it
// (not exactly 6 characters of the code alphabet: error page, no server call).
const emit = defineEmits<{ connect: [code: string] }>()
const { t, locale } = useI18n()
const platform = currentPlatform()

const code = ref(props.qrCode ?? '')
const empty = ref(false)
const name = ref(loadDisplayName())
const downloadStarted = ref(false)
const codeInput = ref<HTMLInputElement | null>(null)

const device = computed(() => t(`device.${platform.device}`))
const detected = computed(() => t('start.detected', { device: device.value, browser: platform.browser }))
// The app is offered only where its store or download link exists (src/config/stores.ts).
const appUrl = computed(() => appUrlFor(platform.store))
const appLabel = computed(() =>
  platform.store === 'google-play'
    ? t('start.appAndroid')
    : platform.store === 'app-store'
      ? t('start.appIos')
      : t('start.appDesktop', { os: platform.os }),
)

// The field shows every typed character in uppercase (at most 9) and never drops one silently.
function onCode(e: Event) {
  const el = e.target as HTMLInputElement
  code.value = codeFieldText(el.value)
  el.value = code.value
  empty.value = false
}
function onName(e: Event) {
  const el = e.target as HTMLInputElement
  name.value = cleanName(el.value)
  saveDisplayName(name.value)
}
function connect() {
  // Validation happens on submit only (spec 5.2): spaces and dashes go, the rest is checked by the caller.
  const submitted = codeFromField(code.value)
  if (submitted.length === 0) {
    empty.value = true
    void nextTick(() => codeInput.value?.focus())
    return
  }
  emit('connect', submitted)
}
function getApp() {
  const url = appUrl.value
  if (!url) return // not reachable: the button is not drawn without a link
  window.location.assign(url)
  if (!platform.store) downloadStarted.value = true
}

// "Should this device be the screen itself? Open {dom}/display." The address is a link; the rest of the
// sentence stays text. The sentence is built with a marker so that every language is cut at the right place.
const beDisplayParts = computed(() => splitSlot((m) => t('start.beDisplay', { dom: m }), '/display'.length))
const displayHref = `${import.meta.env.BASE_URL.replace(/\/$/, '')}/display`
// Footer line (spec 15.9, web part): licence, source, the licence list of this build, and privacy / imprint only where
// there is a page to name (the build's branding file, or the operator's config.json).
const footSource = sourceUrl(__APP_VERSION__)
const footLicences = licencesUrl(import.meta.env.BASE_URL)
const footLegal = computed(() => legalLinks(locale.value === 'de' ? 'de' : 'en', props.legal ?? {}))
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
            :maxlength="CODE_FIELD_MAX"
            autocomplete="off"
            autocapitalize="characters"
            spellcheck="false"
            placeholder="XXX XXX"
            :value="code"
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
        <template v-if="appUrl">
          <div class="cp-or" data-testid="start-or">{{ t('start.or') }}</div>
          <div class="cp-choice">
            <button type="button" class="cp-sbtn" data-testid="start-app" @click="getApp">
              <Icon name="download" />{{ appLabel }}
            </button>
            <p class="cp-small">{{ platform.isPhone ? t('start.appPhone') : t('start.appLaptop') }}</p>
            <p v-if="downloadStarted" class="cp-small cp-dl" role="status" data-testid="download-note">{{ t('start.downloadStarted') }}</p>
          </div>
        </template>
        <p class="cp-det" data-testid="detected"><Icon name="info" />{{ detected }}</p>
        <p v-if="platform.isLaptop" class="cp-small" data-testid="be-display">
          {{ beDisplayParts.before }}<a class="cp-link" :href="displayHref" data-testid="be-display-link">{{ dom }}/display</a>{{ beDisplayParts.after }}
        </p>
      </div>
    </form>
    <p class="cp-small cp-foot" data-testid="start-footer">
      {{ t('about.footer') }}
      · <a class="cp-link" :href="footSource" target="_blank" rel="noopener noreferrer" data-testid="foot-source">{{ t('about.source') }}</a>
      · <a class="cp-link" :href="footLicences" target="_blank" rel="noopener noreferrer" data-testid="foot-licences">{{ t('about.licences') }}</a>
      <template v-if="footLegal.privacy">
        · <a class="cp-link" :href="footLegal.privacy" target="_blank" rel="noopener noreferrer" data-testid="foot-privacy">{{ t('about.privacy') }}</a>
      </template>
      <template v-if="footLegal.imprint">
        · <a class="cp-link" :href="footLegal.imprint" target="_blank" rel="noopener noreferrer" data-testid="foot-imprint">{{ t('about.imprint') }}</a>
      </template>
    </p>
  </main>
</template>
