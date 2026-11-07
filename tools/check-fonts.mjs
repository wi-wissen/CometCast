#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
// Checks crates/cc-office/fonts: every .ttf.gz inflates to the TrueType file whose SHA-256 is recorded in
// FONTS.md, the .gz files match their recorded SHA-256, and the licence files exist.
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gunzipSync } from 'node:zlib';

const dir = resolve(dirname(fileURLToPath(import.meta.url)), '..', 'crates/cc-office/fonts');
const sha = (b) => createHash('sha256').update(b).digest('hex');
const md = readFileSync(join(dir, 'FONTS.md'), 'utf8');
const rec = new Map([...md.matchAll(/^\| `([^`]+\.ttf)` \| `([0-9a-f]{64})` \| `([0-9a-f]{64})` \|$/gm)].map((m) => [m[1], [m[2], m[3]]]));
const errors = [];
const gz = readdirSync(dir).filter((f) => f.endsWith('.ttf.gz'));
if (gz.length !== 20 || rec.size !== 20) errors.push(`expected 20 fonts, found ${gz.length} files and ${rec.size} table rows`);
for (const f of gz) {
  const name = f.slice(0, -3);
  const r = rec.get(name);
  if (!r) { errors.push(`${name} is not in FONTS.md`); continue; }
  const raw = readFileSync(join(dir, f));
  if (sha(raw) !== r[1]) errors.push(`${f}: gz checksum differs`);
  if (sha(gunzipSync(raw)) !== r[0]) errors.push(`${name}: TrueType checksum differs`);
}
for (const l of ['LICENSE-Liberation.txt', 'OFL-Carlito.txt', 'OFL-Caladea.txt']) if (!existsSync(join(dir, l))) errors.push(`missing ${l}`);
if (errors.length) { console.error(errors.join('\n')); process.exit(1); }
console.log('fonts OK: 20 files match FONTS.md');
