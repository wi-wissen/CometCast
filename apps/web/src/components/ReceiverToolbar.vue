<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
// Floating, draggable toolbar over the received screen; collapses to a dot.
// Keyboard: Tab enters the toolbar, Left/Right/Home/End move between buttons, the grip moves with the
// arrow keys, Escape collapses it. All pointer handling uses pointer events (mouse, touch, pen).
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { COLORS, type ColorId, type SizeIndex, type Tool, type ToolbarState } from '../core/toolbarState'

const props = defineProps<{
  state: ToolbarState
  canUndo: boolean
  hasStrokes: boolean
  canFreeze: boolean
  fullscreen: boolean
  fullscreenAvailable: boolean
}>()
const emit = defineEmits<{
  tool: [tool: Tool]
  color: [color: ColorId]
  size: [size: SizeIndex]
  undo: []
  clear: []
  freeze: []
  black: []
  fullscreen: []
  open: [open: boolean]
  move: [x: number, y: number]
}>()

const { t } = useI18n()
const root = ref<HTMLElement | null>(null)
const rov = ref('pen') // item with tabindex 0 (roving tabindex)

const ICONS: Record<string, string> = {
  pen: 'M4 20l1-4L16 5l3 3L8 19l-4 1z M14 7l3 3',
  marker: 'M9 15l7-7 3 3-7 7H9v-3z M5 20h7 M14 6l2-2 4 4-2 2',
  eraser: 'M4 15l8-8a2 2 0 0 1 3 0l3 3a2 2 0 0 1 0 3l-6 6H8z M8 20h11 M8 11l6 6',
  undo: 'M9 14L4 9l5-5 M4 9h10a6 6 0 0 1 0 12h-3',
  clear: 'M5 7h14 M10 7V4h4v3 M7 7l1 13h8l1-13 M10 11v5 M14 11v5',
  laser: 'M12 9a3 3 0 1 0 0 6a3 3 0 0 0 0-6z M12 3v3 M12 18v3 M3 12h3 M18 12h3',
  freeze: 'M9 5v14 M15 5v14',
  black: 'M4 5h16v11H4z M9 20h6 M12 16v4',
  fullscreen: 'M4 9V4h5 M20 9V4h-5 M4 15v5h5 M20 15v5h-5',
  collapse: 'M6 12h12',
}

interface Item {
  id: string
  group: number
  kind: 'icon' | 'color' | 'size'
  icon?: string
  hex?: string
  r?: number
  label: string
  pressed?: boolean
  disabled?: boolean
  run: () => void
}

const items = computed<Item[]>(() => {
  const s = props.state
  const tool = (id: Tool, label: string, group = 1): Item => ({
    id,
    group,
    kind: 'icon',
    icon: id,
    label,
    pressed: s.tool === id,
    run: () => emit('tool', id),
  })
  return [
    tool('pen', t('toolbar.pen')),
    tool('marker', t('toolbar.marker')),
    tool('eraser', t('toolbar.eraser')),
    ...COLORS.map((c): Item => ({
      id: `color-${c.id}`,
      group: 2,
      kind: 'color',
      hex: c.hex,
      label: t(`toolbar.color.${c.id}`),
      pressed: s.color === c.id,
      run: () => emit('color', c.id),
    })),
    ...([0, 1, 2] as const).map((i): Item => ({
      id: `size-${i}`,
      group: 2,
      kind: 'size',
      r: 2 + i * 2,
      label: t(['toolbar.size.small', 'toolbar.size.medium', 'toolbar.size.large'][i]!),
      pressed: s.size === i,
      run: () => emit('size', i),
    })),
    { id: 'undo', group: 3, kind: 'icon', icon: 'undo', label: t('toolbar.undo'), disabled: !props.canUndo, run: () => emit('undo') },
    { id: 'clear', group: 3, kind: 'icon', icon: 'clear', label: t('toolbar.clear'), disabled: !props.hasStrokes, run: () => emit('clear') },
    tool('laser', t('toolbar.laser'), 3),
    {
      id: 'freeze',
      group: 4,
      kind: 'icon',
      icon: 'freeze',
      label: t('toolbar.freeze'),
      pressed: s.frozen,
      disabled: !props.canFreeze && !s.frozen,
      run: () => emit('freeze'),
    },
    { id: 'black', group: 4, kind: 'icon', icon: 'black', label: t('toolbar.black'), pressed: s.black, run: () => emit('black') },
    {
      id: 'fullscreen',
      group: 4,
      kind: 'icon',
      icon: 'fullscreen',
      label: props.fullscreen ? t('toolbar.exitFullscreen') : t('toolbar.fullscreen'),
      pressed: props.fullscreen,
      disabled: !props.fullscreenAvailable,
      run: () => emit('fullscreen'),
    },
  ]
})
const groups = computed(() => {
  const out: Item[][] = []
  for (const it of items.value) {
    ;(out[it.group] ??= []).push(it)
  }
  return out.filter(Boolean)
})

