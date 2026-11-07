#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
// Generates THIRD_PARTY_LICENSES.md from the lock files:
//   * Rust: `cargo metadata --locked` (normal and build dependencies of the workspace members, all features, all
//     targets; dev-dependencies are left out). Licence, copyright and NOTICE files are read from the
//     crate sources in the cargo registry (so the crates must be downloaded: run `cargo fetch --locked` first).
//   * npm: production packages of apps/web/package-lock.json (the `license` field of the lock file).
//   * Flutter: apps/flutter/pubspec.lock; licence files are read from the pub cache when the package is there.
// The header records the SHA-256 of the three lock files. `tools/release-guard.sh licenses-fresh` fails
// when one of them changed after the file was generated.
// The two root files (THIRD_PARTY_LICENSES.md and the hand-written THIRD_PARTY_NOTICES.md) are copied for the
// apps (native-apps.md 21.7): unchanged to apps/flutter/assets/licenses/ and apps/apple-tv/Resources/Licenses/,
// joined (notices first) to apps/web/public/third-party-licenses.txt (the web footer's "Licences" link).
// `tools/release-guard.sh licenses-fresh` also fails when a copy differs from the root files.
// Usage: node tools/gen-third-party.mjs [--check]     (--check: do not write, fail when out of date)
//        node tools/gen-third-party.mjs --copies-only [--check]   (only the copies, no cargo/npm/pub needed)
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const OUT = join(repo, 'THIRD_PARTY_LICENSES.md');
const LOCKS = { 'Cargo.lock': 'Cargo.lock', 'apps/web/package-lock.json': 'apps/web/package-lock.json', 'apps/flutter/pubspec.lock': 'apps/flutter/pubspec.lock' };
const sha = (b) => createHash('sha256').update(b).digest('hex');
// Top-level file names that carry a licence, copyright or notice text (no source files such as license.rs).
const LICENSE_FILE = /^(.*[-_])?(licen[cs]e|copying|notice|unlicense|copyright|patents|ofl)([-_.].*)?$/i;
const SOURCE_EXT = /\.(rs|py|js|mjs|ts|c|h|cc|cpp|toml|json|lock|yml|yaml|sh|swift|java|kt|dart|orig|in)$/i;

function licenceFiles(dir) {
  if (!existsSync(dir)) return [];
  const found = [];
  const walk = (d, depth) => {
    for (const e of readdirSync(d, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      if (e.isFile() && LICENSE_FILE.test(e.name) && !SOURCE_EXT.test(e.name)) found.push(join(d, e.name));
      else if (e.isDirectory() && depth < 1 && /^(licen[cs]es?|legal|fonts|assets|third[-_ ]?party)$/i.test(e.name)) walk(join(d, e.name), depth + 1);
    }
  };
  walk(dir, 0);
  return found;
}
const normalise = (t) => t.replace(/\r\n/g, '\n').trim();
const fence = (t) => '```text\n' + t.replace(/```/g, '` ` `') + '\n```';

/** Groups packages by identical licence text. Returns { groups: [{text, files, pkgs}], without: [pkg] }. */
function groupTexts(items) {
  const byHash = new Map();
  const without = [];
  for (const it of items) {
    if (!it.files.length) { without.push(it); continue; }
    for (const f of it.files) {
      const text = normalise(readFileSync(f, 'utf8'));
      if (!text) continue;
      const h = sha(text);
      const g = byHash.get(h) ?? { text, pkgs: new Set() };
      g.pkgs.add(`${it.name} ${it.version}`);
      byHash.set(h, g);
    }
    if (!it.files.some((f) => normalise(readFileSync(f, 'utf8')))) without.push(it);
  }
  return { groups: [...byHash.values()].sort((a, b) => [...a.pkgs][0].localeCompare([...b.pkgs][0])), without };
}
function renderGroups({ groups, without }) {
  let s = '';
  for (const g of groups) {
    const pk = [...g.pkgs].sort();
    s += `\n#### ${pk.length > 6 ? pk.slice(0, 6).join(', ') + ` and ${pk.length - 6} more` : pk.join(', ')}\n\n`;
    if (pk.length > 6) s += `Used by: ${pk.join(', ')}\n\n`;
    s += fence(g.text) + '\n';
  }
  if (without.length) {
    s += `\n#### Packages without a licence file in their source\n\nThe licence below is the SPDX expression the package declares; no separate licence text was shipped with the package source: ${without.map((p) => `${p.name} ${p.version} (${p.license})`).join(', ')}.\n`;
  }
  return s;
}

