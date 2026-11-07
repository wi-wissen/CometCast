// SPDX-License-Identifier: GPL-3.0-or-later
// Self-test for branding.mjs, gen-branding.mjs and the branding parts of build-site.mjs.
//   node --test tools/branding.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { loadBrandingFile, normaliseBranding, missingImprint, DEFAULT_BRANDING } from './branding.mjs';

const tools = dirname(fileURLToPath(import.meta.url));
const repo = join(tools, '..');
const official = join(repo, 'branding', 'cometcast.json');
const tmp = mkdtempSync(join(tmpdir(), 'branding-test-'));
const run = (script, args, env = {}) =>
  spawnSync(process.execPath, [join(tools, script), ...args], { encoding: 'utf8', env: { ...process.env, COMETCAST_BRANDING: '', COMETCAST_PUBLIC_BUILD: '', ...env } });
const write = (name, obj) => {
  const f = join(tmp, name);
  writeFileSync(f, JSON.stringify(obj));
  return f;
};
test.after(() => rmSync(tmp, { recursive: true, force: true }));

test('the shipped files are valid; the default names no publisher', () => {
  const d = loadBrandingFile(DEFAULT_BRANDING);
  assert.equal(d.publisher_name, '');
  assert.deepEqual(d.privacy_url, { de: '', en: '' });
  const o = loadBrandingFile(official);
  assert.equal(o.publisher_name, 'Schulverwalter GmbH');
  assert.equal(o.privacy_url.en, 'https://cometcast.de/en/privacy/');
  assert.deepEqual(missingImprint(o), ['name', 'street', 'city', 'email']);
});

test('validation: unknown keys, http URLs, a single string for both languages', () => {
  const ok = { source_url: 'https://x.example/r' };
  assert.throws(() => normaliseBranding({ ...ok, typo: 1 }), /unknown key/);
  assert.throws(() => normaliseBranding({ ...ok, privacy_url: 'http://x.example' }), /https/);
  assert.throws(() => normaliseBranding({ ...ok, imprint: { nme: 'x' } }), /unknown imprint field/);
  assert.throws(() => normaliseBranding({}), /source_url must be set/);
  assert.deepEqual(normaliseBranding({ ...ok, privacy_url: 'https://x.example/p' }).privacy_url, { de: 'https://x.example/p', en: 'https://x.example/p' });
});

test('gen-branding writes both files, --check passes, fails after an edit and for another file', () => {
  const root = join(tmp, 'gen');
  assert.equal(run('gen-branding.mjs', ['--root', root]).status, 0);
  const dart = readFileSync(join(root, 'apps/flutter/lib/src/branding.g.dart'), 'utf8');
  const swift = readFileSync(join(root, 'apps/apple-tv/Sources/CometCastTV/Branding.g.swift'), 'utf8');
  assert.match(dart, /static const String publisherName = "";/);
  assert.match(swift, /static let sourceUrl: String = "https:\/\/github.com\/wi-wissen\/CometCast"/);
  assert.equal(run('gen-branding.mjs', ['--check', '--root', root]).status, 0);
  const off = run('gen-branding.mjs', ['--check', '--root', root], { COMETCAST_BRANDING: official });
  assert.equal(off.status, 1);
  assert.match(off.stderr, /out of date/);
  assert.equal(run('gen-branding.mjs', ['--root', root], { COMETCAST_BRANDING: official }).status, 0);
  assert.match(readFileSync(join(root, 'apps/flutter/lib/src/branding.g.dart'), 'utf8'), /publisherName = "Schulverwalter GmbH"/);
  assert.match(readFileSync(join(root, 'apps/apple-tv/Sources/CometCastTV/Branding.g.swift'), 'utf8'), /privacyUrlDe: String = "https:\/\/cometcast.de\/datenschutz\/"/);
  writeFileSync(join(root, 'apps/flutter/lib/src/branding.g.dart'), dart + '// x\n');
  assert.equal(run('gen-branding.mjs', ['--check', '--root', root], { COMETCAST_BRANDING: official }).status, 1);
});

test('gen-branding escapes quotes and dollars', () => {
  const f = write('esc.json', { source_url: 'https://x.example/r', publisher_name: 'A "B" $c \\ d' });
  const root = join(tmp, 'esc');
  assert.equal(run('gen-branding.mjs', ['--root', root], { COMETCAST_BRANDING: f }).status, 0);
  assert.match(readFileSync(join(root, 'apps/flutter/lib/src/branding.g.dart'), 'utf8'), /"A \\"B\\" \\\$c \\\\ d"/);
});