function focusables(): HTMLElement[] {
  return Array.from(root.value?.querySelectorAll<HTMLElement>('[data-tb]:not(:disabled)') ?? [])
}
function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    emit('open', false)
    e.stopPropagation()
    return
  }
  if (!['ArrowRight', 'ArrowLeft', 'Home', 'End'].includes(e.key)) return
  const els = focusables()
  const cur = els.indexOf(document.activeElement as HTMLElement)
  if (cur < 0) return
  let next = cur
  if (e.key === 'ArrowRight') next = (cur + 1) % els.length
  else if (e.key === 'ArrowLeft') next = (cur - 1 + els.length) % els.length
  else if (e.key === 'Home') next = 0
  else next = els.length - 1
  els[next]!.focus()
  e.preventDefault()
}

// --- dragging (grip and collapsed dot) --------------------------------------------------------
let drag: { id: number; sx: number; sy: number; x0: number; y0: number; moved: boolean } | null = null
const MOVE_THRESHOLD = 4

function stageRect(): DOMRect | null {
  return (root.value?.offsetParent as HTMLElement | null)?.getBoundingClientRect() ?? null
}
function onGripDown(e: PointerEvent) {
  if (e.pointerType === 'mouse' && e.button !== 0) return
  drag = { id: e.pointerId, sx: e.clientX, sy: e.clientY, x0: props.state.x, y0: props.state.y, moved: false }
  ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
}
function onGripMove(e: PointerEvent) {
  if (!drag || e.pointerId !== drag.id) return
  const dx = e.clientX - drag.sx
  const dy = e.clientY - drag.sy
  if (!drag.moved && Math.hypot(dx, dy) < MOVE_THRESHOLD) return
  drag.moved = true
  const sr = stageRect()
  const er = root.value?.getBoundingClientRect()
  if (!sr || !er) return
  // left = x% of (stage - element), see the transform on the root element.
  const rx = Math.max(1, sr.width - er.width)
  const ry = Math.max(1, sr.height - er.height)
  emit('move', drag.x0 + (dx / rx) * 100, drag.y0 + (dy / ry) * 100)
}
function onGripUp(e: PointerEvent) {
  if (!drag || e.pointerId !== drag.id) return
  const wasClick = !drag.moved
  drag = null
  // A tap on the collapsed dot expands the toolbar.
  if (wasClick && !props.state.open) emit('open', true)
}
function onGripKey(e: KeyboardEvent) {
  const step = e.shiftKey ? 10 : 2
  const d: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }
  const v = d[e.key]
  if (!v) return
  if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') e.stopPropagation() // do not move focus
  emit('move', props.state.x + v[0], props.state.y + v[1])
  e.preventDefault()
}
function onDotKey(e: KeyboardEvent) {
  if (e.key === 'Enter' || e.key === ' ') {
    emit('open', true)
    e.preventDefault()
  } else onGripKey(e)
}
const posStyle = computed(() => ({
  left: props.state.x + '%',
  top: props.state.y + '%',
  transform: `translate(-${props.state.x}%, -${props.state.y}%)`,
}))
</script>

