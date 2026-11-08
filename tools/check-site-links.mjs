#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
// Checks the static site in site/ without dependencies: every internal link, src, href, canonical
// and hreflang URL must point to an existing file (directory URLs need index.html), every #fragment
// must match an id on the target page, and every page needs lang, title, description and a hreflang pair.
// Also checked: hreflang pairs (en, de, x-default; the counterpart exists and points back), that every page exists in both
// languages, the redirect pages (old paths and short links: meta refresh, canonical and link agree and lead somewhere),
// that nothing is loaded from third parties (no external script, stylesheet, font or image) and the font files of fonts.css.
// External links (https://...) are NOT fetched; the check is offline on purpose.
// The web app lives under /app/ only in the built artifact (the Pages workflow copies apps/web/dist there),
// so the committed site/ directory does not contain it: pass --without-app to check site/ alone (links to
// /app/ are then accepted); without the flag /app/ must exist, which is how the staged artifact is checked.
// Files under /app/ are not page-checked (they are the Vite build, not site pages).
// Usage: node tools/check-site-links.mjs [site-dir] [--without-app]
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { APK_PATH, LANGS, ORIGIN as SITE_ORIGIN, PAGES, REDIRECTS, pageUrl } from './site/pages.mjs';

const args = process.argv.slice(2);
const withoutApp = args.includes('--without-app');
const root = resolve(args.find((a) => !a.startsWith('--')) ?? join(dirname(fileURLToPath(import.meta.url)), '..', 'site'));
const ORIGIN = SITE_ORIGIN;
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
  const isRedirect = /http-equiv="refresh"/.test(html);
  const plain = rel.endsWith('404.html') || isRedirect;
  if (!/<meta name="description" content="[^"]+"/.test(html) && !plain) err('missing description');
  if (!plain) for (const l of ['en', 'de', 'x-default']) if (!new RegExp(`rel="alternate" hreflang="${l}" href="${ORIGIN}/[^"]*"`).test(html)) err(`missing hreflang ${l}`);
  const thisUrl = rel.replace(/index\.html$/, '');
  // nothing from third parties: scripts, stylesheets, images, fonts and frames stay on this site (links to other sites are fine)
  for (const m of html.matchAll(/<(script|link|img|iframe|source|video|audio)\b[^>]*>/gi)) {
    const tag = m[0];
    const ref = /\s(?:src|href|srcset)="(https?:)?\/\/[^"]*"/i.exec(tag);
    if (ref && !(/^<link/i.test(tag) && /rel="(alternate|canonical)"/i.test(tag))) err(`third-party load: ${tag.slice(0, 90)}`);
  }
  if (/<script>[^<]*(fetch|XMLHttpRequest|sendBeacon)/i.test(html) && !rel.endsWith('404.html')) err('inline script makes requests');
  for (const m of html.matchAll(/\s(?:href|src|srcset)="([^"]*)"/g)) {
    let u = m[1];
    if (!u || /^(mailto:|tel:|data:)/.test(u)) continue;
    if (u.startsWith(ORIGIN)) u = u.slice(ORIGIN.length) || '/';
    if (/^https?:\/\//.test(u)) { if (!u.startsWith('https://')) err(`insecure link ${u}`); continue; }
    if (u.startsWith('//')) { err(`protocol-relative URL ${u}`); continue; }
    const [pathPart, frag] = u.split('#');
    const path = pathPart === '' ? thisUrl : pathPart;
    if (!path.startsWith('/')) { err(`relative URL ${u} (use root-relative)`); continue; }
    if (ALLOWED_MISSING.has(path)) continue;
    const target = resolveTarget(path);
    if (!target) { err(`broken link ${u}`); continue; }
    if (frag && target.endsWith('.html') && !ids(target).has(frag)) err(`missing anchor #${frag} in ${path}`);
  }
}

