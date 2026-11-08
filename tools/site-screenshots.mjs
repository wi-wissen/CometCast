#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
// Screenshots of the website for review: home, how it works and download, English and German, light and dark,
// 1440 and 390 px wide, into a directory. Uses Playwright from apps/web (npm ci there; `chromium.launch()` needs no options).
// Usage: node tools/site-screenshots.mjs OUT_DIR [--base http://127.0.0.1:8000]
//   Without --base the script serves site/ itself on a free local port.
// `page.screenshot({ fullPage: true })` fails in some environments, so the viewport is set to the page's scrollHeight instead
// (at most 6000 px; a longer page is cut off and says so). Also reports horizontal overflow at 390 px.
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { existsSync, mkdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, extname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = join(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const out = resolve(args.find((a, i) => !a.startsWith('--') && args[i - 1] !== '--base') ?? 'site-shots');
const baseArg = args.includes('--base') ? args[args.indexOf('--base') + 1] : '';
const { chromium } = createRequire(join(repo, 'apps/web/package.json'))('playwright');
mkdirSync(out, { recursive: true });

const MIME = { '.html': 'text/html; charset=utf-8', '.css': 'text/css', '.js': 'text/javascript', '.svg': 'image/svg+xml', '.png': 'image/png', '.woff2': 'font/woff2', '.xml': 'application/xml', '.txt': 'text/plain' };
function serve() {
  const root = join(repo, 'site');
  const srv = createServer((req, res) => {
    let p = decodeURIComponent(new URL(req.url, 'http://x').pathname);
    let f = join(root, p);
    if (!f.startsWith(root)) { res.writeHead(403).end(); return; }
    if (existsSync(f) && statSync(f).isDirectory()) f = join(f, 'index.html');
    if (!existsSync(f)) { res.writeHead(404).end('not found'); return; }
    res.writeHead(200, { 'content-type': MIME[extname(f)] ?? 'application/octet-stream' }).end(readFileSync(f));
  });
  return new Promise((ok) => srv.listen(0, '127.0.0.1', () => ok({ srv, base: `http://127.0.0.1:${srv.address().port}` })));
}

const PAGES = [
  ['home', { en: '/', de: '/de/' }],
  ['how', { en: '/how-it-works/', de: '/de/so-funktionierts/' }],
  ['download', { en: '/download/', de: '/de/download/' }],
];
const WIDTHS = [1440, 390];
const MAX_H = 6000;

async function shot(ctx, name, path, width, label, lang, scheme) {
  const page = await ctx.newPage();
  await page.goto(base + path, { waitUntil: 'load' });
  await page.evaluate(() => document.fonts.ready);
  const h = await page.evaluate(() => document.documentElement.scrollHeight);
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  if (overflow > 0) { problems++; console.error(`horizontal overflow of ${overflow}px: ${label} ${lang} ${scheme} ${width}`); }
  await page.setViewportSize({ width, height: Math.min(h, MAX_H) });
  if (h > MAX_H) console.warn(`cut at ${MAX_H}px (page is ${h}px): ${label} ${lang} ${scheme} ${width}`);
  const file = join(out, `${name}.png`);
  await page.screenshot({ path: file });
  console.log(file);
  await page.close();
}

const server = baseArg ? null : await serve();
const base = (baseArg || server.base).replace(/\/$/, '');
const browser = await chromium.launch();
let problems = 0;
try {
  for (const scheme of ['light', 'dark']) {
    for (const width of WIDTHS) {
      // a fresh context per combination: colour scheme and a browser language that matches the page (so no language hint) ...
      for (const lang of ['en', 'de']) {
        const ctx = await browser.newContext({ colorScheme: scheme, viewport: { width, height: 900 }, deviceScaleFactor: 1, locale: lang === 'de' ? 'de-DE' : 'en-US' });
        for (const [name, paths] of PAGES) await shot(ctx, `${name}-${lang}-${scheme}-${width}`, paths[lang], width, name, lang, scheme);
        await ctx.close();
      }
      // ... and the English home page in a German browser: the language hint is shown
      const hintCtx = await browser.newContext({ colorScheme: scheme, viewport: { width, height: 900 }, deviceScaleFactor: 1, locale: 'de-DE' });
      await shot(hintCtx, `home-en-hint-${scheme}-${width}`, '/', width, 'home', 'en', scheme);
      await hintCtx.close();
    }
  }
} finally {
  await browser.close();
  server?.srv.close();
}
if (problems) process.exit(1);
