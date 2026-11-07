<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
// A received PDF: the current page drawn on a canvas, rendered on demand by the WASM renderer in a Web Worker.
// Only pixels are shown: no text layer, no links, nothing from the document is clickable or run. There are no
// buttons on the display: pages are turned on the presenter's page, the document goes away when it stops.
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import type { PageBitmap } from '../core/pdfRenderer'

type DocContent = { kind: 'document'; id: number; name: string; pages: number; page: number }
const props = defineProps<{ content: DocContent; render: (page: number, width: number) => Promise<PageBitmap> }>()
const emit = defineEmits<{ ready: [id: number] }>()
const { t, te } = useI18n()

const box = ref<HTMLElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)
const pageError = ref('')
const busy = ref(false)
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
    emit('ready', c.id)
  } catch (e) {
    if (my !== token) return
    const code = (e as { code?: string } | null)?.code ?? 'failed'
    pageError.value = te(`receive.document.pageError.${code}`) ? t(`receive.document.pageError.${code}`) : t('receive.document.pageError.failed')
    emit('ready', c.id)
  } finally {
    if (my === token) busy.value = false
  }
}

watch(() => [props.content.page, props.content.pages], draw, { flush: 'post' })
let observer: ResizeObserver | undefined
onMounted(() => {
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

// Not drawn: the page number for screen readers (the presenter turns pages from its own page).
const label = computed(() => t('receive.document.pageOf', { page: props.content.page, pages: props.content.pages }))
</script>

<template>
  <div ref="box" class="absolute inset-0 z-10 bg-black" data-testid="document-box">
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
    <p data-testid="document-page" role="status" aria-live="polite" class="sr-only">{{ label }}</p>
    <!-- A page that cannot be drawn: its text centred in a name-tag-look box on the canvas (spec 4.5). No red. -->
    <div v-if="pageError" class="cd-hud" style="z-index: 26">
      <p data-testid="document-page-error" role="alert" class="cd-tag cd-centertag">{{ pageError }}</p>
    </div>
  </div>
</template>
