<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { validateLinkUrl } from '../../core/control'

// Link page (spec 5.5): heading, field, hint, "Open on the screen", back.
const props = defineProps<{ send: (url: string) => boolean }>()
const emit = defineEmits<{ back: []; started: [] }>()
const { t } = useI18n()
const url = ref('')
const error = ref('')
function go() {
  if (!validateLinkUrl(url.value)) {
    error.value = t('sendContent.link.invalid')
    return
  }
  try {
    if (props.send(url.value)) emit('started')
    else error.value = t('sendContent.link.invalid')
  } catch {
    error.value = t('sendContent.link.failed')
  }
}
</script>

<template>
  <main class="cp-page" data-testid="link-page">
    <h1 class="cp-h">{{ t('what.link') }}</h1>
    <form class="cp-field" novalidate @submit.prevent="go">
      <label class="cp-lbl" for="link-url">{{ t('link.label') }}</label>
      <input
        id="link-url"
        v-model="url"
        type="url"
        inputmode="url"
        autocomplete="off"
        spellcheck="false"
        placeholder="https://example.org/"
        :class="{ bad: error }"
        aria-describedby="link-hint link-err"
        data-testid="link-url"
        @input="error = ''"
      />
      <p id="link-hint" class="cp-small">{{ t('link.hint') }}</p>
      <p v-if="error" id="link-err" class="cp-errt" role="alert" data-testid="link-error">{{ error }}</p>
      <button type="submit" class="cp-pbtn" data-testid="link-go">{{ t('link.go') }}</button>
    </form>
    <div class="cp-spacer"></div>
    <button type="button" class="cp-sbtn" data-testid="link-back" @click="emit('back')">{{ t('common.back') }}</button>
  </main>
</template>
