<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
// Transparent canvas over the stage: pen, marker, eraser and the laser pointer.
// Everything stays in this browser tab; no annotation is ever sent to the sender.
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { AnnotationModel, drawStrokes, type Point } from '../core/annotations'
import { ERASER_RADIUS, penStyle, type ToolbarState } from '../core/toolbarState'

const props = defineProps<{ model: AnnotationModel; toolbar: ToolbarState; label: string }>()
// `input`: a pointer stroke (down or move while drawing) counts as input, so the controls stay while someone draws.
const emit = defineEmits<{ changed: []; input: [] }>()
const canvas = ref<HTMLCanvasElement | null>(null)
const laser = ref<{ x: number; y: number } | null>(null)
let w = 0
let h = 0
let raf = 0
let ro: ResizeObserver | null = null
let drawing = false
let activeId = -1 // only this pointer draws; other fingers are ignored

function redraw() {
  raf = 0
  const c = canvas.value
  const ctx = c?.getContext('2d')
  if (!c || !ctx) return
  ctx.setTransform(c.width / Math.max(w, 1), 0, 0, c.height / Math.max(h, 1), 0, 0)
  drawStrokes(ctx, props.model.strokes, w, h)
}
function schedule() {
  if (!raf) raf = requestAnimationFrame(redraw)
}
function onModelChange() {
  schedule()
  emit('changed')
}
function resize() {
  const c = canvas.value
  if (!c) return
  const r = c.getBoundingClientRect()
  w = r.width
  h = r.height
  const dpr = window.devicePixelRatio || 1
  c.width = Math.max(1, Math.round(w * dpr))
  c.height = Math.max(1, Math.round(h * dpr))
  schedule()
}

onMounted(() => {
  props.model.onChange = onModelChange
  ro = new ResizeObserver(resize)
  if (canvas.value) ro.observe(canvas.value)
  resize()
})
onBeforeUnmount(() => {
  ro?.disconnect()
  if (raf) cancelAnimationFrame(raf)
  props.model.onChange = null
})
watch(() => props.model, (m, old) => {
  old.onChange = null
  m.onChange = onModelChange
  schedule()
})

function point(e: PointerEvent): Point {
  const r = canvas.value!.getBoundingClientRect()
  const x = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width))
  const y = Math.min(1, Math.max(0, (e.clientY - r.top) / r.height))
  return [x, y]
}

function onDown(e: PointerEvent) {
  const tool = props.toolbar.tool
  if (!tool || drawing || (e.pointerType === 'mouse' && e.button !== 0)) return
  emit('input')
  activeId = e.pointerId
  canvas.value!.setPointerCapture(e.pointerId)
  const p = point(e)
  drawing = true
  if (tool === 'laser') {
    laser.value = { x: p[0], y: p[1] }
  } else if (tool === 'eraser') {
    props.model.beginErase()
    props.model.eraseAt(p, ERASER_RADIUS, w, h)
  } else {
    const s = penStyle(props.toolbar)
    if (s) props.model.beginStroke(s.kind, s.color, s.width, p)
  }
  e.preventDefault()
}
function onMove(e: PointerEvent) {
  const tool = props.toolbar.tool
  if (!tool) return
  if (drawing && e.pointerId !== activeId) return
  const p = point(e)
  if (tool === 'laser') {
    // A mouse shows the laser while hovering; touch and pen only while pressed.
    if (e.pointerType === 'mouse' || drawing) laser.value = { x: p[0], y: p[1] }
    return
  }
  if (!drawing) return
  emit('input')
  if (tool === 'eraser') props.model.eraseAt(p, ERASER_RADIUS, w, h)
  else props.model.extend(p)
}
function onUp(e: PointerEvent) {
  if (!drawing || e.pointerId !== activeId) return
  drawing = false
  props.model.endStroke()
  if (props.toolbar.tool === 'laser') laser.value = null
}
function onLeave(e: PointerEvent) {
  if (e.pointerType === 'mouse' && !drawing) laser.value = null
}
watch(() => props.toolbar.tool, () => {
  laser.value = null
  props.model.endStroke()
  drawing = false
})
</script>

<template>
  <div class="absolute inset-0 z-20" :class="toolbar.tool ? '' : 'pointer-events-none'">
    <canvas
      ref="canvas"
      data-testid="annotation-canvas"
      :data-tool="toolbar.tool ?? ''"
      role="img"
      :aria-label="label"
      class="absolute inset-0 h-full w-full"
      :class="[toolbar.tool === 'eraser' ? 'cursor-cell' : toolbar.tool ? 'cursor-crosshair' : '']"
      style="touch-action: none"
      @pointerdown="onDown"
      @pointermove="onMove"
      @pointerup="onUp"
      @pointercancel="onUp"
      @pointerleave="onLeave"
    />
    <span
      v-if="laser"
      data-testid="laser-dot"
      aria-hidden="true"
      class="pointer-events-none absolute h-5 w-5 -translate-x-1/2 -translate-y-1/2 rounded-full"
      :style="{
        left: laser.x * 100 + '%',
        top: laser.y * 100 + '%',
        background: 'radial-gradient(circle, #fff 0 18%, #ff2d2d 30%, rgba(255,45,45,.45) 60%, rgba(255,45,45,0) 72%)',
        boxShadow: '0 0 14px 4px rgba(255,45,45,.55)',
      }"
    />
  </div>
</template>
