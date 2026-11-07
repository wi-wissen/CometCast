#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
// Exports the stroke icons of apps/web/src/components/Icon.vue (the source of truth, docs/design/native-apps.md 13)
// as SVG files (24 x 24, fill none, round caps and joins, the stroke widths of Icon.vue) to assets/icons, and the
// ones the Apple TV display uses as template image sets with preserved vector data to
// apps/apple-tv/Assets.xcassets/Icons. Usage: node tools/gen-appletv-icons.mjs [--check]
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const vue = readFileSync(join(repo, 'apps/web/src/components/Icon.vue'), 'utf8');
const lit = (name) => {
  const m = vue.match(new RegExp(`const ${name}[^=]*= (\\{[\\s\\S]*?\\})`));
  if (!m) throw new Error(`${name} not found in Icon.vue`);
  return new Function(`return ${m[1]}`)();
};
const PATHS = lit('PATHS');
const WIDTH = lit('WIDTH');
// Icons of the Apple TV display: mode pill and tiles, settings, dialogs, End, offline box, address card, tick, info line.
const APPLE_TV = ['open', 'approval', 'locked', 'gear', 'close', 'stop', 'wifiOff', 'globe', 'info', 'check', 'x'];

const svg = (name, color) =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="${color}" stroke-width="${WIDTH[name] ?? 2}" stroke-linecap="round" stroke-linejoin="round">${PATHS[name].join('')}</svg>\n`;
const contents = (name) =>
  JSON.stringify({ images: [{ filename: `${name}.svg`, idiom: 'universal' }], info: { author: 'xcode', version: 1 }, properties: { 'preserves-vector-representation': true, 'template-rendering-intent': 'template' } }, null, 2) + '\n';

const files = new Map();
for (const name of Object.keys(PATHS)) files.set(join(repo, 'assets/icons', `${name}.svg`), svg(name, 'currentColor'));
const cat = join(repo, 'apps/apple-tv/Assets.xcassets/Icons');
files.set(join(cat, 'Contents.json'), JSON.stringify({ info: { author: 'xcode', version: 1 }, properties: { 'provides-namespace': false } }, null, 2) + '\n');
for (const name of APPLE_TV) {
  files.set(join(cat, `icon-${name}.imageset`, `${name}.svg`), svg(name, '#000000'));
  files.set(join(cat, `icon-${name}.imageset`, 'Contents.json'), contents(name));
}

// The logos (assets/logo): dark on Dark and OLED black, light on Light (spec 3.2), as vectors, original colours.
const logoCat = join(repo, 'apps/apple-tv/Assets.xcassets/Logo');
files.set(join(logoCat, 'Contents.json'), JSON.stringify({ info: { author: 'xcode', version: 1 }, properties: { 'provides-namespace': false } }, null, 2) + '\n');
for (const v of ['dark', 'light']) {
  const dir = join(logoCat, `logo-${v}.imageset`);
  files.set(join(dir, `cometcast-logo-${v}.svg`), readFileSync(join(repo, `assets/logo/cometcast-logo-${v}.svg`), 'utf8'));
  files.set(join(dir, 'Contents.json'), JSON.stringify({ images: [{ filename: `cometcast-logo-${v}.svg`, idiom: 'universal' }], info: { author: 'xcode', version: 1 }, properties: { 'preserves-vector-representation': true, 'template-rendering-intent': 'original' } }, null, 2) + '\n');
}

if (process.argv.includes('--check')) {
  const bad = [...files].filter(([p, t]) => !existsSync(p) || readFileSync(p, 'utf8') !== t).map(([p]) => p.slice(repo.length + 1));
  if (bad.length) { console.error('icons out of date, run node tools/gen-appletv-icons.mjs:\n' + bad.join('\n')); process.exit(1); }
  console.log(`icons OK: ${files.size} files`);
} else {
  for (const [p, t] of files) { mkdirSync(dirname(p), { recursive: true }); writeFileSync(p, t); }
  console.log(`wrote ${files.size} files`);
}
