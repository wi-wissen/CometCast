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

// The web app ships the "du" German set (apps/web/src/locales/de.json). The formal set (de-sie.json) is not loaded
// but kept complete: identical keys and {placeholders} as de.json. The loaded de.json must not address the reader
// formally (Sie, Ihr..., Ihnen); the one known false alarm is the "Sie" of receive.media.hint, which means "it" (the
// media file) and is part of the spec's own du column (docs/design/first-run-and-display.md, 11).
function checkGermanSie(label, deRel, sieRel) {
  const de = join(root, deRel);
  const sieFile = join(root, sieRel);
  if (![de, sieFile].every((p) => existsSync(p))) {
    return { label, status: 'skipped', problems: [], note: 'file missing: ' + [de, sieFile].filter((p) => !existsSync(p)).map((p) => p.slice(root.length + 1)).join(', ') };
  }
  const problems = [];
  try {
    const flat = (o, prefix = '') =>
      Object.entries(o).flatMap(([k, v]) => (v !== null && typeof v === 'object' ? flat(v, prefix + k + '.') : [[prefix + k, String(v)]]));
    const holders = (t) => [...t.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',');
    const du = new Map(flat(readJson(de)));
    const sie = new Map(flat(readJson(sieFile)));
    const formal = /\b(Sie|Ihr|Ihre|Ihren|Ihrem|Ihrer|Ihres|Ihnen)\b/;
    const EXCEPTIONS = new Set(['receive.media.hint']);
    for (const [k, v] of du) {
      if (formal.test(v) && !EXCEPTIONS.has(k)) problems.push(`"${k}" in de.json contains a formal form (Sie/Ihr...): the loaded German is "du"`);
      if (!sie.has(k)) problems.push(`"${k}" is in de.json but missing in de-sie.json`);
      else if (holders(v) !== holders(sie.get(k))) problems.push(`"${k}": placeholders differ between de.json and de-sie.json`);
    }
    for (const k of sie.keys()) if (!du.has(k)) problems.push(`"${k}" is in de-sie.json but missing in de.json`);
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

// Apple TV: Localizable.xcstrings holds the loaded German set ("du"), Localizable-sie.xcstrings the formal alternate
// (not loaded by default, selected by one build setting: docs/design/native-apps.md 16). Same keys and the same
// {placeholders} in en, de and the Sie set; the loaded German must not address the reader formally. "Sie" in
// receive.media.cleartext means "it" (the address): the one known false alarm besides receive.media.hint.
function checkAppleGermanSets(label, loadedRel, sieRel) {
  const a = join(root, loadedRel);
  const b = join(root, sieRel);
  if (![a, b].every((p) => existsSync(p))) return { label, status: 'skipped', problems: [], note: 'file missing: ' + [a, b].filter((p) => !existsSync(p)).map((p) => p.slice(root.length + 1)).join(', ') };
  const problems = [];
  try {
    const val = (e, l) => e?.localizations?.[l]?.stringUnit?.value;
    const holders = (t) => [...String(t).matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',');
    const x = readJson(a).strings ?? {};
    const y = readJson(b).strings ?? {};
    const formal = /\b(Sie|Ihr|Ihre|Ihren|Ihrem|Ihrer|Ihres|Ihnen)\b/;
    const EXCEPTIONS = new Set(['receive.media.hint', 'receive.media.cleartext']);
    for (const [k, e] of Object.entries(x)) {
      if (!(k in y)) { problems.push(`"${k}" is in Localizable.xcstrings but missing in Localizable-sie.xcstrings`); continue; }
      for (const l of ['en', 'de']) {
        if (typeof val(y[k], l) !== 'string' || val(y[k], l) === '') problems.push(`"${k}" has no "${l}" value in Localizable-sie.xcstrings`);
      }
      if (val(e, 'en') !== val(y[k], 'en')) problems.push(`"${k}": the English value differs between the two catalogs`);
      if (formal.test(val(e, 'de') ?? '') && !EXCEPTIONS.has(k)) problems.push(`"${k}" in Localizable.xcstrings contains a formal form (Sie/Ihr...): the loaded German is "du"`);
      for (const [n, t] of [['en', val(e, 'en')], ['sie', val(y[k], 'de')]]) {
        if (holders(t ?? '') !== holders(val(e, 'de') ?? '')) problems.push(`"${k}": placeholders differ between de and ${n}`);
      }
    }
    for (const k of Object.keys(y)) if (!(k in x)) problems.push(`"${k}" is in Localizable-sie.xcstrings but missing in Localizable.xcstrings`);
  } catch (e) {
    problems.push(`cannot read file: ${e.message}`);
  }
  return { label, status: 'checked', problems };
}

const arbKeys = (p) => Object.keys(readJson(p)).filter((k) => !k.startsWith('@'));

const results = [
  comparePair('web (en.json vs de.json)', 'apps/web/src/locales/en.json', 'apps/web/src/locales/de.json', (p) => flatten(readJson(p))),
  checkGermanSie('web (de-sie.json vs de.json)', 'apps/web/src/locales/de.json', 'apps/web/src/locales/de-sie.json'),
  comparePair('flutter (app_en.arb vs app_de.arb)', 'apps/flutter/lib/l10n/app_en.arb', 'apps/flutter/lib/l10n/app_de.arb', arbKeys),
  checkFlutterGermanSets('flutter (app_de.arb du vs sie/app_de.arb)', 'apps/flutter/lib/l10n/app_de.arb', 'apps/flutter/lib/l10n/sie/app_de.arb'),
  checkXcstrings('apple-tv (Localizable.xcstrings)', 'apps/apple-tv/Sources/CometCastTV/Localizable.xcstrings'),
  checkXcstrings('apple-tv (Localizable-sie.xcstrings)', 'apps/apple-tv/Sources/CometCastTV/Localizable-sie.xcstrings'),
  checkAppleGermanSets('apple-tv (Localizable.xcstrings du vs Localizable-sie.xcstrings)', 'apps/apple-tv/Sources/CometCastTV/Localizable.xcstrings', 'apps/apple-tv/Sources/CometCastTV/Localizable-sie.xcstrings'),
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
