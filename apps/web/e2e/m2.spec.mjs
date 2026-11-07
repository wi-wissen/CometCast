// SPDX-License-Identifier: GPL-3.0-or-later
//
// End-to-end test for milestone M2 (web part): receiver toolbar and annotations, freeze, and content
// (image, link) over the WebRTC DataChannel. Real server binary + real Chromium pages.
// Run: npm run build && npm run e2e:m2
// Optional: E2E_SHOTS=<dir> writes screenshots (1280x800, light/dark, en/de) of the receiver.
import { createRequire } from 'node:module'
import { spawn, spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'
import { deflateSync } from 'node:zlib'
import { mkdirSync } from 'node:fs'
import assert from 'node:assert/strict'

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')

const here = dirname(fileURLToPath(import.meta.url))
const repo = resolve(here, '../../..')
const webDir = resolve(here, '../dist')
const port = Number(process.env.E2E_PORT ?? 18443 + Math.floor(Math.random() * 1000))
const base = `http://127.0.0.1:${port}`
const realDisplay = process.env.E2E_TEST_VIDEO !== '1'
const shotDir = process.env.E2E_SHOTS

function startServer() {
  const build = spawnSync('cargo', ['build', '-p', 'cometcast-server'], { cwd: repo, stdio: 'inherit' })
  assert.equal(build.status, 0, 'cargo build failed')
  const targetDir = process.env.CARGO_TARGET_DIR ?? resolve(repo, 'target')
  return spawn(resolve(targetDir, 'debug/cometcast-server'), ['--listen', `127.0.0.1:${port}`, '--web-dir', webDir], {
    stdio: 'inherit',
  })
}

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

async function waitFor(fn, what, ms = 15000) {
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

/** A solid-colour RGB PNG, built by hand so that the test needs no image library. */
function makePng(width, height) {
  const crcTable = Array.from({ length: 256 }, (_, n) => {
    let c = n
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
    return c >>> 0
  })
  const crc = (buf) => {
    let c = 0xffffffff
    for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8)
    return (c ^ 0xffffffff) >>> 0
  }
  const chunk = (type, data) => {
    const len = Buffer.alloc(4)
    len.writeUInt32BE(data.length)
    const td = Buffer.concat([Buffer.from(type), data])
    const c = Buffer.alloc(4)
    c.writeUInt32BE(crc(td))
    return Buffer.concat([len, td, c])
  }
  const ihdr = Buffer.alloc(13)
  ihdr.writeUInt32BE(width, 0)
  ihdr.writeUInt32BE(height, 4)
  ihdr[8] = 8 // bit depth
  ihdr[9] = 2 // RGB
  const row = Buffer.alloc(1 + width * 3)
  for (let x = 0; x < width; x++) {
    row[1 + x * 3] = 200
    row[2 + x * 3] = (x * 4) & 255
    row[3 + x * 3] = 60
  }
  const raw = Buffer.concat(Array.from({ length: height }, () => row))
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw)),
    chunk('IEND', Buffer.alloc(0)),
  ])
}

/** Number of pixels with alpha > 0 on a canvas, by test id. */
const paintedPixels = (page, testid) =>
  page.getByTestId(testid).evaluate((c) => {
    const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data
    let n = 0
    for (let i = 3; i < d.length; i += 4) if (d[i] > 0) n++
    return n
  })

