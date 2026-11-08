#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
// Builds the string catalogs of the Apple TV display from the copy tables of docs/design/first-run-and-display.md
// (as shipped in the web and Flutter locale files and, for the round 5 keys, the table 15.13 of the spec itself):
//   apps/apple-tv/Sources/CometCastTV/Localizable.xcstrings       en + de "du" (shipped, loaded)
//   apps/apple-tv/Sources/CometCastTV/Localizable-sie.xcstrings   en + de "Sie" (kept complete, not loaded by default)
// The keys are the dotted keys of the spec, placeholders stay `{name}` (the app replaces them, native-apps.md 16).
// A key that the display uses but no source has fails the run. Usage: node tools/gen-appletv-strings.mjs [--check]
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const rd = (p) => JSON.parse(readFileSync(join(repo, p), 'utf8'));
const flat = (o, p = '') => Object.entries(o).flatMap(([k, v]) => (v && typeof v === 'object' ? flat(v, p + k + '.') : [[p + k, String(v)]]));
const web = { en: Object.fromEntries(flat(rd('apps/web/src/locales/en.json'))), du: Object.fromEntries(flat(rd('apps/web/src/locales/de.json'))), sie: Object.fromEntries(flat(rd('apps/web/src/locales/de-sie.json'))) };
const arb = { en: rd('apps/flutter/lib/l10n/app_en.arb'), du: rd('apps/flutter/lib/l10n/app_de.arb'), sie: rd('apps/flutter/lib/l10n/sie/app_de.arb') };
const camel = (k) => k.split('.').map((p, i) => (i ? p[0].toUpperCase() + p.slice(1) : p)).join('');