<template>
  <div ref="root" class="cc-tb-root" :style="posStyle" @keydown="onKey">
    <button
      v-if="!state.open"
      type="button"
      data-testid="toolbar-dot"
      class="cc-tbdot"
      :aria-label="t('toolbar.expand')"
      :title="t('toolbar.expand')"
      @pointerdown="onGripDown"
      @pointermove="onGripMove"
      @pointerup="onGripUp"
      @pointercancel="onGripUp"
      @keydown="onDotKey"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true" class="cc-i"><path :d="ICONS.pen" /></svg>
    </button>

    <div v-else role="toolbar" data-testid="toolbar" :aria-label="t('toolbar.label')" class="cc-tb">
      <div class="cc-grp">
        <button
          type="button"
          data-tb
          data-testid="toolbar-grip"
          class="cc-grip"
          :tabindex="rov === 'grip' ? 0 : -1"
          :aria-label="t('toolbar.grip')"
          :title="t('toolbar.grip')"
          @focus="rov = 'grip'"
          @pointerdown="onGripDown"
          @pointermove="onGripMove"
          @pointerup="onGripUp"
          @pointercancel="onGripUp"
          @keydown="onGripKey"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true" class="cc-i">
            <g fill="currentColor" stroke="none">
              <circle cx="9" cy="6" r="1.4" /><circle cx="15" cy="6" r="1.4" />
              <circle cx="9" cy="12" r="1.4" /><circle cx="15" cy="12" r="1.4" />
              <circle cx="9" cy="18" r="1.4" /><circle cx="15" cy="18" r="1.4" />
            </g>
          </svg>
        </button>
      </div>
      <div v-for="(g, gi) in groups" :key="gi" class="cc-grp">
        <button
          v-for="it in g"
          :key="it.id"
          type="button"
          data-tb
          :data-testid="`tb-${it.id}`"
          :aria-label="it.label"
          :title="it.label"
          :aria-pressed="it.pressed === undefined ? undefined : it.pressed"
          :disabled="it.disabled"
          :tabindex="rov === it.id ? 0 : -1"
          @focus="rov = it.id"
          @click="it.run()"
        >
          <svg v-if="it.kind === 'icon'" viewBox="0 0 24 24" aria-hidden="true" class="cc-i"><path :d="ICONS[it.icon!]" /></svg>
          <span v-else-if="it.kind === 'color'" class="cc-sw" :style="{ background: it.hex }" aria-hidden="true" />
          <svg v-else viewBox="0 0 24 24" aria-hidden="true" class="cc-i"><circle cx="12" cy="12" :r="it.r" fill="currentColor" stroke="none" /></svg>
        </button>
      </div>
      <div class="cc-grp">
        <button
          type="button"
          data-tb
          data-testid="tb-collapse"
          :aria-label="t('toolbar.collapse')"
          :title="t('toolbar.collapse')"
          :tabindex="rov === 'collapse' ? 0 : -1"
          @focus="rov = 'collapse'"
          @click="emit('open', false)"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true" class="cc-i"><path :d="ICONS.collapse" /></svg>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* The toolbar sits over a video, so it keeps the dark mockup look in light and dark themes. */
.cc-tb-root {
  position: absolute;
  z-index: 30;
  width: max-content;
  max-width: calc(100% - 16px);
}
.cc-tb {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 2px;
  padding: 4px;
  border-radius: 10px;
  border: 1px solid rgba(255, 255, 255, 0.14);
  background: rgba(26, 29, 36, 0.96);
  color: #eef0f4;
  box-shadow: 0 6px 24px rgba(0, 0, 0, 0.35);
}
.cc-grp {
  display: flex;
  gap: 2px;
  padding: 0 4px;
  border-left: 1px solid rgba(127, 127, 127, 0.3);
}
.cc-grp:first-child {
  border-left: 0;
}
.cc-tb button {
  width: 36px;
  height: 36px;
  border: 0;
  border-radius: 8px;
  background: none;
  display: grid;
  place-items: center;
  color: inherit;
  padding: 0;
  cursor: pointer;
}
.cc-tb button:hover:not(:disabled) {
  background: rgba(127, 127, 127, 0.22);
}
.cc-tb button[aria-pressed='true'] {
  background: #e9b04a;
  color: #2a1f00;
}
.cc-tb button:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}
.cc-tb button:focus-visible,
.cc-tbdot:focus-visible {
  outline: 2px solid #fff;
  outline-offset: -3px;
  box-shadow: 0 0 0 2px #2d4a8a;
}
.cc-grip {
  cursor: grab !important;
  touch-action: none;
}
.cc-i {
  width: 20px;
  height: 20px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}
.cc-sw {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 2px solid rgba(255, 255, 255, 0.85);
  box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.25);
}
button[aria-pressed='true'] .cc-sw {
  outline: 2px solid #2a1f00;
  outline-offset: 1px;
}
.cc-tbdot {
  width: 44px;
  height: 44px;
  border-radius: 50%;
  border: 0;
  background: #e9b04a;
  color: #2a1f00;
  display: grid;
  place-items: center;
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.35);
  cursor: grab;
  touch-action: none;
  padding: 0;
}
</style>