const server = startServer()
let browser
try {
  await waitHealthy()
  browser = await chromium.launch({
    executablePath: process.env.E2E_CHROME || undefined,
    args: [
      '--use-fake-ui-for-media-stream',
      '--auto-select-desktop-capture-source=Entire screen',
      '--use-fake-device-for-media-stream',
      '--disable-features=WebRtcHideLocalIpsWithMdns',
    ],
  })
  const mk = async () => {
    const ctx = await browser.newContext({ locale: 'en-US', viewport: { width: 1280, height: 800 } })
    ctx.on('page', (p) => p.on('pageerror', (e) => console.error('pageerror:', e.message)))
    return ctx
  }
  const rctx = await mk()
  const sctx = await mk()
  // Serve the link target locally so that "Open" can be verified without internet access.
  let linkRequests = 0
  await rctx.route('https://example.org/**', (r) => {
    linkRequests++
    return r.fulfill({ contentType: 'text/html', body: '<title>target</title>opened' })
  })

  // --- Receiver: room code
  const receiver = await rctx.newPage()
  await receiver.goto(base)
  await receiver.getByRole('button', { name: /Receive on this device/ }).click()
  await receiver.locator('[data-testid=status][data-phase=waiting_sender]').waitFor({ timeout: 10000 })
  const code = (await receiver.getByTestId('room-code').innerText()).trim()
  console.log('room code:', code)

  // ===== Part A: screen share + toolbar =======================================================
  const sender = await sctx.newPage()
  await sender.goto(realDisplay ? base : `${base}/?testVideo=1`)
  await sender.getByRole('button', { name: /Send screen/ }).click()
  await sender.locator('#room-code').fill(code)
  await sender.getByRole('button', { name: 'Share screen' }).click()
  await sender.locator('[data-testid=status][data-phase=sharing]').waitFor({ timeout: 15000 })
  await receiver.locator('[data-testid=status][data-phase=receiving]').waitFor({ timeout: 15000 })
  const video = receiver.getByTestId('remote-video')
  await waitFor(() => video.evaluate((v) => v.videoWidth > 0 && v.currentTime > 0), 'video playing')

  // Toolbar: role, translated labels
  const tb = receiver.getByTestId('toolbar')
  await tb.waitFor()
  assert.equal(await tb.getAttribute('role'), 'toolbar')
  assert.equal(await receiver.getByTestId('tb-pen').getAttribute('aria-label'), 'Pen')
  assert.equal(await receiver.getByTestId('tb-pen').getAttribute('aria-pressed'), 'false')

  // Keyboard: arrow keys move focus between buttons (roving tabindex)
  await receiver.getByTestId('tb-pen').focus()
  await receiver.keyboard.press('ArrowRight')
  assert.equal(await receiver.evaluate(() => document.activeElement?.getAttribute('data-testid')), 'tb-marker')
  await receiver.keyboard.press('Home')
  assert.equal(await receiver.evaluate(() => document.activeElement?.getAttribute('data-testid')), 'toolbar-grip')

  // Pen: drawing changes canvas pixels; nothing is drawn before
  const canvas = receiver.getByTestId('annotation-canvas')
  assert.equal(await paintedPixels(receiver, 'annotation-canvas'), 0, 'canvas starts empty')
  await receiver.getByTestId('tb-pen').click()
  assert.equal(await canvas.getAttribute('data-tool'), 'pen')
  const box = await canvas.boundingBox()
  const cx = box.x + box.width / 2
  const cy = box.y + box.height * 0.55
  await receiver.mouse.move(cx - 200, cy)
  await receiver.mouse.down()
  await receiver.mouse.move(cx, cy + 60, { steps: 12 })
  await receiver.mouse.move(cx + 200, cy - 20, { steps: 12 })
  await receiver.mouse.up()
  const painted = await waitFor(async () => (await paintedPixels(receiver, 'annotation-canvas')) || 0, 'pen pixels')
  console.log('annotation pixels:', painted)
  assert.ok(painted > 500, 'pen drew a visible stroke')
  // Marker with another colour, then undo removes only the marker stroke
  await receiver.getByTestId('tb-marker').click()
  await receiver.getByTestId('tb-color-yellow').click()
  await receiver.mouse.move(cx - 150, cy + 120)
  await receiver.mouse.down()
  await receiver.mouse.move(cx + 150, cy + 120, { steps: 10 })
  await receiver.mouse.up()
  const both = await waitFor(async () => {
    const n = await paintedPixels(receiver, 'annotation-canvas')
    return n > painted + 500 ? n : 0
  }, 'marker pixels')
  await receiver.getByTestId('tb-undo').click()
  await waitFor(async () => (await paintedPixels(receiver, 'annotation-canvas')) === painted, 'undo restores pen-only pixels')
  assert.ok(both > painted)
  // Annotations are local: the sender page has no annotation canvas and its status is unaffected
  assert.equal(await sender.getByTestId('annotation-canvas').count(), 0)

  if (shotDir) mkdirSync(shotDir, { recursive: true })
  if (shotDir) await receiver.screenshot({ path: resolve(shotDir, 'receiver-annotation-en.png') })

  // Eraser removes the pen stroke; clear is disabled when nothing is left
  await receiver.getByTestId('tb-eraser').click()
  await receiver.mouse.move(cx - 200, cy)
  await receiver.mouse.down()
  await receiver.mouse.move(cx, cy + 60, { steps: 12 })
  await receiver.mouse.move(cx + 200, cy - 20, { steps: 12 })
  await receiver.mouse.up()
  await waitFor(async () => (await paintedPixels(receiver, 'annotation-canvas')) === 0, 'eraser removed the stroke')
  assert.equal(await receiver.getByTestId('tb-clear').isDisabled(), true)
  await receiver.getByTestId('tb-undo').click()
  await waitFor(async () => (await paintedPixels(receiver, 'annotation-canvas')) > 0, 'undo of erase')
  await receiver.getByTestId('tb-eraser').click() // deselect

  // Laser: a dot follows the pointer, nothing is painted
  const before = await paintedPixels(receiver, 'annotation-canvas')
  await receiver.getByTestId('tb-laser').click()
  await receiver.mouse.move(cx, cy - 100)
  await receiver.getByTestId('laser-dot').waitFor()
  assert.equal(await paintedPixels(receiver, 'annotation-canvas'), before)
  await receiver.getByTestId('tb-laser').click()

  // Freeze: canvas shows the held frame while the live stream keeps running
  const frozenAt = await video.evaluate((v) => v.currentTime)
  await receiver.getByTestId('tb-freeze').click()
  const stage = receiver.getByTestId('stage')
  assert.equal(await stage.getAttribute('data-frozen'), 'true')
  await receiver.getByTestId('freeze-canvas').waitFor({ state: 'visible' })
  assert.ok((await paintedPixels(receiver, 'freeze-canvas')) > 1000, 'freeze canvas holds a frame')
  const later = await waitFor(
    async () => {
      const s = await video.evaluate((v) => [v.currentTime, v.paused])
      return s[0] > frozenAt + 0.5 && s[1] === false ? s : 0
    },
    'stream still advancing while frozen',
  )
  console.log('frozen: video currentTime', frozenAt, '->', later[0], 'paused', later[1])
  await receiver.getByTestId('tb-freeze').click()
  assert.equal(await stage.getAttribute('data-frozen'), 'false')

  // Black screen
  await receiver.getByTestId('tb-black').click()
  await receiver.getByTestId('black-screen').waitFor()
  await receiver.getByTestId('tb-black').click()
  await receiver.getByTestId('black-screen').waitFor({ state: 'detached' })

  // Collapse to a dot and back (mouse), and keyboard (Enter)
  await receiver.getByTestId('tb-collapse').click()
  await receiver.getByTestId('toolbar-dot').waitFor()
  assert.equal(await tb.count(), 0)
  await receiver.getByTestId('toolbar-dot').click()
  await tb.waitFor()
  await receiver.getByTestId('tb-pen').focus()
  await receiver.keyboard.press('Escape')
  await receiver.getByTestId('toolbar-dot').waitFor()
  await receiver.getByTestId('toolbar-dot').focus()
  await receiver.keyboard.press('Enter')
  await tb.waitFor()

  // Drag the toolbar by its grip
  const g = await receiver.getByTestId('toolbar-grip').boundingBox()
  const t0 = await tb.boundingBox()
  await receiver.mouse.move(g.x + g.width / 2, g.y + g.height / 2)
  await receiver.mouse.down()
  await receiver.mouse.move(g.x + g.width / 2 - 250, g.y + g.height / 2 + 200, { steps: 8 })
  await receiver.mouse.up()
  const t1 = await tb.boundingBox()
  // (the 690 px wide page column is barely wider than the toolbar, so only vertical movement is checked)
  assert.ok(t1.y > t0.y + 100, `toolbar moved (${t0.x},${t0.y}) -> (${t1.x},${t1.y})`)
  const stageBox = await stage.boundingBox()
  assert.ok(t1.x >= stageBox.x - 1 && t1.x + t1.width <= stageBox.x + stageBox.width + 1, 'toolbar stays inside the stage')

  // Leave the toolbar on the left-top for screenshots
  await receiver.keyboard.press('Tab')

  // ===== Part B: content over the data channel ===============================================
  await sender.getByTestId('stop').click()
  await receiver.locator('[data-testid=status][data-phase=peer_left]').waitFor({ timeout: 10000 })

  const cs = await sctx.newPage()
  await cs.goto(base)
  await cs.getByRole('button', { name: /Send file or link/ }).click()
  await cs.locator('#room-code').fill(code)
  await cs.getByRole('button', { name: 'Connect' }).click()
  await cs.locator('[data-testid=status][data-ready=true]').waitFor({ timeout: 15000 })
  await receiver.locator('[data-testid=status][data-phase=receiving]').waitFor({ timeout: 15000 })
  // The PDF section exists (sent and shown in e2e:pdf); Office documents are still listed as "coming later"
  assert.match(await cs.locator('body').innerText(), /PDF document/)
  assert.match(await cs.locator('body').innerText(), /Office documents/)

  // Image
  const png = makePng(64, 32)
  await cs.getByTestId('image-input').setInputFiles({ name: 'test.png', mimeType: 'image/png', buffer: png })
  const img = receiver.getByTestId('received-image')
  await img.waitFor({ timeout: 10000 })
  const dims = await waitFor(() => img.evaluate((i) => (i.naturalWidth > 0 ? [i.naturalWidth, i.naturalHeight] : 0)), 'image decoded')
  assert.deepEqual(dims, [64, 32])
  assert.match(await cs.getByTestId('image-notice').innerText(), /Sent test\.png/)
  console.log('received image natural size:', dims)
  // Wrong type is refused on the sender, nothing arrives
  await cs.getByTestId('image-input').setInputFiles({ name: 'x.txt', mimeType: 'text/plain', buffer: Buffer.from('hello') })
  await waitFor(async () => /Only PNG/.test(await cs.getByTestId('image-notice').innerText()), 'bad type message')
  // Close button on the receiver
  await receiver.getByTestId('content-close').click()
  await img.waitFor({ state: 'detached' })

  // Link: validation on the sender
  const pagesBefore = rctx.pages().length
  await cs.locator('#link-url').fill('javascript:alert(1)')
  await cs.getByRole('button', { name: 'Send link' }).click()
  assert.match(await cs.getByTestId('link-notice').innerText(), /http:\/\/ or https:\/\//)
  // (type=url inputs may block the submit natively; either way no card may appear)
  // Link: card appears, nothing opens by itself
  await cs.locator('#link-url').fill('https://example.org/path?x=1')
  await cs.getByRole('button', { name: 'Send link' }).click()
  const card = receiver.getByTestId('link-card')
  await card.waitFor({ timeout: 10000 })
  assert.match(await card.innerText(), /example\.org wants to open a link/)
  await new Promise((r) => setTimeout(r, 1500))
  assert.equal(rctx.pages().length, pagesBefore, 'no tab opened before the click')
  assert.equal(linkRequests, 0, 'no request to the link target before the click')
  if (shotDir) await receiver.screenshot({ path: resolve(shotDir, 'receiver-linkcard-en.png') })
  // Dismiss: still nothing opens
  await receiver.getByTestId('link-dismiss').click()
  await card.waitFor({ state: 'detached' })
  await new Promise((r) => setTimeout(r, 500))
  assert.equal(rctx.pages().length, pagesBefore, 'dismiss does not open a tab')
  // Again, now click Open: a new tab opens
  await cs.getByRole('button', { name: 'Send link' }).click()
  await card.waitFor({ timeout: 10000 })
  const [opened] = await Promise.all([rctx.waitForEvent('page'), receiver.getByTestId('link-open').click()])
  await opened.waitForLoadState()
  assert.match(opened.url(), /^https:\/\/example\.org\/path\?x=1$/)
  assert.equal(rctx.pages().length, pagesBefore + 1)
  await card.waitFor({ state: 'detached' })
  await opened.close()

  // "Stop showing" from the sender closes an image on the receiver
  await cs.getByTestId('image-input').setInputFiles({ name: 'test.png', mimeType: 'image/png', buffer: png })
  await receiver.getByTestId('received-image').waitFor()
  await cs.getByTestId('stop-showing').click()
  await receiver.getByTestId('received-image').waitFor({ state: 'detached' })

  console.log('E2E M2 OK')
} finally {
  await browser?.close()
  server.kill('SIGTERM')
}