test('the committed generated files match the default branding file', () => {
  assert.equal(run('gen-branding.mjs', ['--check']).status, 0);
});

const filled = { source_url: 'https://github.com/wi-wissen/CometCast', publisher_name: 'Muster e.V.', imprint: { name: 'Muster e.V.', street: 'Weg 1', city: '12345 Ort', email: 'a@b.example' } };
const buildSite = (out, env) => {
  const r = run('build-site.mjs', ['--out', out], env);
  if (r.status === 0) cpSync(join(repo, 'site', 'assets'), join(out, 'assets'), { recursive: true });
  if (r.status === 0) for (const f of ['favicon.svg', 'apple-touch-icon.png']) cpSync(join(repo, 'site', f), join(out, f));
  return r;
};

test('site: the public-build guard fails while the imprint is empty, with a clear message', () => {
  for (const file of [DEFAULT_BRANDING, official]) {
    const out = join(tmp, 'guard');
    const r = buildSite(out, { COMETCAST_BRANDING: file, COMETCAST_PUBLIC_BUILD: '1' });
    assert.equal(r.status, 1);
    assert.match(r.stderr, /PUBLIC BUILD REFUSED/);
    assert.match(r.stderr, /imprint\.name, imprint\.street, imprint\.city, imprint\.email/);
    assert.equal(existsSync(out), false, 'nothing is written');
  }
  const partial = write('partial.json', { ...filled, imprint: { ...filled.imprint, street: '' } });
  const r = buildSite(join(tmp, 'guard2'), { COMETCAST_BRANDING: partial, COMETCAST_PUBLIC_BUILD: '1' });
  assert.equal(r.status, 1);
  assert.match(r.stderr, /imprint\.street/);
  assert.match(r.stderr, /filled in: imprint\.street \(/);
});

test('site: a filled imprint passes the public build; no placeholder wording; links are clean', () => {
  const out = join(tmp, 'filled-site');
  const r = buildSite(out, { COMETCAST_BRANDING: write('filled.json', filled), COMETCAST_PUBLIC_BUILD: '1' });
  assert.equal(r.status, 0, r.stderr);
  const page = readFileSync(join(out, 'impressum/index.html'), 'utf8');
  assert.match(page, /<p>Muster e\.V\.<br>Weg 1<br>12345 Ort<\/p>/);
  assert.match(page, /<p>a@b\.example<\/p>/);
  assert.doesNotMatch(page, /todo|Platzhalter/);
  assert.match(readFileSync(join(out, 'index.html'), 'utf8'), /href="\/impressum\/"/);
  assert.match(readFileSync(join(out, 'en/index.html'), 'utf8'), /href="\/en\/imprint\/"/);
  const c = spawnSync(process.execPath, [join(tools, 'check-site-links.mjs'), out, '--without-app'], { encoding: 'utf8' });
  assert.equal(c.status, 0, c.stdout + c.stderr);
});

test('site: the empty default leaves out the imprint (pages, footer link, sitemap) and has no dead links', () => {
  const out = join(tmp, 'default-site');
  const r = buildSite(out, {});
  assert.equal(r.status, 0, r.stderr);
  assert.equal(existsSync(join(out, 'impressum')), false);
  assert.equal(existsSync(join(out, 'en/imprint')), false);
  for (const f of ['index.html', 'en/index.html', 'datenschutz/index.html', 'sitemap.xml']) assert.doesNotMatch(readFileSync(join(out, f), 'utf8'), /impressum|imprint/i);
  const c = spawnSync(process.execPath, [join(tools, 'check-site-links.mjs'), out, '--without-app'], { encoding: 'utf8' });
  assert.equal(c.status, 0, c.stdout + c.stderr);
});

test('site: the official file generates exactly the committed site/', () => {
  const out = join(tmp, 'official-site');
  assert.equal(buildSite(out, { COMETCAST_BRANDING: official }).status, 0);
  for (const f of ['index.html', 'impressum/index.html', 'en/imprint/index.html', 'datenschutz/index.html', 'en/privacy/index.html', 'en/open-source/index.html', 'kontakt/index.html', 'sitemap.xml']) {
    assert.equal(readFileSync(join(out, f), 'utf8'), readFileSync(join(repo, 'site', f), 'utf8'), f);
  }
});
