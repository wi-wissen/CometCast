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
