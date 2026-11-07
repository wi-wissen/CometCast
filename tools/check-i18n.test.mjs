// SPDX-License-Identifier: GPL-3.0-or-later
// Self-test: node --test tools/
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const script = join(dirname(fileURLToPath(import.meta.url)), 'check-i18n.mjs');

function fixture(files) {
  const root = mkdtempSync(join(tmpdir(), 'i18n-'));
  for (const [rel, content] of Object.entries(files)) {
    mkdirSync(dirname(join(root, rel)), { recursive: true });
    writeFileSync(join(root, rel), JSON.stringify(content));
  }
  return root;
}
const run = (root) => spawnSync('node', [script, '--root', root], { encoding: 'utf8' });

test('empty root: everything skipped, exit 0', () => {
  const r = run(fixture({}));
  assert.equal(r.status, 0);
  assert.match(r.stdout, /skipped: file missing/);
});

test('matching nested web files pass', () => {
  const r = run(fixture({
    'apps/web/src/locales/en.json': { home: { a: 'A' } },
    'apps/web/src/locales/de.json': { home: { a: 'Ä' } },
  }));
  assert.equal(r.status, 0);
});

test('missing web key fails', () => {
  const r = run(fixture({
    'apps/web/src/locales/en.json': { home: { a: 'A', b: 'B' } },
    'apps/web/src/locales/de.json': { home: { a: 'Ä' } },
  }));
  assert.equal(r.status, 1);
  assert.match(r.stdout, /home\.b/);
});

test('arb ignores @ metadata', () => {
  const r = run(fixture({
    'apps/flutter/lib/l10n/app_en.arb': { '@@locale': 'en', k: 'K', '@k': { description: 'd' } },
    'apps/flutter/lib/l10n/app_de.arb': { '@@locale': 'de', k: 'K' },
  }));
  assert.equal(r.status, 0);
});

test('xcstrings key without de fails', () => {
  const r = run(fixture({
    'apps/apple-tv/Sources/CometCastTV/Localizable.xcstrings': {
      sourceLanguage: 'en', version: '1.0',
      strings: { k: { localizations: { en: { stringUnit: { state: 'translated', value: 'x' } } } } },
    },
  }));
  assert.equal(r.status, 1);
  assert.match(r.stdout, /"de"/);
});

test('de-sie.json: same keys and placeholders as de.json', () => {
  const ok = run(fixture({
    'apps/web/src/locales/de.json': { a: 'Hallo {name}', b: 'B' },
    'apps/web/src/locales/de-sie.json': { a: 'Hallo {name}', b: 'B' },
  }));
  assert.equal(ok.status, 0);
  const extra = run(fixture({
    'apps/web/src/locales/de.json': { a: 'A' },
    'apps/web/src/locales/de-sie.json': { a: 'A', z: 'Z' },
  }));
  assert.equal(extra.status, 1);
  assert.match(extra.stdout, /"z" is in de-sie.json but missing in de.json/);
  const missing = run(fixture({
    'apps/web/src/locales/de.json': { a: 'A', y: 'Y' },
    'apps/web/src/locales/de-sie.json': { a: 'A' },
  }));
  assert.equal(missing.status, 1);
  assert.match(missing.stdout, /"y" is in de.json but missing in de-sie.json/);
  const ph = run(fixture({
    'apps/web/src/locales/de.json': { a: 'Hallo {name}' },
    'apps/web/src/locales/de-sie.json': { a: 'Hallo {room}' },
  }));
  assert.equal(ph.status, 1);
  assert.match(ph.stdout, /placeholders differ/);
});

