<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { IMAGE_MIME_TYPES, MAX_DOCUMENT_BYTES, MAX_IMAGE_BYTES, SendError } from '../../core/control'

// File page (spec 5.5): heading, body, back. A browser has no list of recent files, so the system
// file dialog is the only way to pick (see docs/design/implementation-questions.md); a picked file
// starts presenting.
const props = defineProps<{ send: (file: File, onProgress: (sent: number, total: number) => void) => Promise<void> }>()
const emit = defineEmits<{ back: []; started: [] }>()
const { t, te } = useI18n()
const input = ref<HTMLInputElement | null>(null)
const progress = ref<{ name: string; sent: number; total: number } | null>(null)
const error = ref('')
const accept = [...IMAGE_MIME_TYPES, 'application/pdf', '.pdf'].join(',')
const percent = computed(() => (progress.value ? Math.floor((progress.value.sent / progress.value.total) * 100) : 0))

async function onPick(e: Event) {
  const el = e.target as HTMLInputElement
  const f = el.files?.[0]
  el.value = '' // allows picking the same file again
  if (!f || progress.value) return
  error.value = ''
  progress.value = { name: f.name, sent: 0, total: f.size }
  const pdf = f.type === 'application/pdf' || /\.pdf$/i.test(f.name)
  try {
    await props.send(f, (sent, total) => (progress.value = { name: f.name, sent, total }))
    emit('started')
  } catch (err) {
    const code = err instanceof SendError ? err.code : 'failed'
    const kind = pdf ? 'document' : 'image'
    const key = `sendContent.${kind}.error.${code}`
    const max = pdf ? `${MAX_DOCUMENT_BYTES / (1024 * 1024)} MiB` : `${MAX_IMAGE_BYTES / (1024 * 1024)} MiB`
    error.value = te(key) ? t(key, { max }) : t(`sendContent.${kind}.error.failed`)
  } finally {
    progress.value = null
  }
}
</script>

<template>
  <main class="cp-page" data-testid="file-page">
    <h1 class="cp-h">{{ t('file.title') }}</h1>
    <p class="cp-p">{{ t('file.body') }}</p>
    <input ref="input" type="file" :accept="accept" class="sr-only" tabindex="-1" data-testid="file-input" @change="onPick" />
    <button type="button" class="cp-sbtn" :disabled="progress !== null" data-testid="file-choose" @click="input?.click()">
      {{ t('what.file') }}
    </button>
    <div v-if="progress" class="cp-progress" data-testid="file-progress">
      <p class="cp-small">{{ progress.name }} · {{ percent }} %</p>
      <div class="cp-pbar"><span :style="{ width: percent + '%' }"></span></div>
    </div>
    <p v-if="error" class="cp-errt" role="alert" data-testid="file-error">{{ error }}</p>
    <div class="cp-spacer"></div>
    <button type="button" class="cp-sbtn" data-testid="file-back" @click="emit('back')">{{ t('common.back') }}</button>
  </main>
</template>
