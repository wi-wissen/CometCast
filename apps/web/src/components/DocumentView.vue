<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
// A received PDF: the current page drawn on a canvas, rendered on demand by the WASM renderer in a Web Worker.
// Only pixels are shown: no text layer, no links, nothing from the document is clickable or run.
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import type { PageBitmap } from '../core/pdfRenderer'

type DocContent = { kind: 'document'; id: number; name: string; pages: number; page: number }
const props = defineProps<{ content: DocContent; render: (page: number, width: number) => Promise<PageBitmap> }>()
const emit = defineEmits<{ close: []; prev: []; next: [] }>()
const { t, te } = useI18n()

const box = ref<HTMLElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)
const pageError = ref('')
const busy = ref(false)
const closeBtn = ref<HTMLButtonElement | null>(null)
let token = 0
let renderedWidth = 0
let resizeTimer: ReturnType<typeof setTimeout> | undefined

// UNCERTAIN: 3000 px is a guess for the widest page picture on a projector; the wasm side clamps again.
const MAX_WIDTH = 3000
const wantedWidth = () => {
  const w = box.value?.clientWidth ?? 800
  return Math.max(200, Math.min(MAX_WIDTH, Math.round(w * (window.devicePixelRatio || 1))))
}

async function draw() {
  const c = props.content
  if (c.pages === 0 || !canvas.value) return
  const my = ++token
  const width = wantedWidth()
  busy.value = true
  try {
    const bmp = await props.render(c.page, width)
    if (my !== token || !canvas.value) return
    canvas.value.width = bmp.width
    canvas.value.height = bmp.height
    canvas.value.getContext('2d')?.putImageData(new ImageData(bmp.rgba, bmp.width, bmp.height), 0, 0)
    renderedWidth = width
    pageError.value = ''
  } catch (e) {
    if (my !== token) return
    const code = (e as { code?: string } | null)?.code ?? 'failed'
    pageError.value = te(`receive.document.pageError.${code}`) ? t(`receive.document.pageError.${code}`) : t('receive.document.pageError.failed')
  } finally {
    if (my === token) busy.value = false
  }
}

watch(() => [props.content.page, props.content.pages], draw, { flush: 'post' })
let observer: ResizeObserver | undefined
onMounted(() => {
  closeBtn.value?.focus()
  void draw()
  if (typeof ResizeObserver !== 'undefined' && box.value) {
    observer = new ResizeObserver(() => {
      clearTimeout(resizeTimer)
      resizeTimer = setTimeout(() => {
        // Render again only when the box has grown clearly (a sharper picture), not for every pixel.
        if (renderedWidth > 0 && wantedWidth() > renderedWidth * 1.25) void draw()
      }, 250)
    })
    observer.observe(box.value)
  }
})
onBeforeUnmount(() => {
  token++
  clearTimeout(resizeTimer)
  observer?.disconnect()
})

const label = computed(() => t('receive.document.pageOf', { page: props.content.page, pages: props.content.pages }))
function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    emit('close')
    e.stopPropagation()
  } else if (e.key === 'ArrowRight' || e.key === 'PageDown') {
    emit('next')
    e.preventDefault()
  } else if (e.key === 'ArrowLeft' || e.key === 'PageUp') {
    emit('prev')
    e.preventDefault()
  }
}
</script>

<template>
  <div ref="box" class="absolute inset-0 z-10 bg-neutral-700" data-testid="document-box">
    <canvas
      v-show="content.pages > 0"
      ref="canvas"
      role="img"
      :aria-label="t('receive.document.canvasLabel', { name: content.name, page: content.page, pages: content.pages })"
      data-testid="document-canvas"
      :data-page="content.page"
      :data-busy="busy"
      class="h-full w-full object-contain"
    />
    <p v-if="content.pages === 0" data-testid="document-loading" class="absolute inset-0 grid place-items-center text-white/80">
      {{ t('receive.document.loading', { name: content.name }) }}
    </p>
    <p
      v-if="pageError"
      data-testid="document-page-error"
      role="alert"
      class="absolute left-1/2 top-1/2 z-[26] max-w-[90%] -translate-x-1/2 -translate-y-1/2 rounded-cc bg-err px-3 py-2 text-sm text-white"
    >
      {{ pageError }}
    </p>
  </div>

  <div
    v-if="content.pages > 0"
    role="group"
    :aria-label="t('receive.document.controls')"
    class="absolute bottom-3 left-3 z-[26] flex items-center gap-2 rounded-full bg-black/75 px-2 py-1 text-sm text-white"
    @keydown="onKey"
  >
    <button
      type="button"
      data-testid="doc-prev"
      :disabled="content.page <= 1"
      :aria-label="t('receive.document.prev')"
      class="rounded-full px-3 py-1 hover:bg-white/20 disabled:opacity-40"
      @click="emit('prev')"
    >
      &larr;
    </button>
    <span data-testid="document-page" role="status" aria-live="polite" class="min-w-[7ch] text-center tabular-nums">{{ label }}</span>
    <button
      type="button"
      data-testid="doc-next"
      :disabled="content.page >= content.pages"
      :aria-label="t('receive.document.next')"
      class="rounded-full px-3 py-1 hover:bg-white/20 disabled:opacity-40"
      @click="emit('next')"
    >
      &rarr;
    </button>
  </div>
  <button
    ref="closeBtn"
    type="button"
    data-testid="content-close"
    class="absolute bottom-3 right-3 z-[26] rounded-cc bg-black/70 px-3 py-1.5 text-sm font-medium text-white hover:bg-black/85"
    @click="emit('close')"
    @keydown="onKey"
  >
    {{ t('receive.document.close') }}
  </button>
</template>
