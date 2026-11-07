// SPDX-License-Identifier: GPL-3.0-or-later
// Self-test for set-version.mjs: runs it on a temporary copy of the files it edits.
//   node --test tools/set-version.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { cpSync, mkdtempSync, mkdirSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { SEMVER, setVersion } from './set-version.mjs';

const repo = join(dirname(fileURLToPath(import.meta.url)), '..');
const files = [
  'Cargo.toml', 'Cargo.lock', 'crates/cc-ffi/Cargo.toml', 'apps/flutter/pubspec.yaml',
  'apps/flutter/android/app/build.gradle.kts', 'apps/flutter/ios/Runner/Info.plist',
  'apps/flutter/macos/Runner/Info.plist', 'apps/flutter/windows/runner/Runner.rc',
  'apps/web/package.json', 'apps/web/package-lock.json', 'apps/apple-tv/project.yml',
  'deploy/docker-compose.yml',
];
function scratch() {
  const root = mkdtempSync(join(tmpdir(), 'setver-'));
  for (const f of files) {
    mkdirSync(dirname(join(root, f)), { recursive: true });
    cpSync(join(repo, f), join(root, f));
  }
  return root;
}
const read = (root, f) => readFileSync(join(root, f), 'utf8');

test('semver validation', () => {
  for (const ok of ['0.1.0', '1.2.3', '10.20.30-rc.1', '1.0.0-alpha-1']) assert.ok(SEMVER.test(ok), ok);
  for (const bad of ['1.2', '01.2.3', 'v1.2.3', '1.2.3+b', '1.2.3-', '1.2.3-01', '']) assert.ok(!SEMVER.test(bad), bad);
});

test('sets every file, is idempotent, keeps the lock file usable', () => {
  const root = scratch();
  const opts = { version: '2.3.4-rc.1', build: 9, image: null, h264: 'libloading', root };
  setVersion(opts);
  assert.match(read(root, 'Cargo.toml'), /\[workspace\.package\]\nversion = "2\.3\.4-rc\.1"/);
  assert.match(read(root, 'Cargo.lock'), /name = "cc-proto"\nversion = "2\.3\.4-rc\.1"/);
  assert.match(read(root, 'apps/flutter/pubspec.yaml'), /^version: 2\.3\.4\+9$/m);
  assert.match(read(root, 'apps/web/package.json'), /"version": "2\.3\.4-rc\.1"/);
  assert.match(read(root, 'apps/apple-tv/project.yml'), /MARKETING_VERSION: "2\.3\.4"/);
  assert.match(read(root, 'deploy/docker-compose.yml'), /image: ghcr\.io\/wi-wissen\/cometcast-server:2\.3\.4-rc\.1/);
  assert.match(read(root, 'crates/cc-ffi/Cargo.toml'), /default = \["h264-libloading"\]/);
  const before = files.map((f) => read(root, f));
  setVersion(opts);
  assert.deepEqual(files.map((f) => read(root, f)), before);
});
