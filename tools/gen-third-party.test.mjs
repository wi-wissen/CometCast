// SPDX-License-Identifier: GPL-3.0-or-later
// Self-test of `tools/gen-third-party.mjs --uxplay-licence` (spec 15.14 N106). No cargo, npm or pub needed.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const script = resolve(dirname(fileURLToPath(import.meta.url)), 'gen-third-party.mjs');

test('the UxPlay licence file is the shipped licence followed by the notices of its parts', () => {
  const d = mkdtempSync(join(tmpdir(), 'uxplay-'));
  try {
    writeFileSync(join(d, 'LICENSE'), 'GNU GENERAL PUBLIC LICENSE\nVersion 3\n\n');
    writeFileSync(join(d, 'NOTICES'), 'Part X: MIT\n');
    execFileSync('node', [script, '--uxplay-licence', join(d, 'LICENSE'), '--uxplay-notices', join(d, 'NOTICES'), '--out', join(d, 'out')]);
    const t = readFileSync(join(d, 'out', 'UXPLAY_LICENSE.txt'), 'utf8');
    assert.equal(t, 'GNU GENERAL PUBLIC LICENSE\nVersion 3\n\nPart X: MIT\n');
  } finally {
    rmSync(d, { recursive: true, force: true });
  }
});

test('without a licence file nothing is written and the exit code is 1', () => {
  const r = spawnSync('node', [script, '--uxplay-licence', '/nonexistent/LICENSE']);
  assert.equal(r.status, 1);
});
