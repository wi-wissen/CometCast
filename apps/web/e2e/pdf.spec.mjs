// SPDX-License-Identifier: GPL-3.0-or-later
//
// End-to-end test for PDF documents in the web client: a web sender (content-only mode) sends a small
// generated PDF over the WebRTC DataChannel `control` to a web receiver, which renders pages with the WASM
// build of cc-docs/hayro in a Web Worker.
// Checks: the page canvas is not blank and shows what the PDF draws (a black and a red box at known places),
// the page label, next/prev with the sender's buttons (cmd messages), clamping at both ends (checked by
// ordered command pairs, no sleeps), page 3 content (blank), that a real click on the link annotation's area
// does NOT follow it (no new tab, no navigation), that a damaged PDF gives the translated
// notice, the German texts, and stop.
// Starts the real cometcast-server and a real Chromium (or Chrome: set E2E_CHROME=/path/to/chrome).
// Run: npm run build:wasm && npm run build && npm run e2e:pdf
//   (needs the wasm target and a wasm-bindgen CLI matching Cargo.lock, see scripts/build-wasm.sh)
import { createRequire } from 'node:module'
import { spawn, spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'
import { existsSync } from 'node:fs'
import { createServer } from 'node:net'
import assert from 'node:assert/strict'
import { contentSender, openDisplay, presenterJoin } from './helpers.mjs'
import { makePdf } from './pdfFixture.mjs'

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')

const here = dirname(fileURLToPath(import.meta.url))
const repo = resolve(here, '../../..')
const webDir = resolve(here, '../dist')
assert.ok(
  existsSync(resolve(webDir, 'pdf/cc_docs_wasm_bg.wasm')),
  'dist/pdf/cc_docs_wasm_bg.wasm is missing: run `npm run build:wasm && npm run build` first',
)
const freePort = () =>
  new Promise((res, rej) => {
    const srv = createServer()
    srv.once('error', rej)
    srv.listen(0, '127.0.0.1', () => {
      const { port } = srv.address()
      srv.close(() => res(port))
    })
  })
const port = process.env.E2E_PORT ? Number(process.env.E2E_PORT) : await freePort()
const base = `http://127.0.0.1:${port}`

const build = spawnSync('cargo', ['build', '-p', 'cometcast-server'], { cwd: repo, stdio: 'inherit' })
assert.equal(build.status, 0, 'cargo build failed')
const targetDir = process.env.CARGO_TARGET_DIR ?? resolve(repo, 'target')
const server = spawn(resolve(targetDir, 'debug/cometcast-server'), ['--listen', `127.0.0.1:${port}`, '--web-dir', webDir], {
  stdio: 'inherit',
})

async function waitHealthy() {
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(`${base}/healthz`)).ok) return
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 100))
  }
  throw new Error('server did not become healthy')
}

async function waitFor(fn, what, ms = 20000) {
  const end = Date.now() + ms
  while (Date.now() < end) {
    try {
      const v = await fn()
      if (v) return v
    } catch {
      /* retry */
    }
    await new Promise((r) => setTimeout(r, 150))
  }
  throw new Error(`timeout waiting for: ${what}`)
}

/** Reads the page canvas: size, number of non-white pixels, and the colour at two relative points. */
const readCanvas = (page) =>
  page.getByTestId('document-canvas').evaluate((c) => {
    const ctx = c.getContext('2d')
    const { width, height } = c
    const d = ctx.getImageData(0, 0, width, height).data
    let nonWhite = 0
    for (let i = 0; i < d.length; i += 4) if (d[i] < 250 || d[i + 1] < 250 || d[i + 2] < 250) nonWhite++
    const at = (fx, fy) => {
      const i = (Math.floor(fy * height) * width + Math.floor(fx * width)) * 4
      return [d[i], d[i + 1], d[i + 2]]
    }
    return { width, height, nonWhite, topLeftBox: at(60 / 300, 1 - 140 / 200), bottomRight: at(140 / 200, 1 - 60 / 300) }
  })

/** Waits until the receiver shows exactly "Page n of 3" (exact text, not a substring). */
const pageIs = (page, n, lang = 'en') =>
  waitFor(
    async () => (await page.getByTestId('document-page').innerText()).trim() === `${lang === 'en' ? 'Page' : 'Seite'} ${n} ${lang === 'en' ? 'of' : 'von'} 3`,
    `receiver shows page ${n}`,
    10000,
  )