test('de.json: the loaded German is du, no formal forms (Sie, Ihr..., Ihnen)', () => {
  const bad = run(fixture({
    'apps/web/src/locales/de.json': { a: 'Bitte geben Sie den Code ein', b: 'Ihr Gerät', c: 'Neutral' },
    'apps/web/src/locales/de-sie.json': { a: 'x', b: 'y', c: 'z' },
  }));
  assert.equal(bad.status, 1);
  assert.match(bad.stdout, /"a" in de.json contains a formal form/);
  assert.match(bad.stdout, /"b" in de.json contains a formal form/);
  assert.doesNotMatch(bad.stdout, /"c" in de.json/);
  const ok = run(fixture({
    'apps/web/src/locales/de.json': { a: 'Bitte gib den Code ein', b: 'Dein Gerät' },
    'apps/web/src/locales/de-sie.json': { a: 'Bitte geben Sie den Code ein', b: 'Ihr Gerät' },
  }));
  assert.equal(ok.status, 0);
  // the one known false alarm
  const hint = run(fixture({
    'apps/web/src/locales/de.json': { 'receive.media.hint': 'Sie wird nur geladen' },
    'apps/web/src/locales/de-sie.json': { 'receive.media.hint': 'Sie wird nur geladen' },
  }));
  assert.equal(hint.status, 0);
});

const xc = (strings) => ({
  sourceLanguage: 'en', version: '1.0',
  strings: Object.fromEntries(Object.entries(strings).map(([k, [en, de]]) => [k, { localizations: { en: { stringUnit: { state: 'translated', value: en } }, de: { stringUnit: { state: 'translated', value: de } } } }])),
});
const APPLE = 'apps/apple-tv/Sources/CometCastTV/';

test('apple-tv: the Sie catalog has the same keys, English and placeholders as the du catalog', () => {
  const ok = run(fixture({
    [APPLE + 'Localizable.xcstrings']: xc({ a: ['Hello {name}', 'Hallo {name}'], b: ['B', 'Dein B'] }),
    [APPLE + 'Localizable-sie.xcstrings']: xc({ a: ['Hello {name}', 'Hallo {name}'], b: ['B', 'Ihr B'] }),
  }));
  assert.equal(ok.status, 0, ok.stdout);
  const missing = run(fixture({
    [APPLE + 'Localizable.xcstrings']: xc({ a: ['A', 'A'], b: ['B', 'B'] }),
    [APPLE + 'Localizable-sie.xcstrings']: xc({ a: ['A', 'A'] }),
  }));
  assert.equal(missing.status, 1);
  assert.match(missing.stdout, /"b" is in Localizable.xcstrings but missing in Localizable-sie.xcstrings/);
  const extra = run(fixture({
    [APPLE + 'Localizable.xcstrings']: xc({ a: ['A', 'A'] }),
    [APPLE + 'Localizable-sie.xcstrings']: xc({ a: ['A', 'A'], z: ['Z', 'Z'] }),
  }));
  assert.match(extra.stdout, /"z" is in Localizable-sie.xcstrings but missing in Localizable.xcstrings/);
  const ph = run(fixture({
    [APPLE + 'Localizable.xcstrings']: xc({ a: ['Hello {name}', 'Hallo {name}'] }),
    [APPLE + 'Localizable-sie.xcstrings']: xc({ a: ['Hello {name}', 'Hallo {room}'] }),
  }));
  assert.equal(ph.status, 1);
  assert.match(ph.stdout, /placeholders differ between de and sie/);
});

test('apple-tv: the loaded German catalog is du (formal forms fail, the address "Sie" is excepted)', () => {
  const bad = run(fixture({
    [APPLE + 'Localizable.xcstrings']: xc({ a: ['A', 'Geben Sie den Code ein'], c: ['C', 'Neutral'] }),
    [APPLE + 'Localizable-sie.xcstrings']: xc({ a: ['A', 'x'], c: ['C', 'z'] }),
  }));
  assert.equal(bad.status, 1);
  assert.match(bad.stdout, /"a" in Localizable.xcstrings contains a formal form/);
  assert.doesNotMatch(bad.stdout, /"c" in Localizable.xcstrings/);
  const ok = run(fixture({
    [APPLE + 'Localizable.xcstrings']: xc({ 'receive.media.cleartext': ['E', 'Sie kann nicht abgespielt werden.'] }),
    [APPLE + 'Localizable-sie.xcstrings']: xc({ 'receive.media.cleartext': ['E', 'Sie kann nicht abgespielt werden.'] }),
  }));
  assert.equal(ok.status, 0, ok.stdout);
});
