// SPDX-License-Identifier: GPL-3.0-or-later
// Self-test of tools/release-summary.mjs with temporary asset folders.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

const script = new URL('./release-summary.mjs', import.meta.url).pathname;

function run(files) {
  const dir = mkdtempSync(join(tmpdir(), 'rs-'));
  const assets = join(dir, 'assets');
  spawnSync('mkdir', ['-p', assets]);
  for (const f of files) writeFileSync(join(assets, f), 'x');
  const out = join(dir, 'notes.md');
  const r = spawnSync('node', [script, '--version', '1.0.0', '--assets', assets, '--out', out], {
    env: { ...process.env, NEEDS_JSON: '{}' },
    encoding: 'utf8',
  });
  return { status: r.status, stderr: r.stderr, notes: readFileSync(out, 'utf8') };
}

test('fails when there is no file at all', () => {
  assert.equal(run([]).status, 1);
});

test('fails when only meta files exist', () => {
  const r = run(['LICENSE', 'THIRD_PARTY_NOTICES.md', 'cometcast-sbom-web-1.0.0.cdx.json', 'cometcast-deploy-1.0.0.tar.gz']);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /no platform artefact/);
});

test('passes with one platform artefact', () => {
  assert.equal(run(['LICENSE', 'cometcast-web-1.0.0.zip']).status, 0);
});

test('lists NO-CORE and UNSIGNED files as not shippable', () => {
  const r = run(['cometcast-ios-1.0.0-NO-CORE.ipa', 'cometcast-ios-1.0.0-UNSIGNED-NO-CORE.xcarchive.zip', 'cometcast-web-1.0.0.zip']);
  assert.equal(r.status, 0);
  assert.match(r.notes, /`cometcast-ios-1\.0\.0-NO-CORE\.ipa`/);
  assert.doesNotMatch(r.notes, /- `cometcast-web/);
});
