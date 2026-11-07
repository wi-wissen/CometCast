<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
// "Send file or link": images (picker or drag and drop) and http(s) links over the WebRTC control channel.
// Nothing goes through the signalling server except the connection setup.
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSender } from '../composables/useSender'
import { IMAGE_MIME_TYPES, MAX_DOCUMENT_BYTES, MAX_IMAGE_BYTES, SendError, validateLinkUrl } from '../core/control'
import { isValidRoomCode, normalizeRoomCode, ROOM_CODE_LENGTH } from '../core/roomCode'

const { t, te } = useI18n()
const code = ref('')
const touched = ref(false)
const valid = computed(() => isValidRoomCode(code.value))
const { state, start, stop, sendImage, sendDocument, sendLink, sendCmd } = useSender({ contentOnly: true })

const active = computed(() => ['connecting', 'joining', 'negotiating', 'sharing'].includes(state.value.phase))
const ready = computed(() => state.value.channelOpen)
const errorText = computed(() => {
  const c = state.value.errorCode
  return c ? (te(`errors.${c}`) ? t(`errors.${c}`) : t('errors.unknown')) : ''
})
const maxLabel = `${MAX_IMAGE_BYTES / (1024 * 1024)} MiB`
const accept = IMAGE_MIME_TYPES.join(',')

function onSubmit() {
  if (valid.value && !active.value) void start(code.value)
}
function onInput(e: Event) {
  const el = e.target as HTMLInputElement
  code.value = normalizeRoomCode(el.value)
  el.value = code.value
  touched.value = true
}

// --- image ---------------------------------------------------------------------------------------
type Notice = { kind: 'ok' | 'err'; text: string } | null
const imageNotice = ref<Notice>(null)
const progress = ref<{ name: string; sent: number; total: number } | null>(null)
const dragging = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)

async function sendFile(file: File) {
  imageNotice.value = null
  progress.value = { name: file.name, sent: 0, total: file.size }
  try {
    await sendImage(file, (sent, total) => (progress.value = { name: file.name, sent, total }))
    imageNotice.value = { kind: 'ok', text: t('sendContent.image.sent', { name: file.name }) }
  } catch (e) {
    const code = e instanceof SendError ? e.code : 'failed'
    const key = `sendContent.image.error.${code}`
    imageNotice.value = {
      kind: 'err',
      text: te(key) ? t(key, { max: maxLabel }) : t('sendContent.image.error.failed'),
    }
  } finally {
    progress.value = null
  }
}
function onPick(e: Event) {
  const el = e.target as HTMLInputElement
  const f = el.files?.[0]
  el.value = '' // allows picking the same file again
  if (f && ready.value) void sendFile(f)
}
function onDrop(e: DragEvent) {
  dragging.value = false
  const f = e.dataTransfer?.files?.[0]
  if (f && ready.value && !progress.value) void sendFile(f)
}
const percent = computed(() => (progress.value ? Math.floor((progress.value.sent / progress.value.total) * 100) : 0))

// --- PDF document ---------------------------------------------------------------------------------
const docNotice = ref<Notice>(null)
const docProgress = ref<{ name: string; sent: number; total: number } | null>(null)
const docDragging = ref(false)
const docInput = ref<HTMLInputElement | null>(null)
const docMaxLabel = `${MAX_DOCUMENT_BYTES / (1024 * 1024)} MiB`

async function sendPdf(file: File) {
  docNotice.value = null
  docProgress.value = { name: file.name, sent: 0, total: file.size }
  try {
    await sendDocument(file, (sent, total) => (docProgress.value = { name: file.name, sent, total }))
    docNotice.value = { kind: 'ok', text: t('sendContent.document.sent', { name: file.name }) }
  } catch (e) {
    const code = e instanceof SendError ? e.code : 'failed'
    const key = `sendContent.document.error.${code}`
    docNotice.value = {
      kind: 'err',
      text: te(key) ? t(key, { max: docMaxLabel }) : t('sendContent.document.error.failed'),
    }
  } finally {
    docProgress.value = null
  }
}
function onPickPdf(e: Event) {
  const el = e.target as HTMLInputElement
  const f = el.files?.[0]
  el.value = '' // allows picking the same file again
  if (f && ready.value && !docProgress.value) void sendPdf(f)
}
function onDropPdf(e: DragEvent) {
  docDragging.value = false
  const f = e.dataTransfer?.files?.[0]
  if (f && ready.value && !docProgress.value) void sendPdf(f)
}
const docPercent = computed(() => (docProgress.value ? Math.floor((docProgress.value.sent / docProgress.value.total) * 100) : 0))
function turnPage(cmd: 'next_page' | 'prev_page') {
  try {
    sendCmd(cmd)
  } catch {
    /* channel closed; the status line already says so */
  }
}

