<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
// Annotation toolbar of the web display (spec 3.5, 4.5): top centre, drawn with the display tokens. It is hidden at
// rest and fades in and out together with the name tag and End (`shown`). No grip, no collapse: it does not move
// and hides by itself. It is not in the D-pad focus rows; Tab reaches it while it is shown.
// Keyboard: Left/Right/Home/End move between buttons (roving tab stop). Pointer handling lives in AnnotationLayer.
import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { COLORS, type ColorId, type SizeIndex, type Tool, type ToolbarState } from '../core/toolbarState'

const props = defineProps<{
  state: ToolbarState
  canUndo: boolean
  hasStrokes: boolean
  canFreeze: boolean
  fullscreen: boolean
  fullscreenAvailable: boolean
  /** Shown together with the name tag and End; hidden (faded, not focusable) at rest. */
  shown: boolean
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
}>()

const { t } = useI18n()
const root = ref<HTMLElement | null>(null)
const rov = ref('pen') // item that last had focus (roving tabindex)

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

// The tab stop must never sit on a disabled item: fall back to the first enabled one (the pen).
const effRov = computed(() => {
  const it = items.value.find((i) => i.id === rov.value)
  return !it || it.disabled ? 'pen' : it.id
})
// A focused button that becomes disabled drops focus to <body>; move it to the tab stop instead.
watch(effRov, async () => {
  await nextTick()
  if (props.shown && document.activeElement === document.body) root.value?.querySelector<HTMLElement>('[data-tb][tabindex="0"]')?.focus()
})
</script>

<template>
  <div ref="root" class="cc-tb-root" :class="{ hid: !shown }" :inert="shown ? undefined : true" :aria-hidden="!shown" data-testid="toolbar-root" @keydown="onKey">
    <div role="toolbar" data-testid="toolbar" :aria-label="t('toolbar.label')" class="cc-tb">
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
          :tabindex="effRov === it.id ? 0 : -1"
          @focus="rov = it.id"
          @click="it.run()"
        >
          <svg v-if="it.kind === 'icon'" viewBox="0 0 24 24" aria-hidden="true" class="cc-i"><path :d="ICONS[it.icon!]" /></svg>
          <span v-else-if="it.kind === 'color'" class="cc-sw" :style="{ background: it.hex }" aria-hidden="true" />
          <svg v-else viewBox="0 0 24 24" aria-hidden="true" class="cc-i"><circle cx="12" cy="12" :r="it.r" fill="currentColor" stroke="none" /></svg>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* Reference px (the parent is a scaled `.cd-hud` layer). Colours are the display tokens; the ink swatches keep
   the colours they draw with. Same fill, border and fade as the name tag (spec 3.5). */
.cc-tb-root {
  position: absolute;
  top: 12px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 3;
  transition: opacity 0.4s ease;
}
.cc-tb-root.hid {
  opacity: 0;
  pointer-events: none;
}
.cc-tb {
  display: flex;
  flex-wrap: nowrap;
  align-items: center;
  gap: 2px;
  height: 46px;
  padding: 0 4px;
  border-radius: 10px;
  border: 2px solid var(--line);
  background: color-mix(in srgb, var(--panel) 92%, transparent);
  color: var(--fg);
}
.cc-grp {
  display: flex;
  gap: 2px;
  padding: 0 4px;
  border-left: 1px solid var(--line);
}
.cc-grp:first-child {
  border-left: 0;
}
.cc-tb button {
  width: 36px;
  height: 36px;
  border: 2px solid transparent;
  border-radius: 8px;
  background: none;
  display: grid;
  place-items: center;
  color: var(--fg);
  padding: 0;
  cursor: pointer;
}
.cc-tb button[aria-pressed='true'] {
  border-color: var(--acc);
}
.cc-tb button:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}
.cc-tb button:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 1px;
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
  box-shadow: 0 0 0 1px var(--line);
}
@media (prefers-reduced-motion: reduce) {
  .cc-tb-root {
    transition: none;
  }
}
</style>