/**
 * Screen point of a spot of the PDF page drawn in the (object-contain) canvas: `fx`, `fy` are fractions of
 * the page, from its top left corner.
 */
const pagePoint = (page, fx, fy) =>
  page.getByTestId('document-canvas').evaluate(
    (c, [x, y]) => {
      const r = c.getBoundingClientRect()
      const scale = Math.min(r.width / c.width, r.height / c.height)
      const w = c.width * scale
      const h = c.height * scale
      return { x: r.left + (r.width - w) / 2 + x * w, y: r.top + (r.height - h) / 2 + y * h }
    },
    [fx, fy],
  )

let browser
try {
  await waitHealthy()
  browser = await chromium.launch({
    executablePath: process.env.E2E_CHROME || undefined,
    args: ['--disable-features=WebRtcHideLocalIpsWithMdns'],
  })
  const errors = []
  const newPage = async (locale = 'en-US') => {
    const ctx = await browser.newContext({ locale, viewport: { width: 1280, height: 800 } })
    const p = await ctx.newPage()
    p.on('pageerror', (e) => errors.push(e.message))
    return { ctx, page: p }
  }

  // ---------- English: receiver and sender ----------
  const { ctx: rctx, page: receiver } = await newPage()
  const code = await openDisplay(receiver, base)

  const { ctx: sctx, page: sender } = await newPage()
  await presenterJoin(sender, base, code)
  const content = contentSender(sender)

  // Anything the PDF's link would do must not happen: watch for new tabs and navigations.
  const opened = []
  rctx.on('page', (p) => opened.push(p.url()))
  receiver.on('popup', (p) => opened.push(`popup:${p.url()}`))
  let navigations = 0
  receiver.on('framenavigated', () => navigations++)
  const receiverUrl = receiver.url()

  const pdf = makePdf()
  await content.file({ name: 'lesson.pdf', mimeType: 'application/pdf', buffer: pdf })
  await sender.getByTestId('presenting-page').waitFor({ timeout: 20000 })

  await receiver.getByTestId('document-canvas').waitFor({ state: 'visible', timeout: 30000 })
  await waitFor(() => receiver.getByTestId('document-canvas').getAttribute('data-busy').then((b) => b === 'false'), 'first page drawn', 30000)
  assert.equal((await receiver.getByTestId('document-page').innerText()).trim(), 'Page 1 of 3')
  const p1 = await waitFor(async () => {
    const r = await readCanvas(receiver)
    return r.nonWhite > 0 ? r : null
  }, 'page 1 canvas is not blank')
  console.log('page 1 canvas:', JSON.stringify(p1))
  assert.ok(p1.width > p1.height, 'page 1 is landscape')
  assert.ok(p1.nonWhite > p1.width * p1.height * 0.05, 'the black box covers at least 5 % of the page')
  assert.deepEqual(p1.topLeftBox.map((v) => v < 30), [true, true, true], `black box expected, got ${p1.topLeftBox}`)

  // Really click where the link annotation sits (the black box, /Rect [20 100 100 180] on a 300 x 200 page):
  // a viewer that followed links would open a tab or navigate now.
  const linkSpot = await pagePoint(receiver, 60 / 300, 1 - 140 / 200)
  const navBefore = navigations
  await receiver.mouse.click(linkSpot.x, linkSpot.y)
  await receiver.mouse.dblclick(linkSpot.x, linkSpot.y)
  await new Promise((r) => setTimeout(r, 500)) // absence cannot be awaited: give a navigation time to start
  assert.deepEqual(opened, [], 'clicking the link area opened no tab')
  assert.equal(receiver.url(), receiverUrl, 'clicking the link area did not navigate')
  assert.equal(navigations, navBefore, 'no navigation event after the click')
  await pageIs(receiver, 1)

  // Next page (sender button, a `cmd` message): red box, portrait page.
  await sender.getByTestId('send-next-page').click()
  await receiver.getByText('Page 2 of 3').waitFor({ timeout: 10000 })
  const p2 = await waitFor(async () => {
    const r = await readCanvas(receiver)
    return r.height > r.width && r.nonWhite > 0 ? r : null
  }, 'page 2 canvas drawn')
  assert.ok(p2.bottomRight[0] > 200 && p2.bottomRight[1] < 60 && p2.bottomRight[2] < 60, `red box expected, got ${p2.bottomRight}`)

  await sender.getByTestId('send-next-page').click()
  await pageIs(receiver, 3)
  // Page 3 is empty (white) and landscape; a stale picture of page 2 (portrait) would not satisfy this.
  await waitFor(() => receiver.getByTestId('document-canvas').getAttribute('data-busy').then((b) => b === 'false'), 'page 3 drawn', 30000)
  const p3 = await waitFor(async () => {
    const r = await readCanvas(receiver)
    return r.width > r.height && r.nonWhite === 0 ? r : null
  }, 'page 3 canvas is blank and landscape')
  console.log('page 3 canvas:', JSON.stringify(p3))

  // Clamping without sleeps: commands on the sender's channel arrive in order, so "next" (clamped at 3)
  // followed by "prev" must end on exactly page 2; without the clamp it would be page 3 (4 - 1).
  await sender.getByTestId('send-next-page').click()
  await sender.getByTestId('send-prev-page').click()
  await pageIs(receiver, 2)
  // The display has no page buttons any more (pages are turned on the presenter's page).
  assert.equal(await receiver.getByTestId('doc-prev').count(), 0)
  assert.equal(await receiver.getByTestId('content-close').count(), 0)
  assert.equal(await receiver.locator('[data-testid=stage] button:not([data-tb])').count(), 0)
  await sender.getByTestId('send-prev-page').click()
  await pageIs(receiver, 1)
  // The same at the first page: "prev" (clamped at 1) then "next" must end on exactly page 2.
  await sender.getByTestId('send-prev-page').click()
  await sender.getByTestId('send-next-page').click()
  await pageIs(receiver, 2)
  await sender.getByTestId('send-prev-page').click()
  await pageIs(receiver, 1)

  // Still nothing opened after all the page turns, and the viewer has no link at all.
  assert.deepEqual(opened, [], 'no new tab opened')
  assert.equal(receiver.url(), receiverUrl, 'receiver did not navigate')
  assert.equal(await receiver.getByTestId('stage').locator('a[href]').count(), 0, 'the document view contains no links')

  // A damaged PDF (valid marker, garbage after it) gives the translated notice and no document.
  const garbage = Buffer.concat([Buffer.from('%PDF-1.4\n'), Buffer.alloc(40000, 0x41)])
  await content.file({ name: 'broken.pdf', mimeType: 'application/pdf', buffer: garbage })
  await receiver.getByTestId('toast').filter({ hasText: /damaged or not supported/ }).waitFor({ timeout: 30000 })
  assert.equal(await receiver.getByTestId('document-canvas').count(), 0)

  // The sender refuses a file that is no PDF before sending anything.
  await content.file({ name: 'text.pdf', mimeType: 'application/pdf', buffer: Buffer.from('hello world') })
  await sender.getByTestId('file-error').filter({ hasText: /does not look like a PDF/ }).waitFor({ timeout: 10000 })

  // A good PDF again, then stop from the presenter closes it.
  await content.file({ name: 'lesson.pdf', mimeType: 'application/pdf', buffer: pdf })
  await receiver.getByText('Page 1 of 3').waitFor({ timeout: 30000 })
  await content.stop()
  await receiver.getByTestId('document-canvas').waitFor({ state: 'detached', timeout: 10000 })
  await sctx.close()
  await rctx.close()

  // ---------- German texts ----------
  const { ctx: dr, page: de } = await newPage('de-DE')
  const code2 = await openDisplay(de, base)
  const { ctx: ds, page: deSender } = await newPage('de-DE')
  await presenterJoin(deSender, base, code2)
  await contentSender(deSender).file({ name: 'folien.pdf', mimeType: 'application/pdf', buffer: pdf })
  await de.getByText('Seite 1 von 3').waitFor({ timeout: 30000 })
  await deSender.getByRole('button', { name: 'Nächste Seite' }).click()
  await de.getByText('Seite 2 von 3').waitFor({ timeout: 10000 })
  await ds.close()
  await dr.close()

  assert.deepEqual(errors, [], `page errors: ${errors.join('; ')}`)
  console.log('E2E PDF OK')
} finally {
  await browser?.close()
  server.kill('SIGTERM')
}