// --- link ----------------------------------------------------------------------------------------
const link = ref('')
const linkNotice = ref<Notice>(null)
function onSendLink() {
  if (!ready.value) return
  if (!validateLinkUrl(link.value)) {
    linkNotice.value = { kind: 'err', text: t('sendContent.link.invalid') }
    return
  }
  try {
    sendLink(link.value)
    linkNotice.value = { kind: 'ok', text: t('sendContent.link.sent') }
  } catch {
    linkNotice.value = { kind: 'err', text: t('sendContent.link.failed') }
  }
}
function stopShowing() {
  try {
    sendCmd('stop')
  } catch {
    /* channel closed; the status line already says so */
  }
}
</script>

<template>
  <section class="flex flex-col gap-5">
    <h1 class="text-2xl font-semibold tracking-tight">{{ t('sendContent.title') }}</h1>
    <p class="text-muted">{{ t('sendContent.intro') }}</p>

    <form class="flex flex-col gap-2" @submit.prevent="onSubmit">
      <label for="room-code" class="text-sm font-medium">{{ t('send.codeLabel') }}</label>
      <input
        id="room-code"
        :value="code"
        :disabled="active"
        type="text"
        autocomplete="off"
        autocapitalize="characters"
        spellcheck="false"
        :maxlength="ROOM_CODE_LENGTH"
        :aria-invalid="touched && !valid"
        aria-describedby="room-code-hint"
        class="w-full max-w-xs rounded-cc border border-line2 bg-surface px-3 py-2 font-mono text-xl uppercase tracking-[0.3em]"
        @input="onInput"
      />
      <p id="room-code-hint" class="text-sm" :class="touched && !valid ? 'text-err' : 'text-muted'">
        {{ touched && !valid ? t('send.codeInvalid') : t('send.codeHint') }}
      </p>
      <div class="flex flex-wrap items-center gap-3">
        <button
          type="submit"
          :disabled="!valid || active"
          class="rounded-cc bg-accent px-4 py-2 font-medium text-accent-fg disabled:cursor-not-allowed disabled:opacity-50"
        >
          {{ t(state.phase === 'error' || state.phase === 'peer_left' ? 'send.tryAgain' : 'sendContent.connect') }}
        </button>
        <button
          v-if="active"
          type="button"
          data-testid="stop"
          class="rounded-cc border border-line2 bg-surface px-4 py-2 font-medium"
          @click="stop"
        >
          {{ t('sendContent.disconnect') }}
        </button>
      </div>
    </form>

    <p
      data-testid="status"
      :data-phase="state.phase"
      :data-ready="ready"
      role="status"
      aria-live="polite"
      class="rounded-cc border px-3 py-2"
      :class="state.phase === 'error' ? 'border-err text-err' : 'border-line bg-surface'"
    >
      <template v-if="state.phase === 'sharing' && !ready">{{ t('sendContent.status.negotiating') }}</template>
      <template v-else>{{ t(`sendContent.status.${state.phase}`) }}</template>
      <template v-if="state.phase === 'error'"> {{ errorText }}</template>
    </p>

    <fieldset class="flex flex-col gap-3 rounded-cc border border-line bg-surface p-4" :disabled="!ready">
      <legend class="px-1 font-semibold">{{ t('sendContent.image.title') }}</legend>
      <p class="text-sm text-muted">{{ t('sendContent.image.help', { max: maxLabel }) }}</p>
      <div
        data-testid="drop-zone"
        class="flex flex-col items-center gap-2 rounded-cc border-2 border-dashed px-4 py-6 text-center"
        :class="dragging ? 'border-accent bg-accent-soft' : 'border-line2'"
        @dragover.prevent="dragging = true"
        @dragleave="dragging = false"
        @drop.prevent="onDrop"
      >
        <span class="text-sm text-muted">{{ t('sendContent.image.drop') }}</span>
        <input ref="fileInput" type="file" :accept="accept" class="sr-only" data-testid="image-input" tabindex="-1" @change="onPick" />
        <button
          type="button"
          :disabled="!ready || progress !== null"
          class="rounded-cc border border-line2 bg-surface px-4 py-2 font-medium disabled:cursor-not-allowed disabled:opacity-50"
          @click="fileInput?.click()"
        >
          {{ t('sendContent.image.choose') }}
        </button>
      </div>
      <div v-if="progress" data-testid="image-progress">
        <label for="send-progress" class="text-sm">{{ t('sendContent.image.progress', { name: progress.name, percent }) }}</label>
        <progress id="send-progress" class="mt-1 block w-full" :value="progress.sent" :max="progress.total" />
      </div>
      <p
        v-if="imageNotice"
        data-testid="image-notice"
        role="status"
        class="text-sm"
        :class="imageNotice.kind === 'ok' ? 'text-ok' : 'text-err'"
      >
        {{ imageNotice.text }}
      </p>
    </fieldset>

    <fieldset class="flex flex-col gap-3 rounded-cc border border-line bg-surface p-4" :disabled="!ready">
      <legend class="px-1 font-semibold">{{ t('sendContent.document.title') }}</legend>
      <p class="text-sm text-muted">{{ t('sendContent.document.help', { max: docMaxLabel }) }}</p>
      <div
        data-testid="doc-drop-zone"
        class="flex flex-col items-center gap-2 rounded-cc border-2 border-dashed px-4 py-6 text-center"
        :class="docDragging ? 'border-accent bg-accent-soft' : 'border-line2'"
        @dragover.prevent="docDragging = true"
        @dragleave="docDragging = false"
        @drop.prevent="onDropPdf"
      >
        <span class="text-sm text-muted">{{ t('sendContent.document.drop') }}</span>
        <input ref="docInput" type="file" accept="application/pdf,.pdf" class="sr-only" data-testid="pdf-input" tabindex="-1" @change="onPickPdf" />
        <button
          type="button"
          :disabled="!ready || docProgress !== null"
          class="rounded-cc border border-line2 bg-surface px-4 py-2 font-medium disabled:cursor-not-allowed disabled:opacity-50"
          @click="docInput?.click()"
        >
          {{ t('sendContent.document.choose') }}
        </button>
      </div>
      <div v-if="docProgress" data-testid="pdf-progress">
        <label for="send-pdf-progress" class="text-sm">{{ t('sendContent.document.progress', { name: docProgress.name, percent: docPercent }) }}</label>
        <progress id="send-pdf-progress" class="mt-1 block w-full" :value="docProgress.sent" :max="docProgress.total" />
      </div>
      <p
        v-if="docNotice"
        data-testid="pdf-notice"
        role="status"
        class="text-sm"
        :class="docNotice.kind === 'ok' ? 'text-ok' : 'text-err'"
      >
        {{ docNotice.text }}
      </p>
      <div role="group" :aria-label="t('sendContent.document.pagesHelp')" class="flex flex-wrap items-center gap-3">
        <button
          type="button"
          data-testid="send-prev-page"
          class="rounded-cc border border-line2 bg-surface px-4 py-2 font-medium disabled:cursor-not-allowed disabled:opacity-50"
          :disabled="!ready"
          @click="turnPage('prev_page')"
        >
          {{ t('sendContent.document.prev') }}
        </button>
        <button
          type="button"
          data-testid="send-next-page"
          class="rounded-cc border border-line2 bg-surface px-4 py-2 font-medium disabled:cursor-not-allowed disabled:opacity-50"
          :disabled="!ready"
          @click="turnPage('next_page')"
        >
          {{ t('sendContent.document.next') }}
        </button>
        <span class="text-sm text-muted">{{ t('sendContent.document.pagesHelp') }}</span>
      </div>
    </fieldset>

    <fieldset class="flex flex-col gap-2 rounded-cc border border-line bg-surface p-4" :disabled="!ready">
      <legend class="px-1 font-semibold">{{ t('sendContent.link.title') }}</legend>
      <form class="flex flex-col gap-2" @submit.prevent="onSendLink">
        <label for="link-url" class="text-sm font-medium">{{ t('sendContent.link.label') }}</label>
        <input
          id="link-url"
          v-model="link"
          type="url"
          inputmode="url"
          autocomplete="off"
          spellcheck="false"
          placeholder="https://example.org/"
          aria-describedby="link-hint"
          class="w-full rounded-cc border border-line2 bg-surface px-3 py-2"
          @input="linkNotice = null"
        />
        <p id="link-hint" class="text-sm text-muted">{{ t('sendContent.link.hint') }}</p>
        <div>
          <button
            type="submit"
            :disabled="!ready"
            class="rounded-cc bg-accent px-4 py-2 font-medium text-accent-fg disabled:cursor-not-allowed disabled:opacity-50"
          >
            {{ t('sendContent.link.send') }}
          </button>
        </div>
        <p
          v-if="linkNotice"
          data-testid="link-notice"
          role="status"
          class="text-sm"
          :class="linkNotice.kind === 'ok' ? 'text-ok' : 'text-err'"
        >
          {{ linkNotice.text }}
        </p>
      </form>
    </fieldset>

    <div>
      <button
        type="button"
        :disabled="!ready"
        data-testid="stop-showing"
        class="rounded-cc border border-line2 bg-surface px-4 py-2 font-medium disabled:cursor-not-allowed disabled:opacity-50"
        @click="stopShowing"
      >
        {{ t('sendContent.stopShowing') }}
      </button>
    </div>

    <div class="rounded-cc border border-line bg-sunk p-4">
      <h2 class="mb-1 font-semibold">{{ t('sendContent.later.title') }}</h2>
      <ul class="list-disc pl-5 text-sm text-muted">
        <li>{{ t('sendContent.later.office') }}</li>
        <li>{{ t('sendContent.later.video') }}</li>
      </ul>
      <p class="mt-2 text-sm text-muted">{{ t('sendContent.later.note') }}</p>
    </div>
  </section>
</template>
