#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
// Generates the static website in site/ (English at /, German at /de/) from the texts in tools/site/text.*.mjs,
// site-content/*.md (legal pages and self-hosting), the finder matrix (tools/site/finder.mjs) and the branding file.
// Run: node tools/build-site.mjs   (Node 22; the fonts need `npm ci --prefix tools/site`). The generated files are committed.
//   COMETCAST_BRANDING=...     branding file (branding/README.md), default branding/default.json
//   COMETCAST_PUBLIC_BUILD=1   fail while required imprint fields are empty (the guard "fill the imprint before enabling Pages")
//   --out DIR                  write there instead of site/ (used by the tests)
//   --offline | COMETCAST_OFFLINE=1   do not ask GitHub for the latest release (the download page then says "coming soon")
// site/assets/style.css and site/assets/site.js are written by hand; the build adds assets/fonts*, the pages and the plumbing.
import { mkdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { IMPRINT_REQUIRED, REPO_ROOT, loadBranding, missingImprint } from './branding.mjs';
import { LANGS, ORIGIN, PAGES, REDIRECTS, APK_PATH, SLUG, pageUrl, otherLang } from './site/pages.mjs';
import textEn from './site/text.en.mjs';
import textDe from './site/text.de.mjs';
import { renderFinder, ICON } from './site/finder.mjs';
import { bundleFonts, fontsInstalled } from './site/fonts.mjs';

const oi = process.argv.indexOf('--out');
const root = oi >= 0 ? resolve(process.argv[oi + 1]) : join(REPO_ROOT, 'site');
const OFFLINE = process.argv.includes('--offline') || process.env.COMETCAST_OFFLINE === '1';
const { file: brandingFile, branding } = loadBranding();
const PUBLIC_BUILD = process.env.COMETCAST_PUBLIC_BUILD === '1';
const missing = missingImprint(branding);
if (PUBLIC_BUILD && missing.length) {
  console.error(`build-site: PUBLIC BUILD REFUSED. The imprint is not filled in: ${missing.map((k) => `imprint.${k}`).join(', ')} (required: ${IMPRINT_REQUIRED.map((k) => `imprint.${k}`).join(', ')}).`);
  console.error(`Fill these in ${brandingFile} (see branding/README.md and docs/website.md), or select your own file with COMETCAST_BRANDING. Never publish the site without an imprint.`);
  process.exit(1);
}
const incomplete = missing.length > 0;
// The imprint page exists when the branding names a publisher or any imprint detail; the default has neither.
const hasImprint = Boolean(branding.publisher_name) || Object.values(branding.imprint).some(Boolean);
const GH = branding.source_url.replace(/\/$/, '');
const RELAY = 'https://github.com/wi-wissen/CometCast-Relay';
const pub = branding.publisher_name;
// The site's own domain as the display shows it (no port): the host of the legal pages in the branding file, else cometcast.de.
const DOMAIN = (() => {
  for (const u of [branding.privacy_url.en, branding.imprint_url.en, branding.privacy_url.de, branding.publisher_url]) {
    if (u) return new URL(u).hostname;
  }
  return new URL(ORIGIN).hostname;
})();
const esc = (v) => String(v).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const todo = (s) => `<span class="todo">TODO: ${s}</span>`;
const pages = PAGES.filter((p) => p !== 'imprint' || hasImprint);
const url = pageUrl;

// Where the apps are offered. An empty string means "not published yet": the download page then says "coming soon".
const STORES = { appStore: '', googlePlay: '', appleTv: '', googleTv: '' };

// ---- texts and the check that both languages have the same keys and the same HTML structure ------------------------
const ctx = { GH, DOMAIN, pub };
const T = { en: textEn(ctx), de: textDe(ctx) };
const dummy = { pub: 'X', contactHtml: '<b>x</b>' };
function flatten(v, path = '', out = new Map()) {
  if (typeof v === 'function') return flatten(v(dummy), path, out);
  if (typeof v === 'string') {
    if (!v.trim()) out.set(path, 'EMPTY');
    else out.set(path, [...v.matchAll(/<(\/?[a-z][a-z0-9]*)/gi)].map((m) => m[1].toLowerCase()).join(' '));
  } else if (Array.isArray(v)) v.forEach((x, i) => flatten(x, `${path}[${i}]`, out));
  else if (v && typeof v === 'object') for (const k of Object.keys(v)) flatten(v[k], path ? `${path}.${k}` : k, out);
  else out.set(path, String(v));
  return out;
}
{
  const a = flatten(T.en), b = flatten(T.de);
  const problems = [];
  for (const k of new Set([...a.keys(), ...b.keys()])) {
    if (!a.has(k)) problems.push(`text.en.mjs lacks "${k}"`);
    else if (!b.has(k)) problems.push(`text.de.mjs lacks "${k}"`);
    else if (a.get(k) === 'EMPTY' || b.get(k) === 'EMPTY') problems.push(`"${k}" is empty`);
    else if (a.get(k) !== b.get(k)) problems.push(`"${k}" has different HTML in en and de (${a.get(k).slice(0, 60)} | ${b.get(k).slice(0, 60)})`);
  }
  if (problems.length) {
    console.error(`build-site: the two languages are out of step:\n  ${problems.join('\n  ')}`);
    process.exit(1);
  }
}


// ---- the latest GitHub release (download page) ------------------------------------------------------------------
// The build asks GitHub once; the visitor's browser loads nothing from GitHub until they click a download.
async function latestRelease() {
  if (OFFLINE) return null;
  const m = /^https:\/\/github\.com\/([^/]+\/[^/]+)/.exec(GH);
  if (!m) return null;
  const headers = { accept: 'application/vnd.github+json', 'user-agent': 'cometcast-site-build' };
  if (process.env.GITHUB_TOKEN) headers.authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  try {
    const r = await fetch(`https://api.github.com/repos/${m[1]}/releases/latest`, { headers, signal: AbortSignal.timeout(10000) });
    if (r.status === 404) return null; // no published release yet
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
    return await r.json();
  } catch (e) {
    console.warn(`build-site: could not read the latest release (${e.message}); the download page says "coming soon"`);
    return null;
  }
}
/** Release assets by platform; names as made by .github/workflows/release.yml. Unsigned/unverified/debug-key files are left out. */
function platformAssets(release) {
  const out = { windows: [], macos: [], linux: [], android: [] };
  if (!release) return out;
  const files = (release.assets ?? [])
    .filter((a) => a.browser_download_url?.startsWith('https://') && !/debugkey|unverified|unsigned/i.test(a.name))
    .map((a) => ({ name: a.name, url: a.browser_download_url }));
  const pick = (re, label) => files.filter((f) => re.test(f.name)).map((f) => ({ ...f, label }));
  out.windows = [...pick(/^cometcast-windows-.*\.msi$/i, '.msi'), ...pick(/^cometcast-windows-.*\.exe$/i, '.exe'), ...pick(/^cometcast-windows-.*\.zip$/i, '.zip')].slice(0, 1);
  out.macos = [...pick(/^cometcast-macos-.*\.dmg$/i, '.dmg'), ...pick(/^cometcast-macos-.*\.zip$/i, '.zip')].slice(0, 1);
  out.linux = [...pick(/\.deb$/i, '.deb'), ...pick(/^cometcast-linux-.*\.(tar\.gz)$/i, '.tar.gz'), ...pick(/\.AppImage$/i, '.AppImage')];
  out.android = pick(/^cometcast-android-.*\.apk$/i, '.apk').slice(0, 1);
  return out;
}
const release = await latestRelease();
const assets = platformAssets(release);
const releaseVersion = release?.tag_name ? String(release.tag_name).replace(/^v/, '') : '';

// ---- pages from site-content/*.md ----------------------------------------------------------------
// ---- pages from site-content/*.md ----------------------------------------------------------------
// A small block format (no dependencies): front matter (title, description, optional title_incomplete /
// description_incomplete for the placeholder state), `# ` / `## ` headings, paragraphs (lines joined with <br>),
// `- ` lists, `> ` notes; inline HTML passes through. `{{GH}}` `{{RELAY}}` `{{publisher_name}}` `{{contact}}`
// `{{imprint.x|hint}}` (the value; empty: a TODO marker with the hint while the imprint is incomplete, else nothing)
// `{{todo:text}}`; `{{#if a|b}}` ... `{{/if}}` on lines of their own (true when one of the names is set; `incomplete`
// is true while required imprint fields are empty).
const lookup = (name) => (name === 'incomplete' ? incomplete : name.startsWith('imprint.') ? branding.imprint[name.slice(8)] : name === 'publisher_name' || name === 'contact' ? branding[name] : undefined);

function renderPage(name, lang) {
  const f = join(REPO_ROOT, 'site-content', `${name}.${lang}.md`);
  const m = /^---\n([\s\S]*?)\n---\n([\s\S]*)$/.exec(readFileSync(f, 'utf8'));
  if (!m) throw new Error(`${f}: front matter missing`);
  const fm = Object.fromEntries(m[1].split('\n').map((l) => [l.slice(0, l.indexOf(':')), l.slice(l.indexOf(':') + 1).trim()]));
  const lines = [];
  const stack = [];
  for (const raw of m[2].split('\n')) {
    const c = /^\{\{#if ([^}]+)\}\}$/.exec(raw.trim());
    if (c) { stack.push(c[1].split('|').some((n) => Boolean(lookup(n.trim())))); continue; }
    if (raw.trim() === '{{/if}}') { stack.pop(); continue; }
    if (stack.includes(false)) continue;
    let hadToken = false;
    const text = raw.replace(/\{\{([^}]+)\}\}/g, (_, t) => {
      hadToken = true;
      if (t === 'GH') return GH;
      if (t === 'RELAY') return RELAY;
      if (t.startsWith('todo:')) return todo(t.slice(5));
      const [key, hint] = t.split('|');
      const v = lookup(key.trim());
      if (v === undefined) throw new Error(`${f}: unknown placeholder {{${t}}}`);
      return v ? esc(v) : hint !== undefined && incomplete ? todo(hint) : '';
    });
    if (hadToken && !text.trim()) continue;
    lines.push(text);
  }
  const blocks = [];
  let cur = null;
  const flush = () => {
    if (!cur) return;
    if (cur.t === 'ul') blocks.push(`<ul>\n${cur.l.map((x) => `<li>${x}</li>`).join('\n')}\n</ul>`);
    else if (cur.t === 'note') blocks.push(`<div class="note"><p>${cur.l.join(' ')}</p></div>`);
    else blocks.push(`<p>${cur.l.join('<br>')}</p>`);
    cur = null;
  };
  for (const l of lines) {
    const h = /^(#{1,2}) (.*)$/.exec(l);
    if (!l.trim()) flush();
    else if (h) { flush(); blocks.push(`<h${h[1].length}>${h[2]}</h${h[1].length}>`); }
    else {
      const t = l.startsWith('- ') ? 'ul' : l.startsWith('> ') ? 'note' : 'p';
      if (!cur || cur.t !== t) { flush(); cur = { t, l: [] }; }
      cur.l.push(t === 'p' ? l : l.slice(2));
    }
  }
  flush();
  const pick = (k) => (incomplete && fm[`${k}_incomplete`]) || fm[k];
  return { title: pick('title'), desc: pick('description'), body: `\n${blocks.join('\n')}` };
}

const MD = { privacy: 'privacy', selfhost: 'self-hosting', imprint: 'imprint' };
const content = {};
for (const lang of LANGS) {
  content[lang] = {};
  for (const p of pages) {
    const t = T[lang];
    if (MD[p]) {
      content[lang][p] = renderPage(MD[p], lang);
    } else if (p === 'how' || p === 'open') {
      content[lang][p] = { title: t[p].title, desc: t[p].desc, body: t[p].body({ pub }) };
    } else if (p === 'contact') {
      const contactHtml = branding.contact ? esc(branding.contact) : '';
      content[lang][p] = { title: t.contact.title, desc: t.contact.desc, body: t.contact.body({ contactHtml }) };
    }
  }
}

// ---- pictures drawn at build time (no JavaScript needed) ----------------------------------------------------------
const lcg = (seed) => { let s = seed; return () => (s = (s * 16807) % 2147483647) / 2147483647; };
/** Faint stars behind the hero: a seeded scatter, drawn as SVG circles (colours from CSS tokens). */
function starsSvg() {
  const rr = lcg(5);
  const dots = [];
  for (let i = 0; i < 70; i++) {
    const op = (0.2 + rr() * 0.5).toFixed(2);
    const accent = rr() < 0.3;
    const x = (rr() * 100).toFixed(1), y = (rr() * 100).toFixed(1), r = (rr() * 1.4 + 0.3).toFixed(2);
    dots.push(`<circle class="${accent ? 'star a' : 'star'}" cx="${x}%" cy="${y}%" r="${r}" opacity="${op}"/>`);
  }
  return `<svg class="sky-bg" aria-hidden="true" focusable="false" width="100%" height="100%">${dots.join('')}</svg>`;
}
/** A QR-code look-alike for the display mock (finder patterns plus seeded modules). It does not encode anything. */
function qrSvg() {
  const n = 29, rnd = lcg(11);
  const g = Array.from({ length: n }, () => Array(n).fill(0));
  for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) {
    const inF = (x < 8 && y < 8) || (x > n - 9 && y < 8) || (x < 8 && y > n - 9);
    if (!inF && rnd() < 0.48) g[y][x] = 1;
  }
  for (const [fx, fy] of [[0, 0], [n - 7, 0], [0, n - 7]]) {
    for (let y = 0; y < 7; y++) for (let x = 0; x < 7; x++) g[fy + y][fx + x] = x === 0 || y === 0 || x === 6 || y === 6 || (x >= 2 && x <= 4 && y >= 2 && y <= 4) ? 1 : 0;
  }
  let d = '';
  for (let y = 0; y < n; y++) {
    for (let x = 0; x < n;) {
      if (!g[y][x]) { x++; continue; }
      let e = x;
      while (e < n && g[y][e]) e++;
      d += `M${x} ${y}h${e - x}v1h${x - e}z`;
      x = e;
    }
  }
  return `<svg viewBox="0 0 29 29" shape-rendering="crispEdges" aria-hidden="true"><path d="${d}"/></svg>`;
}
const I = {
  approval: '<circle cx="9" cy="8" r="4"/><path d="M2 21c0-4 3-6 7-6s7 2 7 6"/><path d="M16 11l2 2 4-4"/>',
  gear: '<path d="M10.3 2.6h3.4l.5 2.6a7.5 7.5 0 0 1 1.9 1.1l2.5-.9 1.7 2.9-2 1.7a7.6 7.6 0 0 1 0 2.2l2 1.7-1.7 2.9-2.5-.9a7.5 7.5 0 0 1-1.9 1.1l-.5 2.6h-3.4l-.5-2.6a7.5 7.5 0 0 1-1.9-1.1l-2.5.9-1.7-2.9 2-1.7a7.6 7.6 0 0 1 0-2.2l-2-1.7 1.7-2.9 2.5.9a7.5 7.5 0 0 1 1.9-1.1z"/><circle cx="12" cy="12" r="3"/>',
};
const icon = (name) => `<svg class="ic" viewBox="0 0 24 24" aria-hidden="true">${I[name]}</svg>`;

// ---- page shell ---------------------------------------------------------------------------------------------------
const logo = (alt, size) => `<picture><source srcset="/assets/cometcast-logo-dark.svg" media="(prefers-color-scheme: dark)"><img src="/assets/cometcast-logo-light.svg" alt="${esc(alt)}" width="${size}" height="${size}"></picture>`;

function layout(lang, p, c) {
  const t = T[lang];
  const o = otherLang(lang);
  const home = url(lang, 'home');
  const finder = `${home}#finder`;
  const self = url(lang, p);
  const ot = T[o];
  const alt = (l) => `<link rel="alternate" hreflang="${l}" href="${ORIGIN}${url(l, p)}">`;
  const noindex = p === 'imprint' ? '\n<meta name="robots" content="noindex">' : '';
  const cur = (k) => (k === p ? ' aria-current="page"' : '');
  const col = (title, links) => `<div class="col"><p class="coltitle">${esc(title)}</p><ul>${links.map(([h, label]) => `<li><a href="${h}">${esc(label)}</a></li>`).join('')}</ul></div>`;
  const body = p === 'home' ? c.body : `<div class="doc"><div class="wrap narrow${p === 'download' ? ' wide' : ''}">${c.body}\n</div></div>`;
  return `<!doctype html>
<html lang="${lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(c.title)}</title>
<meta name="description" content="${esc(c.desc)}">${noindex}
<link rel="canonical" href="${ORIGIN}${self}">
${alt('en')}
${alt('de')}
<link rel="alternate" hreflang="x-default" href="${ORIGIN}${url('en', p)}">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<meta name="color-scheme" content="light dark">
<meta name="theme-color" content="#f6f7fb" media="(prefers-color-scheme: light)">
<meta name="theme-color" content="#0b1126" media="(prefers-color-scheme: dark)">
<link rel="stylesheet" href="/assets/fonts.css">
<link rel="stylesheet" href="/assets/style.css">
<script src="/assets/site.js" defer></script>
</head>
<body class="${p}">
<a class="skip" href="#main">${esc(t.ui.skip)}</a>
<div class="langhint" lang="${o}" data-target="${o}" hidden><div class="wrap"><span>${esc(ot.hint.text)}</span><a href="${url(o, p)}" hreflang="${o}" data-hint-link>${esc(ot.hint.link)}</a><button type="button" aria-label="${esc(ot.hint.close)}" data-hint-close>×</button></div></div>
<header class="site-head"><div class="wrap">
<a class="brand" href="${home}">${logo(t.ui.brandAlt, 34)}<span>Comet Cast</span></a>
<nav class="nav" aria-label="${esc(t.ui.mainNav)}">
<div class="navlinks"><a href="${finder}">${esc(t.ui.whatYouNeed)}</a><a href="${url(lang, 'download')}"${cur('download')}>${esc(t.ui.download)}</a></div>
<a class="lang" href="${url(o, p)}" hreflang="${o}" lang="${o}" title="${esc(t.ui.switchLabel)}" aria-label="${esc(t.ui.switchLabel)}">${esc(t.ui.switchText)}</a>
<a class="btn btn-primary" href="/app/">${esc(t.ui.openApp)}</a>
</nav>
</div></header>
<main id="main">${body}
</main>
<footer class="foot"><div class="wrap">
<div class="cols">
${col(t.ui.footProduct, [['/app/', t.ui.footApp], [url(lang, 'download'), t.ui.download], [finder, t.ui.whatYouNeed]])}
${col(t.ui.footTrust, [[url(lang, 'privacy'), t.ui.footPrivacy], [url(lang, 'selfhost'), t.ui.footSelfhost], [url(lang, 'open'), t.ui.footOpen], [url(lang, 'how'), t.ui.footHow]])}
${col(t.ui.footProject, [[url(lang, 'contact'), t.ui.footContact], ...(hasImprint ? [[url(lang, 'imprint'), t.ui.footImprint]] : []), [GH, 'GitHub']])}
</div>
<p class="fine">${esc(t.ui.footFine)}</p>
</div></footer>
</body>
</html>
`;
}

// ---- home page ------------------------------------------------------------------------------------------------------
function homeBody(lang) {
  const t = T[lang];
  const h = t.home;
  return `
<section class="hero">${starsSvg()}
<div class="wrap hero-grid">
<div class="hero-text">
<h1>${h.h1}</h1>
<p class="lead">${esc(h.lead)}</p>
<div class="ctas"><a href="${url(lang, 'download')}" class="btn btn-primary">${esc(h.ctaDownload)}</a><a href="/app/" class="btn btn-ghost">${esc(h.ctaApp)}</a></div>
<p class="small"><span class="beta">${esc(h.beta)}</span><span>${esc(h.tryIt)}</span></p>
</div>
<div class="tv" role="img" aria-label="${esc(h.tv.label)}">
<div class="top"><img class="tv-logo" src="/assets/cometcast-logo-dark.svg" alt="" width="40" height="40"><span class="room">${esc(h.tv.room)}</span><span class="pill">${icon('approval')}${esc(h.tv.pill)}</span></div>
<div class="steps">
<div class="st"><span class="num">1</span><div><div class="lbl">${esc(h.tv.step1)}</div><div class="big">${esc(DOMAIN)}</div></div></div>
<div class="st"><span class="num">2</span><div><div class="lbl">${esc(h.tv.step2)}</div><div class="code">DXZ PNJ</div></div></div>
</div>
<div class="qr">${qrSvg()}</div>
<div class="scan">${esc(h.tv.scan)}</div>
<div class="tvfoot">${esc(h.tv.foot)}</div>
<div class="gear">${icon('gear')}${esc(h.tv.settings)}</div>
</div>
</div>
</section>

<section class="section finder-section" id="finder"><div class="wrap">
<h2>${esc(h.finderH2)}</h2>
<p class="intro">${esc(h.finderIntro)}</p>
${renderFinder(lang, t.finder, DOMAIN)}
</div></section>

<section class="section steps-section"><div class="wrap">
<h2>${esc(h.stepsH2)}</h2>
<ol class="steps3">${h.steps.map((s) => `<li class="step"><h3>${esc(s.h)}</h3><p>${esc(s.p)}</p></li>`).join('')}</ol>
</div></section>`;
}

// ---- download page --------------------------------------------------------------------------------------------------
function downloadBody(lang) {
  const t = T[lang];
  const d = t.download;
  const card = (id, ic, links, store) => {
    const c = d.cards[id];
    const yours = d.yours[id] ? ` data-yours="${esc(d.yours[id])}"` : '';
    let actions = '';
    if (id === 'web') actions = `<a class="btn btn-primary" href="/app/">${esc(c.btn)}</a>`;
    else if (links.length) {
      actions = `<a class="btn btn-primary" href="${esc(links[0].url)}">${esc(c.btn)}</a><span class="dl-meta">${links.map((l, i) => (i === 0 ? esc(l.label) : `<a href="${esc(l.url)}">${esc(l.label)}</a>`)).join(' · ')}</span>`;
    }
    if (store) actions += `<a class="btn ${links.length ? 'btn-ghost' : 'btn-primary'}" href="${esc(store)}">${esc(c.store)}</a>`;
    if (!actions) actions = `<span class="soon">${esc(d.soon)}</span>`;
    return `<article class="dl" id="${id}" data-platform="${id}"${yours}><div class="dl-ic"><svg viewBox="0 0 56 44" aria-hidden="true">${ICON[ic]}</svg></div><div class="dl-main"><h3>${esc(c.h)}</h3><p>${esc(c.p)}</p><div class="dl-actions">${actions}</div></div></article>`;
  };
  return `
<span class="eyebrow">${esc(d.eyebrow)}</span>
<h1>${esc(d.h1)}</h1>
<p class="lead">${esc(d.lead)}</p>
<p class="small"><span class="beta">${esc(d.beta)}</span><span>${esc(d.tryIt)}</span>${releaseVersion ? `<span class="ver">${esc(d.version.replace('{v}', releaseVersion))}</span>` : ''}</p>
<div class="dls" id="downloads">
${card('web', 'desktop', [], '')}
${card('windows', 'laptop', assets.windows, '')}
${card('macos', 'laptop', assets.macos, '')}
${card('linux', 'laptop', assets.linux, '')}
${card('android', 'android', assets.android, STORES.googlePlay)}
${card('ios', 'iphone', [], STORES.appStore)}
${card('appletv', 'appletv', [], STORES.appleTv)}
${card('googletv', 'googletv', [], STORES.googleTv)}
</div>
<p class="after">${esc(d.notSure)} <a href="${url(lang, 'home')}#finder">${esc(d.notSureLink)}</a>${release ? ` · <a href="${GH}/releases">${esc(d.allReleases)}</a>` : ''}</p>`;
}

for (const lang of LANGS) {
  content[lang].home = { title: T[lang].home.title, desc: T[lang].home.desc, body: homeBody(lang) };
  content[lang].download = { title: T[lang].download.title, desc: T[lang].download.desc, body: downloadBody(lang) };
}

// ---- output ---------------------------------------------------------------------------------------------------------
function write(rel, text) {
  const f = join(root, rel);
  mkdirSync(dirname(f), { recursive: true });
  writeFileSync(f, text);
}
const fileFor = (path) => path.slice(1) + 'index.html';

for (const lang of LANGS) for (const p of pages) write(fileFor(url(lang, p)), layout(lang, p, content[lang][p]));

/** A small page that sends the browser on: meta refresh, canonical (for our own pages) and a link. */
function redirectPage(lang, to) {
  const t = T[lang].redirect;
  const internal = to.startsWith('/');
  const abs = internal ? ORIGIN + to : to;
  return `<!doctype html>
<html lang="${lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(t.title)}</title>
<meta name="robots" content="noindex">${internal ? `\n<link rel="canonical" href="${esc(abs)}">` : ''}
<meta http-equiv="refresh" content="0; url=${esc(abs)}">
<link rel="stylesheet" href="/assets/fonts.css">
<link rel="stylesheet" href="/assets/style.css">
</head>
<body class="redirect">
<main id="main"><div class="wrap narrow doc"><p class="lead">${esc(t.p)} <a href="${esc(internal ? to : abs)}">${esc(internal ? ORIGIN.replace('https://', '') + to : abs)}</a></p></div></main>
</body>
</html>
`;
}
for (const r of REDIRECTS) {
  if (r.optional && !pages.includes(r.to)) continue;
  write(fileFor(r.from), redirectPage(r.lang, url(r.lang, r.to)));
}
// /apk: the latest Android file straight away, else the Android card of the download page
write(fileFor(APK_PATH), redirectPage('en', assets.android[0]?.url ?? `${url('en', 'download')}#android`));

// 404 (both languages, root-relative links)
write('404.html', `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(T.en.notFound.title)} / ${esc(T.de.notFound.title)}</title>
<meta name="robots" content="noindex">
<meta name="color-scheme" content="light dark">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="stylesheet" href="/assets/fonts.css">
<link rel="stylesheet" href="/assets/style.css">
<script>
// GitHub Pages answers every unknown path with this page. Under /app/ that means a join link (/app/K7QM2W) or
// /app/display of the web app, which chooses its page by the path: show the app's index.html here, the address stays.
// Every other path keeps this 404 page. (docs/website.md)
(function () {
  if (!/^\\/app(\\/|$)/.test(location.pathname)) return;
  document.documentElement.style.visibility = 'hidden';
  fetch('/app/index.html', { credentials: 'omit' })
    .then(function (r) { if (!r.ok) throw new Error('no app'); return r.text(); })
    .then(function (html) { document.open(); document.write(html); document.close(); })
    .catch(function () { document.documentElement.style.visibility = ''; });
})();
</script>
</head>
<body class="notfound">
<main id="main"><div class="wrap narrow doc">
<a class="brand" href="/">${logo(T.en.ui.brandAlt, 40)}<span>Comet Cast</span></a>
<h1>${T.en.notFound.h1}</h1>
<p class="lead" lang="en">${esc(T.en.notFound.p)} <a href="/">${esc(T.en.notFound.link)}</a>.</p>
<p class="lead" lang="de">${esc(T.de.notFound.p)} <a href="/de/">${esc(T.de.notFound.link)}</a>.</p>
</div></main>
</body>
</html>
`);

// sitemap with hreflang alternates, robots
const pagesForMap = pages.filter((p) => p !== 'imprint');
const entry = (lang, p) =>
  `<url><loc>${ORIGIN}${url(lang, p)}</loc>` +
  [...LANGS, 'x-default'].map((l) => `<xhtml:link rel="alternate" hreflang="${l}" href="${ORIGIN}${url(l === 'x-default' ? 'en' : l, p)}"/>`).join('') +
  `</url>`;
write('sitemap.xml', `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
${LANGS.flatMap((l) => pagesForMap.map((p) => entry(l, p))).join('\n')}
</urlset>
`);
write('robots.txt', `User-agent: *\nAllow: /\n\nSitemap: ${ORIGIN}/sitemap.xml\n`);
write('CNAME', 'cometcast.de\n');
write('.nojekyll', '');

// fonts: copied from the npm packages when installed; otherwise the committed copies stay (an --out build has none, the tests copy site/assets)
if (fontsInstalled()) console.log(`fonts: ${bundleFonts(join(root, 'assets'))} woff2 files copied from @fontsource`);
else if (!existsSync(join(root, 'assets', 'fonts'))) console.warn('fonts: the @fontsource packages are not installed (npm ci --prefix tools/site); keeping the committed site/assets/fonts');
console.log(`wrote ${pages.length * LANGS.length} pages + ${REDIRECTS.length + 1} redirects + 404 under ${root === join(REPO_ROOT, 'site') ? 'site/' : root}${release ? ` (release ${releaseVersion})` : ''}`);
