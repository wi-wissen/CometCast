<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
// The icons of the approved mockup (docs/design/first-run-and-display.md; stroke icons, 24 x 24).
import { computed } from 'vue'

export type IconName =
  | 'open'
  | 'approval'
  | 'locked'
  | 'gear'
  | 'close'
  | 'stop'
  | 'wifiOff'
  | 'globe'
  | 'download'
  | 'info'
  | 'check'
  | 'x'
  | 'clock'
  | 'screen'
  | 'file'
  | 'link'
  | 'scan'

const props = defineProps<{ name: IconName; width?: number }>()

const PATHS: Record<IconName, string[]> = {
  open: ['<rect x="4" y="11" width="16" height="10" rx="2"/>', '<path d="M8 11V7a4 4 0 0 1 7.5-2"/>'],
  approval: ['<circle cx="9" cy="8" r="4"/>', '<path d="M2 21c0-4 3-6 7-6s7 2 7 6"/>', '<path d="M16 11l2 2 4-4"/>'],
  locked: ['<rect x="4" y="11" width="16" height="10" rx="2"/>', '<path d="M8 11V7a4 4 0 0 1 8 0v4"/>'],
  gear: [
    '<path d="M10.3 2.6h3.4l.5 2.6a7.5 7.5 0 0 1 1.9 1.1l2.5-.9 1.7 2.9-2 1.7a7.6 7.6 0 0 1 0 2.2l2 1.7-1.7 2.9-2.5-.9a7.5 7.5 0 0 1-1.9 1.1l-.5 2.6h-3.4l-.5-2.6a7.5 7.5 0 0 1-1.9-1.1l-2.5.9-1.7-2.9 2-1.7a7.6 7.6 0 0 1 0-2.2l-2-1.7 1.7-2.9 2.5.9a7.5 7.5 0 0 1 1.9-1.1z"/>',
    '<circle cx="12" cy="12" r="3"/>',
  ],
  close: ['<path d="M6 6l12 12M18 6L6 18"/>'],
  stop: ['<rect x="5" y="5" width="14" height="14" rx="2"/>'],
  wifiOff: [
    '<path d="M2 2l20 20"/>',
    '<path d="M8.5 16.5a5 5 0 0 1 7 0"/>',
    '<path d="M5 12.9a10 10 0 0 1 5.2-2.8"/>',
    '<path d="M19 12.9a10 10 0 0 0-2.3-1.6"/>',
    '<path d="M12 20h.01"/>',
  ],
  globe: ['<circle cx="12" cy="12" r="9"/>', '<path d="M3 12h18M12 3c2.5 2.7 3.7 5.7 3.7 9s-1.2 6.3-3.7 9c-2.5-2.7-3.7-5.7-3.7-9S9.5 5.7 12 3z"/>'],
  download: ['<path d="M12 3v12M7 10l5 5 5-5M5 21h14"/>'],
  info: ['<circle cx="12" cy="12" r="9"/>', '<path d="M12 11v6M12 7.5h.01"/>'],
  check: ['<path d="M5 12l5 5 9-10"/>'],
  x: ['<path d="M6 6l12 12M18 6L6 18"/>'],
  clock: ['<circle cx="12" cy="12" r="9"/>', '<path d="M12 7v5l3 2"/>'],
  screen: ['<rect x="3" y="4" width="18" height="12" rx="2"/>', '<path d="M8 20h8M12 16v4"/>'],
  file: ['<path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/>', '<path d="M14 3v6h6"/>'],
  link: ['<path d="M10 14a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1"/>', '<path d="M14 10a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1"/>'],
  scan: ['<path d="M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3"/>'],
}
// Stroke weights of the mockup: 2 by default; the tick is heavier.
const WIDTH: Partial<Record<IconName, number>> = { check: 2.6, stop: 2.4, x: 2.4 }
const html = computed(() => PATHS[props.name].join(''))
const sw = computed(() => props.width ?? WIDTH[props.name] ?? 2)
</script>

<template>
  <svg
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    :stroke-width="sw"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    focusable="false"
    v-html="html"
  />
</template>
