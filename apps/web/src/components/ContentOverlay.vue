<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
// Content sent by the sender: an image (fit contain, with a close button) or a link confirmation card.
// A link is opened only by a click on "Open" here, never automatically.
import { nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import type { ReceivedContent } from '../core/receiverSession'
import type { PageBitmap } from '../core/pdfRenderer'
import DocumentView from './DocumentView.vue'

const props = defineProps<{ content: ReceivedContent | null; renderPage: (page: number, width: number) => Promise<PageBitmap> }>()
const emit = defineEmits<{ close: []; prev: []; next: [] }>()
const { t } = useI18n()
const dismissBtn = ref<HTMLButtonElement | null>(null)
const closeBtn = ref<HTMLButtonElement | null>(null)

watch(
  () => props.content,
  async (c) => {
    if (!c || c.kind === 'document') return // the document view focuses its own close button
    await nextTick()
    // Focus the safe choice, so an accidental Enter does not open a link.
    ;(c.kind === 'link' ? dismissBtn.value : closeBtn.value)?.focus()
  },
)
/** Remove the card only after the browser has started the navigation of the clicked anchor. */
function afterOpen() {
  setTimeout(() => emit('close'), 0)
}
/** Keeps Tab inside the link card (Open <-> Dismiss) while it is shown. */
function trapTab(e: KeyboardEvent) {
  const card = (e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('a[href], button')
  const first = card[0]
  const last = card[card.length - 1]
  if (!first || !last) return
  if (e.shiftKey && document.activeElement === first) {
    last.focus()
    e.preventDefault()
  } else if (!e.shiftKey && document.activeElement === last) {
    first.focus()
    e.preventDefault()
  }
}
function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.content) {
    emit('close')
    e.stopPropagation()
  }
}
</script>

<template>
  <!-- Image below the annotation layer (z-10 < layer < z-[26]); its close button and the link card above it. -->
  <template v-if="content">
    <DocumentView
      v-if="content.kind === 'document'"
      :content="content"
      :render="renderPage"
      @close="emit('close')"
      @prev="emit('prev')"
      @next="emit('next')"
    />
    <div v-else-if="content.kind === 'image'" class="absolute inset-0 z-10 bg-black">
      <img
        :src="content.url"
        :alt="t('receive.content.imageAlt', { name: content.name })"
        data-testid="received-image"
        class="h-full w-full object-contain"
      />
    </div>
    <button
      v-if="content.kind === 'image'"
      ref="closeBtn"
      type="button"
      data-testid="content-close"
      class="absolute bottom-3 right-3 z-[26] rounded-cc bg-black/70 px-3 py-1.5 text-sm font-medium text-white hover:bg-black/85"
      @click="emit('close')"
      @keydown="onKey"
    >
      {{ t('receive.content.close') }}
    </button>

    <div v-else-if="content.kind === 'link'" class="absolute inset-0 z-[26] grid place-items-center bg-black/55 p-4" @keydown="onKey">
      <div
        role="alertdialog"
        aria-modal="true"
        @keydown.tab="trapTab"
        aria-labelledby="link-card-title"
        aria-describedby="link-card-desc"
        data-testid="link-card"
        class="w-full max-w-md rounded-cc border border-line bg-surface p-5 text-fg shadow-xl"
      >
        <h2 id="link-card-title" class="text-lg font-semibold">
          {{ t('receive.link.title', { host: content.host }) }}
        </h2>
        <p id="link-card-desc" class="mt-1 text-sm text-muted">{{ t('receive.link.hint') }}</p>
        <div class="mt-3 text-xs font-semibold uppercase tracking-wider text-muted">{{ t('receive.link.address') }}</div>
        <p data-testid="link-url" class="mt-1 break-all rounded-cc border border-line bg-sunk px-2 py-1.5 font-mono text-sm">{{ content.url }}</p>
        <div class="mt-4 flex flex-wrap gap-3">
          <a
            :href="content.url"
            target="_blank"
            rel="noopener noreferrer"
            data-testid="link-open"
            class="rounded-cc bg-accent px-4 py-2 font-medium text-accent-fg"
            @click="afterOpen"
          >
            {{ t('receive.link.open') }}
          </a>
          <button
            ref="dismissBtn"
            type="button"
            data-testid="link-dismiss"
            class="rounded-cc border border-line2 bg-surface px-4 py-2 font-medium"
            @click="emit('close')"
          >
            {{ t('receive.link.dismiss') }}
          </button>
        </div>
      </div>
    </div>
  </template>
</template>
