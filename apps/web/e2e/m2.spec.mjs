// SPDX-License-Identifier: GPL-3.0-or-later
//
// End-to-end test for milestone M2 (web part): receiver toolbar and annotations, freeze, and content
// (image, link) over the WebRTC DataChannel. Real server binary + real Chromium pages.
// Run: npm run build && npm run e2e:m2
// Optional: E2E_SHOTS=<dir> writes screenshots (1280x800, light/dark, en/de) of the receiver.
import { deflateSync } from 'node:zlib'
import { mkdirSync } from 'node:fs'
import { resolve } from 'node:path'
import assert from 'node:assert/strict'
import { launch, newContext, openDisplay, presenterJoin, startServer, status, waitFor, waitHealthy } from './helpers.mjs'

const realDisplay = process.env.E2E_TEST_VIDEO !== '1'
const shotDir = process.env.E2E_SHOTS

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
const { base } = server
let browser
try {
  await waitHealthy(base)
  browser = await launch()
  const rctx = await newContext(browser, { viewport: { width: 1280, height: 800 } })
  const sctx = await newContext(browser, { viewport: { width: 1280, height: 800 } })
  // Serve the link target locally so that "Open" can be verified without internet access.
  let linkRequests = 0
  await rctx.route('https://example.org/**', (r) => {
    linkRequests++
    return r.fulfill({ contentType: 'text/html', body: '<title>target</title>opened' })
  })

  // --- Display: room code
  const receiver = await rctx.newPage()
  const code = await openDisplay(receiver, base)
  console.log('room code:', code)

  // ===== Part A: screen share + toolbar =======================================================
  const sender = await sctx.newPage()
  await sender.goto(realDisplay ? base : `${base}/?testVideo=1`)
  await sender.getByTestId('start-code').fill(code)
  await sender.getByTestId('start-browser').click()
  await sender.getByTestId('what-page').waitFor({ timeout: 15000 })
  await sender.getByTestId('what-screen').click()
  await sender.locator('[data-testid=status][data-phase=sharing]').waitFor({ timeout: 15000 })
  await status(receiver, 'receiving')
  const video = receiver.getByTestId('remote-video')
  await waitFor(() => video.evaluate((v) => v.videoWidth > 0 && v.currentTime > 0), 'video playing')

  // The toolbar is hidden at rest (presenting is quiet) and comes with the name tag and End (spec 4.5).
  const tb = receiver.getByTestId('toolbar')
  const tbRoot = receiver.getByTestId('toolbar-root')
  const opacity = () => tbRoot.evaluate((e) => Number(getComputedStyle(e).opacity))
  const hidden = async () => (await tbRoot.getAttribute('class')).split(/\s+/).includes('hid')
  let jiggle = 0
  /** Input on the picture: moves the mouse a little and waits until the controls are shown. */
  const reveal = async () => {
    const vp = await receiver.evaluate(() => [innerWidth, innerHeight])
    jiggle = (jiggle + 37) % 200
    await receiver.mouse.move(vp[0] / 2 - 100 + jiggle, vp[1] * 0.7 + (jiggle % 50))
    await waitFor(async () => !(await hidden()), 'controls shown')
  }
  await waitFor(async () => (await opacity()) === 0, 'toolbar faded out at rest')
  assert.notEqual(await tbRoot.getAttribute('inert'), null, 'the hidden toolbar is not focusable')
  assert.equal(await receiver.getByTestId('byline').getAttribute('class').then((c) => c.includes('hid')), true)
  await reveal()
  await waitFor(async () => (await opacity()) === 1, 'toolbar faded in with the name tag')
  assert.equal(await tbRoot.getAttribute('inert'), null)
  assert.equal(await receiver.getByTestId('end').isVisible(), true)

  // Toolbar: role, translated labels; no grip, no collapse, no dot (it does not move and hides by itself)
  assert.equal(await tb.getAttribute('role'), 'toolbar')
  assert.equal(await receiver.getByTestId('tb-pen').getAttribute('aria-label'), 'Pen')
  assert.equal(await receiver.getByTestId('tb-pen').getAttribute('aria-pressed'), 'false')
  for (const id of ['toolbar-grip', 'tb-collapse', 'toolbar-dot']) assert.equal(await receiver.getByTestId(id).count(), 0, id)
  // top 12 reference px, centred, tokens only: panel fill, 2 px --line border (dark theme: #2e3c68)
  const tbBox = await tb.boundingBox()
  const vpW = await receiver.evaluate(() => innerWidth)
  const scale = Math.min(vpW / 960, (await receiver.evaluate(() => innerHeight)) / 540)
  assert.ok(Math.abs(tbBox.y - 12 * scale) < 2, `toolbar top ${tbBox.y} ~ ${12 * scale}`)
  assert.ok(Math.abs(tbBox.x + tbBox.width / 2 - vpW / 2) < 2, 'toolbar centred')
  assert.ok(Math.abs(tbBox.height - 46 * scale) < 2, 'toolbar 46 reference px high')
  const tbStyle = await tb.evaluate((e) => {
    const s = getComputedStyle(e)
    return [s.borderTopColor, s.borderTopWidth]
  })
  assert.equal(tbStyle[0], 'rgb(46, 60, 104)')
  assert.ok(Math.abs(parseFloat(tbStyle[1]) - 2 * scale) <= 1, `border ${tbStyle[1]} ~ ${2 * scale} (rounded to device pixels)`)

  // Keyboard: arrow keys move focus between buttons (roving tabindex)
  await receiver.getByTestId('tb-pen').focus()
  await receiver.keyboard.press('ArrowRight')
  assert.equal(await receiver.evaluate(() => document.activeElement?.getAttribute('data-testid')), 'tb-marker')
  await receiver.keyboard.press('Home')
  assert.equal(await receiver.evaluate(() => document.activeElement?.getAttribute('data-testid')), 'tb-pen')

  // Pen: drawing changes canvas pixels; nothing is drawn before
  const canvas = receiver.getByTestId('annotation-canvas')
  assert.equal(await paintedPixels(receiver, 'annotation-canvas'), 0, 'canvas starts empty')
  await reveal()
  await receiver.getByTestId('tb-pen').click()
  assert.equal(await canvas.getAttribute('data-tool'), 'pen')
  const box = await canvas.boundingBox()
  const cx = box.x + box.width / 2
  // Only the visible part of the canvas can be drawn on: with the relay line and the connection
  // check code above the stage, the canvas reaches below a 800 px high window.
  const viewportHeight = await receiver.evaluate(() => window.innerHeight)
  const visibleHeight = Math.min(box.height, viewportHeight - box.y)
  const cy = box.y + visibleHeight * 0.55
  await receiver.mouse.move(cx - 200, cy)
  await receiver.mouse.down()
  await receiver.mouse.move(cx, cy + 60, { steps: 12 })
  await receiver.mouse.move(cx + 200, cy - 20, { steps: 12 })
  await receiver.mouse.up()
  // The toolbar auto-hide: drawing is input (strokes stay under the faded toolbar); the toolbar goes 5 s after the last input
  await waitFor(async () => (await hidden()), 'toolbar hidden again 5 s after the last input', 9000)
  assert.ok((await paintedPixels(receiver, 'annotation-canvas')) > 0, 'strokes stay visible after the controls fade')
  const painted = await waitFor(async () => (await paintedPixels(receiver, 'annotation-canvas')) || 0, 'pen pixels')
  console.log('annotation pixels:', painted)
  assert.ok(painted > 500, 'pen drew a visible stroke')
  // Marker with another colour, then undo removes only the marker stroke
  await reveal()
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
  await reveal()
  await receiver.getByTestId('tb-undo').click()
  await waitFor(async () => (await paintedPixels(receiver, 'annotation-canvas')) === painted, 'undo restores pen-only pixels')
  assert.ok(both > painted)
  // Annotations are local: the sender page has no annotation canvas and its status is unaffected
  assert.equal(await sender.getByTestId('annotation-canvas').count(), 0)

  if (shotDir) mkdirSync(shotDir, { recursive: true })
  if (shotDir) await receiver.screenshot({ path: resolve(shotDir, 'receiver-annotation-en.png') })

  // Eraser removes the pen stroke; clear is disabled when nothing is left
  await reveal()
  await receiver.getByTestId('tb-eraser').click()
  await receiver.mouse.move(cx - 200, cy)
  await receiver.mouse.down()
  await receiver.mouse.move(cx, cy + 60, { steps: 12 })
  await receiver.mouse.move(cx + 200, cy - 20, { steps: 12 })
  await receiver.mouse.up()
  await waitFor(async () => (await paintedPixels(receiver, 'annotation-canvas')) === 0, 'eraser removed the stroke')
  assert.equal(await receiver.getByTestId('tb-clear').isDisabled(), true)
  await reveal()
  await receiver.getByTestId('tb-undo').click()
  await waitFor(async () => (await paintedPixels(receiver, 'annotation-canvas')) > 0, 'undo of erase')
  await receiver.getByTestId('tb-eraser').click() // deselect

  // Laser: a dot follows the pointer, nothing is painted
  const before = await paintedPixels(receiver, 'annotation-canvas')
  await reveal()
  await receiver.getByTestId('tb-laser').click()
  await receiver.mouse.move(cx, cy - 100)
  await receiver.getByTestId('laser-dot').waitFor()
  assert.equal(await paintedPixels(receiver, 'annotation-canvas'), before)
  await reveal()
  await receiver.getByTestId('tb-laser').click()

  // Freeze: canvas shows the held frame while the live stream keeps running
  const frozenAt = await video.evaluate((v) => v.currentTime)
  await reveal()
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
  // The frozen badge is a state: drawn at rest (name tag style, top left) while the controls are hidden
  await waitFor(async () => (await hidden()), 'controls hidden while frozen', 9000)
  await receiver.getByTestId('frozen-badge').waitFor({ state: 'visible' })
  const badge = await receiver.getByTestId('frozen-badge').boundingBox()
  assert.ok(Math.abs(badge.x - 12 * scale) < 2 && Math.abs(badge.y - 12 * scale) < 2, 'frozen badge top left')
  await reveal()
  await receiver.getByTestId('tb-freeze').click()
  assert.equal(await stage.getAttribute('data-frozen'), 'false')

  // Black screen: the hint is 15 reference px --mut on black, drawn at rest too
  await reveal()
  await receiver.getByTestId('tb-black').click()
  await receiver.getByTestId('black-screen').waitFor()
  await waitFor(async () => (await hidden()), 'controls hidden over the black screen', 9000)
  assert.equal(await receiver.getByTestId('black-screen').isVisible(), true)
  const hint = await receiver.locator('.cd-blackhint').evaluate((e) => {
    const s = getComputedStyle(e)
    return [s.fontSize, s.color]
  })
  assert.equal(parseFloat(hint[0]), 15 * scale)
  assert.equal(hint[1], 'rgb(185, 194, 216)')
  await reveal()
  await receiver.getByTestId('tb-black').click()
  await receiver.getByTestId('black-screen').waitFor({ state: 'detached' })

  // ===== Part B: content over the data channel ===============================================
  // The presenter stops showing the screen but stays connected, then shows a file and a link.
  await sender.getByTestId('presenting-stop').click()
  await sender.getByTestId('what-page').waitFor()
  await receiver.getByTestId('idle').waitFor({ state: 'visible', timeout: 10000 })
  const cs = sender
  const toFile = async () => {
    await cs.getByTestId('what-file').click()
    await cs.getByTestId('file-page').waitFor()
  }
  const toLink = async () => {
    await cs.getByTestId('what-link').click()
    await cs.getByTestId('link-page').waitFor()
  }
  const back = async () => {
    await cs.getByTestId('presenting-stop').click()
    await cs.getByTestId('what-page').waitFor()
  }

  // Image
  const png = makePng(64, 32)
  await toFile()
  // Wrong type is refused on the presenter, nothing arrives
  await cs.getByTestId('file-input').setInputFiles({ name: 'x.txt', mimeType: 'text/plain', buffer: Buffer.from('hello') })
  await waitFor(async () => /Only PNG/.test(await cs.getByTestId('file-error').innerText()), 'bad type message')
  await cs.getByTestId('file-input').setInputFiles({ name: 'test.png', mimeType: 'image/png', buffer: png })
  const img = receiver.getByTestId('received-image')
  await img.waitFor({ timeout: 10000 })
  const dims = await waitFor(() => img.evaluate((i) => (i.naturalWidth > 0 ? [i.naturalWidth, i.naturalHeight] : 0)), 'image decoded')
  assert.deepEqual(dims, [64, 32])
  await cs.getByTestId('presenting-page').waitFor()
  console.log('received image natural size:', dims)
  // Nothing is drawn over the picture: no close button (the presenter stops it)
  assert.equal(await receiver.getByTestId('content-close').count(), 0)
  assert.equal(await receiver.locator('[data-testid=stage] button:not([data-tb])').count(), 0)
  await back()
  await img.waitFor({ state: 'detached' })

  // Link: validation on the sender
  const pagesBefore = rctx.pages().length
  await toLink()
  await cs.getByTestId('link-url').fill('javascript:alert(1)')
  await cs.getByTestId('link-go').click()
  assert.match(await cs.getByTestId('link-error').innerText(), /http:\/\/ or https:\/\//)
  // Link: card appears, nothing opens by itself
  await cs.getByTestId('link-url').fill('https://example.org/path?x=1')
  await cs.getByTestId('link-go').click()
  const card = receiver.getByTestId('link-card')
  await card.waitFor({ timeout: 10000 })
  assert.match(await card.innerText(), /example\.org wants to open a link/i) // the title is drawn in capitals
  // The card is a display dialog: Open has the focus; a key press or click within 1 s does nothing
  assert.equal(await receiver.evaluate(() => document.activeElement?.getAttribute('data-testid')), 'link-open')
  assert.equal(await receiver.getByTestId('link-open').getAttribute('class').then((c) => c.includes('p')), true)
  await receiver.keyboard.press('Enter')
  await receiver.keyboard.press('Escape')
  await receiver.getByTestId('link-dismiss').click()
  await new Promise((r) => setTimeout(r, 300))
  assert.equal(await card.count(), 1, 'the card ignores input for 1 s')
  assert.equal(rctx.pages().length, pagesBefore, 'a key press within 1 s opens nothing')
  await new Promise((r) => setTimeout(r, 1500))
  assert.equal(rctx.pages().length, pagesBefore, 'no tab opened before the click')
  assert.equal(linkRequests, 0, 'no request to the link target before the click')
  if (shotDir) await receiver.screenshot({ path: resolve(shotDir, 'receiver-linkcard-en.png') })
  // The focus row is [Open, Dismiss]; Back (Escape) = Dismiss; still nothing opens
  await receiver.keyboard.press('ArrowRight')
  assert.equal(await receiver.evaluate(() => document.activeElement?.getAttribute('data-testid')), 'link-dismiss')
  await receiver.keyboard.press('ArrowLeft')
  await receiver.keyboard.press('Escape')
  await card.waitFor({ state: 'detached' })
  await new Promise((r) => setTimeout(r, 500))
  assert.equal(rctx.pages().length, pagesBefore, 'dismiss does not open a tab')
  // Again, now click Open: a new tab opens
  await back()
  await toLink()
  await cs.getByTestId('link-url').fill('https://example.org/path?x=1')
  await cs.getByTestId('link-go').click()
  await card.waitFor({ timeout: 10000 })
  await new Promise((r) => setTimeout(r, 1300)) // the 1 s guard
  const [opened] = await Promise.all([rctx.waitForEvent('page'), receiver.getByTestId('link-open').click()])
  await opened.waitForLoadState()
  assert.match(opened.url(), /^https:\/\/example\.org\/path\?x=1$/)
  assert.equal(rctx.pages().length, pagesBefore + 1)
  await card.waitFor({ state: 'detached' })
  await opened.close()

  // "Stop showing" from the presenter closes an image on the display
  await back()
  await toFile()
  await cs.getByTestId('file-input').setInputFiles({ name: 'test.png', mimeType: 'image/png', buffer: png })
  await receiver.getByTestId('received-image').waitFor()
  await cs.getByTestId('presenting-stop').click()
  await receiver.getByTestId('received-image').waitFor({ state: 'detached' })

  console.log('E2E M2 OK')
} finally {
  await browser?.close()
  server.proc.kill('SIGTERM')
}
