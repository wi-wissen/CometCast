#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
// Checks that every translation file has the same keys in English and German.
// Usage: node tools/check-i18n.mjs [--root <dir>]
// Exit code: 0 = all good (or files missing -> skipped), 1 = mismatch/invalid file.

import { readFileSync, existsSync } from 'node:fs';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
const rootIdx = argv.indexOf('--root');
const root = resolve(rootIdx >= 0 ? argv[rootIdx + 1] : join(here, '..'));

// Turns {a:{b:"x"}} into ["a.b"].
function flatten(obj, prefix = '') {
  return Object.entries(obj).flatMap(([k, v]) =>
    v !== null && typeof v === 'object' && !Array.isArray(v)
      ? flatten(v, prefix + k + '.')
      : [prefix + k],
  );
}

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'));
}

// Returns a list of problem strings for two key lists.
function diff(nameA, keysA, nameB, keysB) {
  const a = new Set(keysA);
  const b = new Set(keysB);
  return [
    ...[...a].filter((k) => !b.has(k)).map((k) => `"${k}" is in ${nameA} but missing in ${nameB}`),
    ...[...b].filter((k) => !a.has(k)).map((k) => `"${k}" is in ${nameB} but missing in ${nameA}`),
  ];
}

function comparePair(label, enRel, deRel, load) {
  const en = join(root, enRel);
  const de = join(root, deRel);
  const missing = [en, de].filter((p) => !existsSync(p));
  if (missing.length) {
    return { label, status: 'skipped', problems: [], note: 'file missing: ' + missing.map((p) => p.slice(root.length + 1)).join(', ') };
  }
  try {
    return { label, status: 'checked', problems: diff('en', load(en), 'de', load(de)) };
  } catch (e) {
    return { label, status: 'checked', problems: [`cannot read file: ${e.message}`] };
  }
}

function checkXcstrings(label, rel) {
  const file = join(root, rel);
  if (!existsSync(file)) return { label, status: 'skipped', problems: [], note: 'file missing: ' + rel };
  const problems = [];
  try {
    const strings = readJson(file).strings ?? {};
    for (const [key, entry] of Object.entries(strings)) {
      for (const lang of ['en', 'de']) {
        const value = entry?.localizations?.[lang]?.stringUnit?.value;
        if (typeof value !== 'string' || value === '') problems.push(`"${key}" has no "${lang}" translation`);
      }
    }
  } catch (e) {
    problems.push(`cannot read file: ${e.message}`);
  }
  return { label, status: 'checked', problems };
}

// The second German set (the "du" form, apps/web/src/locales/de-du.json) is not loaded by the app. It
// must only have keys that de.json has, with the same {placeholders} (docs/design/first-run-and-display.md, 11.1), and
// must have every key whose Sie text addresses the reader (11.8).
function checkGermanDu(label, deRel, duRel) {
  const de = join(root, deRel);
  const du = join(root, duRel);
  if (![de, du].every((p) => existsSync(p))) {
    return { label, status: 'skipped', problems: [], note: 'file missing: ' + [de, du].filter((p) => !existsSync(p)).map((p) => p.slice(root.length + 1)).join(', ') };
  }
  const problems = [];
  try {
    const flat = (o, prefix = '') =>
      Object.entries(o).flatMap(([k, v]) => (v !== null && typeof v === 'object' ? flat(v, prefix + k + '.') : [[prefix + k, String(v)]]));
    const holders = (t) => [...t.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',');
    const sie = new Map(flat(readJson(de)));
    const duKeys = new Set(flat(readJson(du)).map(([k]) => k));
    // A Sie value that addresses the reader (Sie, Ihr..., Ihnen) must have its own du value (spec 11.8), otherwise the
    // du set silently shows Sie text. The "Sie" of receive.media.hint means "it" (the media file): the one known false alarm.
    const addresses = /\b(Sie|Ihr|Ihre|Ihren|Ihrem|Ihrer|Ihres|Ihnen)\b/;
    const EXCEPTIONS = new Set(['receive.media.hint']);
    for (const [k, v] of sie) {
      if (addresses.test(v) && !EXCEPTIONS.has(k) && !duKeys.has(k)) problems.push(`"${k}" addresses the reader in de.json (Sie/Ihr...) but is missing in de-du.json`);
    }
    for (const [k, v] of flat(readJson(du))) {
      if (!sie.has(k)) problems.push(`"${k}" is in de-du.json but missing in de.json`);
      else if (holders(v) !== holders(sie.get(k))) problems.push(`"${k}": placeholders differ between de.json and de-du.json`);
    }
  } catch (e) {
    problems.push(`cannot read file: ${e.message}`);
  }
  return { label, status: 'checked', problems };
}

// Flutter: the loaded German set (app_de.arb, "du") and the formal alternate (sie/app_de.arb, not loaded) must
// have the same keys and the same {placeholders} (docs/design/first-run-and-display.md, 11.1).
function checkFlutterGermanSets(label, loadedRel, alternateRel) {
  const a = join(root, loadedRel);
  const b = join(root, alternateRel);
  if (![a, b].every((p) => existsSync(p))) return { label, status: 'skipped', problems: [], note: 'file missing' };
  const problems = [];
  try {
    const x = readJson(a);
    const y = readJson(b);
    const holders = (t) => [...String(t).matchAll(/\{(\w+)/g)].map((m) => m[1]).sort().join(',');
    for (const k of Object.keys(x).filter((k) => !k.startsWith('@'))) {
      if (!(k in y)) problems.push(`"${k}" is in app_de.arb but missing in sie/app_de.arb`);
      else if (holders(x[k]) !== holders(y[k])) problems.push(`"${k}": placeholders differ between the two German sets`);
    }
    for (const k of Object.keys(y).filter((k) => !k.startsWith('@'))) if (!(k in x)) problems.push(`"${k}" is in sie/app_de.arb but missing in app_de.arb`);
  } catch (e) {
    problems.push(`cannot read file: ${e.message}`);
  }
  return { label, status: 'checked', problems };
}

const arbKeys = (p) => Object.keys(readJson(p)).filter((k) => !k.startsWith('@'));

const results = [
  comparePair('web (en.json vs de.json)', 'apps/web/src/locales/en.json', 'apps/web/src/locales/de.json', (p) => flatten(readJson(p))),
  checkGermanDu('web (de-du.json vs de.json)', 'apps/web/src/locales/de.json', 'apps/web/src/locales/de-du.json'),
  comparePair('flutter (app_en.arb vs app_de.arb)', 'apps/flutter/lib/l10n/app_en.arb', 'apps/flutter/lib/l10n/app_de.arb', arbKeys),
  checkFlutterGermanSets('flutter (app_de.arb du vs sie/app_de.arb)', 'apps/flutter/lib/l10n/app_de.arb', 'apps/flutter/lib/l10n/sie/app_de.arb'),
  checkXcstrings('apple-tv (Localizable.xcstrings)', 'apps/apple-tv/Sources/CometCastTV/Localizable.xcstrings'),
];

let failed = false;
for (const r of results) {
  if (r.status === 'skipped') {
    console.log(`SKIP  ${r.label}: skipped: ${r.note}`);
  } else if (r.problems.length) {
    failed = true;
    console.log(`FAIL  ${r.label}`);
    r.problems.forEach((p) => console.log(`        - ${p}`));
  } else {
    console.log(`OK    ${r.label}`);
  }
}
console.log(failed ? '\ni18n check FAILED' : '\ni18n check passed');
process.exit(failed ? 1 : 0);