// Table 15.13 of the spec: | `key` | EN | DE du | DE Sie |  ("same as du" = the du value).
const spec = readFileSync(join(repo, 'docs/design/first-run-and-display.md'), 'utf8');
const t1513 = {};
for (const m of spec.slice(spec.indexOf('### 15.13 Copy')).matchAll(/^\| `([^`]+)`[^|]*\| (.*?) \| (.*?) \| (.*?) \|$/gm)) {
  t1513[m[1]] = { en: m[2], du: m[3], sie: /^same as du/.test(m[4]) ? m[3] : m[4] };
}

// Apple TV only copy that the spec does not list (kept from the earlier catalog; du converted by pronoun only).
const APPLE_ONLY = {
  'receive.media.cleartext': { en: 'This address uses http://, which Apple TV blocks for security. It cannot be played.', du: 'Diese Adresse verwendet http://, was Apple TV aus Sicherheitsgründen blockiert. Sie kann nicht abgespielt werden.', sie: 'Diese Adresse verwendet http://, was Apple TV aus Sicherheitsgründen blockiert. Sie kann nicht abgespielt werden.' },
  'receive.transferError.image_undecodable': { en: 'The image could not be decoded and was discarded.', du: 'Das Bild konnte nicht dekodiert werden und wurde verworfen.', sie: 'Das Bild konnte nicht dekodiert werden und wurde verworfen.' },
  'receive.transferError.image_too_many_pixels': { en: 'The image has too many pixels to be shown and was discarded.', du: 'Das Bild hat zu viele Pixel, um angezeigt zu werden, und wurde verworfen.', sie: 'Das Bild hat zu viele Pixel, um angezeigt zu werden, und wurde verworfen.' },
  'receive.media.formatUnsupported': { en: 'Apple TV cannot play this file format.', du: 'Apple TV kann dieses Dateiformat nicht abspielen.', sie: 'Apple TV kann dieses Dateiformat nicht abspielen.' },
};

const keys = `
common.back common.next common.noName common.quoted common.stepOf
mode.open mode.approval mode.locked
appearance.dark appearance.light appearance.oled appearance.auto
setup.welcome.title setup.welcome.body setup.welcome.start
setup.name.title setup.name.body setup.name.custom setup.name.s1 setup.name.s2 setup.name.s3 setup.name.s4 setup.name.s5
setup.mode.title setup.mode.fewPeople setup.mode.manyPeople setup.mode.openText setup.mode.approvalText setup.mode.later
setup.address.title setup.address.body setup.address.sourceDefault setup.address.sourceOwn setup.address.sourceBox setup.address.sourcePolicy setup.address.sourceTyped setup.address.ok setup.address.change setup.address.fieldLabel
setup.done.title setup.done.bodyRemote setup.done.finish
relay.errors.empty relay.errors.invalid relay.errors.insecure relay.errors.credentials relay.errors.path
idle.step1 idle.step2 idle.altAddress idle.locked idle.qrCaption idle.qrLocked idle.appLine idle.appLineWeb
idle.offline.title idle.offline.body idle.offline.bodyBusy idle.offline.bodyOutdatedApp
settings.open settings.title settings.close settings.closeRemote settings.join.label settings.join.openText settings.join.approvalText settings.join.lockedText settings.appearance
about.open about.title about.version about.licence about.publisher about.sourceTv about.webTv about.licences about.featured about.all
request.title request.queue request.subtitle request.check request.yes request.no request.noBlock request.aria
present.byline present.sas present.end
pause.presenter.button pause.presenter.resume pause.display.byPresenter toolbar.frozenHint
toast.joined toast.allowed toast.denied toast.blocked toast.expired toast.withdrawn toast.left toast.sessionEnded toast.openDisabled
receive.link.title receive.link.hintTv receive.link.dismiss
receive.media.play receive.media.stop receive.media.from receive.media.label receive.media.localWarning receive.media.cleartext receive.media.formatUnsupported
receive.media.error.aborted receive.media.error.network receive.media.error.decode receive.media.error.unsupported receive.media.error.blocked
receive.transfer receive.document.loading
receive.document.pageError.failed receive.document.pageError.page_out_of_range receive.document.pageError.bad_size receive.document.pageError.timeout receive.document.pageError.memory
receive.transferError.bad_message receive.transferError.unexpected_chunk receive.transferError.too_large receive.transferError.size_mismatch receive.transferError.no_transfer receive.transferError.stalled receive.transferError.bad_chunk
receive.transferError.document_unsupported receive.transferError.document_invalid receive.transferError.document_encrypted receive.transferError.document_too_large receive.transferError.document_too_many_pages receive.transferError.document_rejected receive.transferError.document_memory receive.transferError.document_timeout receive.transferError.document_failed
receive.transferError.image_undecodable receive.transferError.image_too_many_pixels
`.split(/\s+/).filter(Boolean);

function lookup(key) {
  if (APPLE_ONLY[key]) return APPLE_ONLY[key];
  if (t1513[key]) return t1513[key];
  if (web.en[key] !== undefined && web.du[key] !== undefined && web.sie[key] !== undefined) return { en: web.en[key], du: web.du[key], sie: web.sie[key] };
  const c = camel(key);
  if (arb.en[c] !== undefined) return { en: arb.en[c], du: arb.du[c], sie: arb.sie[c] };
  return null;
}

const missing = [];
const rows = keys.map((k) => {
  const v = lookup(k);
  if (v) return [k, v];
  missing.push(k);
  return [k, null];
});
if (missing.length) { console.error('no source for: ' + missing.join(', ')); process.exit(1); }

const bad = rows.filter(([, v]) => [v.en, v.du, v.sie].some((s) => typeof s !== 'string' || s === '' || /@:|\|/.test(s)));
if (bad.length) { console.error('empty or unsupported value for: ' + bad.map(([k]) => k).join(', ')); process.exit(1); }

const catalog = (german) => JSON.stringify({
  sourceLanguage: 'en',
  strings: Object.fromEntries(rows.map(([k, v]) => [k, { extractionState: 'manual', localizations: {
    en: { stringUnit: { state: 'translated', value: v.en } },
    de: { stringUnit: { state: 'translated', value: v[german] } },
  } }])),
  version: '1.0',
}, null, 2) + '\n';

const out = [
  ['apps/apple-tv/Sources/CometCastTV/Localizable.xcstrings', catalog('du')],
  ['apps/apple-tv/Sources/CometCastTV/Localizable-sie.xcstrings', catalog('sie')],
];
if (process.argv.includes('--check')) {
  const stale = out.filter(([p, t]) => !existsSync(join(repo, p)) || readFileSync(join(repo, p), 'utf8') !== t).map(([p]) => p);
  if (stale.length) { console.error('out of date, run node tools/gen-appletv-strings.mjs: ' + stale.join(', ')); process.exit(1); }
  console.log(`apple-tv catalogs OK: ${rows.length} keys`);
} else {
  for (const [p, t] of out) writeFileSync(join(repo, p), t);
  console.log(`wrote 2 catalogs, ${rows.length} keys`);
}
