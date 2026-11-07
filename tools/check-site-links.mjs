#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
// Checks the static site in site/ without dependencies: every internal link, src, href, canonical
// and hreflang URL must point to an existing file (directory URLs need index.html), every #fragment
// must match an id on the target page, and every page needs lang, title, description and a hreflang pair.
// External links (https://...) are NOT fetched; the check is offline on purpose.
// The web app lives under /app/ only in the built artifact (the Pages workflow copies apps/web/dist there),
// so the committed site/ directory does not contain it: pass --without-app to check site/ alone (links to
// /app/ are then accepted); without the flag /app/ must exist, which is how the staged artifact is checked.
// Files under /app/ are not page-checked (they are the Vite build, not site pages).
// Usage: node tools/check-site-links.mjs [site-dir] [--without-app]
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const args = process.argv.slice(2);
const withoutApp = args.includes('--without-app');
const root = resolve(args.find((a) => !a.startsWith('--')) ?? join(dirname(fileURLToPath(import.meta.url)), '..', 'site'));
const ORIGIN = 'https://cometcast.de';
const ALLOWED_MISSING = new Set(withoutApp ? ['/app/'] : []); // see docs/website.md

const walk = (d) => readdirSync(d).flatMap((n) => {
  const f = join(d, n);
  return statSync(f).isDirectory() ? walk(f) : [f];
});
const files = walk(root);
const appDir = join(root, 'app') + '/';
const htmls = files.filter((f) => f.endsWith('.html') && !f.startsWith(appDir));
const errors = [];
const idsCache = new Map();

function resolveTarget(path) {
  let f = join(root, path);
  if (path.endsWith('/')) f = join(f, 'index.html');
  return existsSync(f) && statSync(f).isFile() ? f : null;
}
function ids(f) {
  if (!idsCache.has(f)) idsCache.set(f, new Set([...readFileSync(f, 'utf8').matchAll(/\sid="([^"]+)"/g)].map((m) => m[1])));
  return idsCache.get(f);
}

for (const file of htmls) {
  const rel = file.slice(root.length);
  const html = readFileSync(file, 'utf8');
  const err = (m) => errors.push(`${rel}: ${m}`);
  if (!/<html lang="[a-z]{2}"/.test(html)) err('missing html lang');
  if (!/<title>[^<]+<\/title>/.test(html)) err('missing title');
  if (!/<meta name="description" content="[^"]+"/.test(html) && !rel.endsWith('404.html')) err('missing description');
  if (!rel.endsWith('404.html') && !/rel="alternate" hreflang="de"/.test(html)) err('missing hreflang de');
  if (!rel.endsWith('404.html') && !/rel="alternate" hreflang="en"/.test(html)) err('missing hreflang en');
  const pageUrl = rel.replace(/index\.html$/, '');
  for (const m of html.matchAll(/\s(?:href|src|srcset)="([^"]*)"/g)) {
    let u = m[1];
    if (!u || /^(mailto:|tel:|data:)/.test(u)) continue;
    if (u.startsWith(ORIGIN)) u = u.slice(ORIGIN.length) || '/';
    if (/^https?:\/\//.test(u)) { if (!u.startsWith('https://')) err(`insecure link ${u}`); continue; }
    if (u.startsWith('//')) { err(`protocol-relative URL ${u}`); continue; }
    const [pathPart, frag] = u.split('#');
    const path = pathPart === '' ? pageUrl : pathPart;
    if (!path.startsWith('/')) { err(`relative URL ${u} (use root-relative)`); continue; }
    if (ALLOWED_MISSING.has(path)) continue;
    const target = resolveTarget(path);
    if (!target) { err(`broken link ${u}`); continue; }
    if (frag && target.endsWith('.html') && !ids(target).has(frag)) err(`missing anchor #${frag} in ${path}`);
  }
}
for (const need of ['404.html', 'CNAME', '.nojekyll', 'sitemap.xml', 'robots.txt']) {
  if (!existsSync(join(root, need))) errors.push(`missing ${need}`);
}
// Join links of the web app (/app/{CODE}, /app/display) reach the app through the 404 page of GitHub Pages (docs/website.md),
// and the app links of the apps are verified by files under /.well-known/ (native-apps.md 21.6).
if (existsSync(join(root, '404.html')) && !/fetch\('\/app\/index\.html'/.test(readFileSync(join(root, '404.html'), 'utf8'))) {
  errors.push('404.html does not hand paths under /app/ to the web app');
}
if (!withoutApp) {
  for (const f of ['.well-known/assetlinks.json', '.well-known/apple-app-site-association']) {
    const p = join(root, f);
    if (!existsSync(p)) { errors.push(`missing ${f}`); continue; }
    try { JSON.parse(readFileSync(p, 'utf8')); } catch { errors.push(`${f} is not JSON`); }
  }
}
if (existsSync(join(root, 'sitemap.xml'))) {
  for (const m of readFileSync(join(root, 'sitemap.xml'), 'utf8').matchAll(/<loc>([^<]+)<\/loc>/g)) {
    if (!m[1].startsWith(ORIGIN + '/')) errors.push(`sitemap: foreign URL ${m[1]}`);
    else if (!resolveTarget(m[1].slice(ORIGIN.length))) errors.push(`sitemap: missing page ${m[1]}`);
  }
}
if (existsSync(join(root, 'CNAME')) && readFileSync(join(root, 'CNAME'), 'utf8').trim() !== 'cometcast.de') errors.push('CNAME is not cometcast.de');
if (errors.length) { console.error(errors.join('\n')); console.error(`\n${errors.length} problem(s)`); process.exit(1); }
console.log(`site OK: ${htmls.length} pages checked`);