// ---------------------------------------------------------------- Rust
function rust() {
  const meta = JSON.parse(execFileSync('cargo', ['metadata', '--format-version', '1', '--locked', '--all-features'], { cwd: repo, maxBuffer: 1 << 30 }).toString());
  const pkgs = new Map(meta.packages.map((p) => [p.id, p]));
  const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
  const seen = new Set();
  const stack = [...meta.workspace_members];
  while (stack.length) {
    const id = stack.pop();
    if (seen.has(id)) continue;
    seen.add(id);
    for (const d of nodes.get(id)?.deps ?? []) {
      if (d.dep_kinds.some((k) => k.kind === null || k.kind === 'build')) stack.push(d.pkg);
    }
  }
  const items = [...seen].map((id) => pkgs.get(id)).filter((p) => p.source || p.manifest_path.includes('/vendor/')) // vendor/: patched third-party crates (vendor/mdns-sd)
    .map((p) => ({ name: p.name, version: p.version, license: p.license ?? (p.license_file ? `see file ${p.license_file}` : 'UNKNOWN'), files: licenceFiles(dirname(p.manifest_path)) }))
    .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
  const missing = items.filter((i) => !existsSync(i.files[0] ?? '/nonexistent') && !i.files.length);
  if (!items.length) throw new Error('no Rust packages found');
  return { items, missing };
}