// pages exist in both languages and the hreflang links point at each other
const alternates = (f) => Object.fromEntries([...readFileSync(f, 'utf8').matchAll(/<link rel="alternate" hreflang="([^"]+)" href="([^"]+)"/g)].map((m) => [m[1], m[2].startsWith(ORIGIN) ? m[2].slice(ORIGIN.length) : m[2]]));
const pageKeys = PAGES.filter((p) => p !== 'imprint' || LANGS.some((l) => resolveTarget(pageUrl(l, p))));
for (const p of pageKeys) {
  for (const l of LANGS) {
    const u = pageUrl(l, p);
    const f = resolveTarget(u);
    if (!f) { errors.push(`page missing in ${l}: ${u}`); continue; }
    const alt = alternates(f);
    for (const k of LANGS) if (alt[k] !== pageUrl(k, p)) errors.push(`${u}: hreflang ${k} is ${alt[k]}, expected ${pageUrl(k, p)}`);
    if (alt['x-default'] !== pageUrl('en', p)) errors.push(`${u}: hreflang x-default should be ${pageUrl('en', p)}`);
    const canon = /rel="canonical" href="([^"]+)"/.exec(readFileSync(f, 'utf8'));
    if (!canon || canon[1] !== ORIGIN + u) errors.push(`${u}: canonical should be ${ORIGIN}${u}`);
  }
}
// redirects: old paths and short links
const redirectTarget = (f) => {
  const html = readFileSync(f, 'utf8');
  const meta = /http-equiv="refresh" content="0; url=([^"]+)"/.exec(html);
  if (!meta) return null;
  const target = meta[1].replace(/&amp;/g, '&');
  const canon = /rel="canonical" href="([^"]+)"/.exec(html);
  const link = [...html.matchAll(/<a href="([^"]+)"/g)].map((m) => m[1]);
  return { target, canon: canon?.[1], link };
};
const wanted = [...REDIRECTS.filter((r) => !r.optional || resolveTarget(pageUrl(r.lang, r.to))).map((r) => ({ from: r.from, to: ORIGIN + pageUrl(r.lang, r.to) })), { from: APK_PATH, to: null }];
for (const w of wanted) {
  const f = resolveTarget(w.from);
  const info = f && redirectTarget(f);
  if (!info) { errors.push(`redirect missing: ${w.from}`); continue; }
  if (w.to && info.target !== w.to) errors.push(`redirect ${w.from} goes to ${info.target}, expected ${w.to}`);
  if (info.target.startsWith(ORIGIN)) {
    if (info.canon !== info.target) errors.push(`redirect ${w.from}: canonical differs from the target`);
    if (!resolveTarget(info.target.slice(ORIGIN.length).split('#')[0])) errors.push(`redirect ${w.from}: target page missing`);
    else {
      const [tp, fr] = info.target.slice(ORIGIN.length).split('#');
      if (fr && !ids(resolveTarget(tp)).has(fr)) errors.push(`redirect ${w.from}: missing anchor #${fr}`);
    }
  } else if (!info.target.startsWith('https://')) errors.push(`redirect ${w.from}: not https`);
  if (!info.link.some((l) => l === info.target || l === info.target.slice(ORIGIN.length) || info.target.startsWith(ORIGIN) && info.target.slice(ORIGIN.length) === l)) errors.push(`redirect ${w.from}: no link to the target`);
}
// the font files named in fonts.css
if (existsSync(join(root, 'assets', 'fonts.css'))) {
  const css = readFileSync(join(root, 'assets', 'fonts.css'), 'utf8');
  for (const m of css.matchAll(/url\(([^)]+)\)/g)) if (!resolveTarget(m[1].replace(/^['"]|['"]$/g, ''))) errors.push(`fonts.css: missing file ${m[1]}`);
  if (/https?:\/\//.test(css.replace(/\/\*[\s\S]*?\*\//g, ''))) errors.push('fonts.css loads from another host');
} else errors.push('missing assets/fonts.css');
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