// ---------------------------------------------------------------- npm
function npm() {
  const lock = JSON.parse(readFileSync(join(repo, 'apps/web/package-lock.json'), 'utf8'));
  return Object.entries(lock.packages).filter(([k, v]) => k && !v.dev && !v.optional)
    .map(([k, v]) => ({ name: k.replace(/^.*node_modules\//, ''), version: v.version, license: v.license ?? 'UNKNOWN', files: [] }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

// ---------------------------------------------------------------- Flutter (pub)
function pub() {
  const text = readFileSync(join(repo, 'apps/flutter/pubspec.lock'), 'utf8');
  const cache = join(process.env.PUB_CACHE ?? join(homedir(), '.pub-cache'), 'hosted', 'pub.dev');
  const out = [];
  for (const m of text.matchAll(/^ {2}(\S+):\n((?: {4}.*\n)+)/gm)) {
    const body = m[2];
    const get = (k) => body.match(new RegExp(`^ {4}${k}: "?([^"\\n]+)"?$`, 'm'))?.[1];
    const source = get('source');
    if (source !== 'hosted') continue; // sdk packages (flutter, flutter_test, ...) are covered by the Flutter SDK licences
    const version = get('version');
    out.push({ name: m[1], version, dependency: get('dependency'), files: licenceFiles(join(cache, `${m[1]}-${version}`)) });
  }
  return out.sort((a, b) => a.name.localeCompare(b.name));
}

function build() {
  const hashes = Object.fromEntries(Object.entries(LOCKS).map(([k, f]) => [k, sha(readFileSync(join(repo, f)))]));
  const r = rust();
  const n = npm();
  const p = pub();
  const rg = groupTexts(r.items);
  const pg = groupTexts(p.filter((x) => x.files.length));
  const pubNoText = p.filter((x) => !x.files.length);
  const counts = new Map();
  for (const i of r.items) counts.set(i.license, (counts.get(i.license) ?? 0) + 1);

  const fontDir = join(repo, 'crates/cc-office/fonts');
  const fontTexts = [['Liberation 2.1.5 (Sans, Serif, Mono)', 'LICENSE-Liberation.txt'], ['Carlito 20230309', 'OFL-Carlito.txt'], ['Caladea 20200211', 'OFL-Caladea.txt']]
    .map(([n, f]) => `#### ${n}\n\n${fence(normalise(readFileSync(join(fontDir, f), 'utf8')))}\n`).join('\n');
  let s = `<!-- GENERATED by tools/gen-third-party.mjs. Do not edit by hand. -->
# Third-party licences (generated)

This file lists the third-party software that the Comet Cast source tree depends on, taken from the lock
files, with the licence texts found in the package sources. It complements the hand-written
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) (what is bundled, what is only downloaded at run time,
special terms such as MPL-2.0 and the AVC patent statement). It is not legal advice.

Regenerate with \`cargo fetch --locked && node tools/gen-third-party.mjs\` (the Flutter section reads the pub
cache: run \`flutter pub get\` in \`apps/flutter\` first). \`tools/release-guard.sh licenses-fresh\` fails a release
when one of the lock files below changed after this file was generated.

\`\`\`text
Cargo.lock-sha256: ${hashes['Cargo.lock']}
package-lock.json-sha256: ${hashes['apps/web/package-lock.json']}
pubspec.lock-sha256: ${hashes['apps/flutter/pubspec.lock']}
\`\`\`

Scope and limits:

- Rust: ${r.items.length} external crates, the normal and build dependencies of all workspace members with **all features** (including optional ones such as the OpenH264 source build) for **all
  target platforms together** (a given binary contains a subset; for example the \`windows\` crates are only
  in Windows builds). Dev-dependencies are not listed. The licence shown is the SPDX expression of the crate; where a
  crate offers a choice (\`MIT OR Apache-2.0\`) the recipient may pick either. The texts come from the crate sources.
- npm: the production dependency tree of \`apps/web/package-lock.json\`. What ends up in the built web app is Vue,
  vue-i18n and their \`@vue/*\` and \`@intlify/*\` runtime packages; the other entries are part of the production tree
  of the lock file (for example Vue's template compiler) and are listed to be safe. Licence texts of npm packages
  are not collected here; each package's licence is the SPDX identifier from the lock file.
- Flutter (pub): all \`hosted\` packages of \`apps/flutter/pubspec.lock\`, including packages that are only used by tests (the lock
  file does not say which). SDK packages are covered by the Flutter SDK's own licences (BSD-3-Clause, plus the notices
  of the engine's dependencies, which are **not** collected here).
- Not covered by lock files (see THIRD_PARTY_NOTICES.md): UxPlay, yt-dlp, coturn, Cisco OpenH264, the LiveKit WebRTC
  xcframework and Swift packages of the Apple TV app, fonts.

## Rust crates: licence summary

| Licence expression | Crates |
| --- | ---: |
${[...counts.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).map(([l, c]) => `| ${l.replace(/\|/g, '\\|')} | ${c} |`).join('\n')}

## Rust crates

| Crate | Version | Licence |
| --- | --- | --- |
${r.items.map((i) => `| ${i.name} | ${i.version} | ${i.license.replace(/\|/g, '\\|')} |`).join('\n')}

### Rust licence texts

Identical texts are printed once; the heading names the crates that carry that text.
${renderGroups(rg)}
## npm packages (apps/web, production tree)

| Package | Version | Licence |
| --- | --- | --- |
${n.map((i) => `| ${i.name} | ${i.version} | ${i.license} |`).join('\n')}

## Fonts bundled in the repository (cc-office)

Metric-compatible Office fonts, compiled into \`cc-office-worker\` (details, sources and checksums: \`crates/cc-office/fonts/FONTS.md\`).
All under the SIL Open Font License 1.1; the files are unmodified.

${fontTexts}
## Flutter (pub) packages

| Package | Version | Kind |
| --- | --- | --- |
${p.map((i) => `| ${i.name} | ${i.version} | ${i.dependency} |`).join('\n')}

### Flutter licence texts
${pg.groups.length ? renderGroups({ groups: pg.groups, without: [] }) : '\nNo pub packages were found in the pub cache when this file was generated, so no texts are included. Run `flutter pub get` and regenerate.\n'}${pubNoText.length && pg.groups.length ? `\nNo licence file found in the pub cache for: ${pubNoText.map((x) => `${x.name} ${x.version}`).join(', ')}.\n` : ''}`;
  return { text: s, missing: r.items.filter((i) => !i.files.length), pubNoText: pubNoText.length, total: { rust: r.items.length, npm: n.length, pub: p.length } };
}

// The copies of the two root files (native-apps.md 21.7). Pure functions of the root files.
const NOTICES = join(repo, 'THIRD_PARTY_NOTICES.md');
function copies(licencesText, noticesText) {
  return [
    ['apps/flutter/assets/licenses/THIRD_PARTY_LICENSES.md', licencesText],
    ['apps/flutter/assets/licenses/THIRD_PARTY_NOTICES.md', noticesText],
    ['apps/apple-tv/Resources/Licenses/THIRD_PARTY_LICENSES.md', licencesText],
    ['apps/apple-tv/Resources/Licenses/THIRD_PARTY_NOTICES.md', noticesText],
    ['apps/web/public/third-party-licenses.txt', `${noticesText.replace(/\n+$/, '')}\n\n${licencesText}`],
  ];
}
/** Names of the copies that differ from what the root files give (missing counts as different). */
function staleCopies(licencesText, noticesText) {
  return copies(licencesText, noticesText)
    .filter(([f, t]) => !existsSync(join(repo, f)) || readFileSync(join(repo, f), 'utf8') !== t)
    .map(([f]) => f);
}
function writeCopies(licencesText, noticesText) {
  for (const [f, t] of copies(licencesText, noticesText)) {
    mkdirSync(dirname(join(repo, f)), { recursive: true });
    writeFileSync(join(repo, f), t);
  }
}

if (process.argv.includes('--copies-only')) {
  // The root files as they are in the checkout.
  const l = readFileSync(OUT, 'utf8');
  const n = readFileSync(NOTICES, 'utf8');
  if (process.argv.includes('--check')) {
    const stale = staleCopies(l, n);
    if (stale.length) { console.error(`out of date: ${stale.join(', ')}: run node tools/gen-third-party.mjs --copies-only`); process.exit(1); }
    console.log('the licence copies are up to date');
  } else {
    writeCopies(l, n);
    console.log('wrote the licence copies');
  }
  process.exit(0);
}

const res = build();
if (process.argv.includes('--check')) {
  const cur = existsSync(OUT) ? readFileSync(OUT, 'utf8') : '';
  if (cur !== res.text) { console.error('THIRD_PARTY_LICENSES.md is out of date: run node tools/gen-third-party.mjs'); process.exit(1); }
  const stale = staleCopies(res.text, readFileSync(NOTICES, 'utf8'));
  if (stale.length) { console.error(`out of date: ${stale.join(', ')}: run node tools/gen-third-party.mjs`); process.exit(1); }
  console.log('THIRD_PARTY_LICENSES.md and its copies are up to date');
} else {
  writeFileSync(OUT, res.text);
  writeCopies(res.text, readFileSync(NOTICES, 'utf8'));
  console.log(`wrote THIRD_PARTY_LICENSES.md (${(res.text.length / 1024).toFixed(0)} KiB): ${res.total.rust} crates, ${res.total.npm} npm, ${res.total.pub} pub; ${res.missing.length} crates without licence file${res.missing.length ? ': ' + res.missing.map((m) => m.name).join(', ') : ''}; ${res.pubNoText} pub packages without text`);
}
