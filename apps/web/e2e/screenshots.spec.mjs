// SPDX-License-Identifier: GPL-3.0-or-later
//
// Writes a PNG of every state of the display and of the presenter pages, for Dark / Light / OLED black
// (display) and light / dark (presenter), at 1280 x 720 and at a phone viewport (390 x 844). It checks
// (almost) nothing; it exists so that a designer can compare the pages with the spec. Every display shot is taken
// after one Shift key press (an arrow only where a moved focus is the point), so that the focus ring shows where a control has focus (a click hides it by design);
// the exceptions are the states that exist only while nothing was touched (presenting at rest, dimmed idle).
// Needs the PDF renderer: npm run build:wasm && npm run build
// Run: E2E_CHROME=/usr/bin/google-chrome E2E_SHOTS=/tmp/shots3 npm run e2e:shots
import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import assert from 'node:assert/strict'
import { makePdf } from './pdfFixture.mjs'
import { launch, newContext, presenterJoin, presenterScreen, presenterStart, runWizard, startServer, status, waitHealthy, webDist, webRoot } from './helpers.mjs'

assert.ok(existsSync(join(webDist, 'pdf/cc_docs_wasm_bg.wasm')), 'dist/pdf/cc_docs_wasm_bg.wasm is missing: run `npm run build:wasm && npm run build` first')
const out = process.env.E2E_SHOTS || '/tmp/shots'
mkdirSync(out, { recursive: true })
const tmp = mkdtempSync(join(tmpdir(), 'cometcast-shots-'))
const dimDist = join(tmp, 'dist-dim')
assert.equal(spawnSync('npx', ['vite', 'build', '--outDir', dimDist, '--emptyOutDir'], { cwd: webRoot, stdio: 'ignore', env: { ...process.env, VITE_E2E_DIM_MS: '1200' } }).status, 0)

const VIEWS = [
  ['1280', { width: 1280, height: 720 }, {}],
  ['phone', { width: 390, height: 844 }, { isMobile: true, hasTouch: true }],
]
const THEMES = (process.env.E2E_SHOTS_THEMES || 'dark,light,oled').split(',')
const servers = []
const server = async (opts) => {
  const s = startServer(opts)
  servers.push(s)
  await waitHealthy(s.base)
  return s
}
const shot = (page, name) => page.screenshot({ path: join(out, `${name}.png`) })
// A display shot: after one key press (the first key press, a modifier alone included, shows the focus ring again).
// Shift shows the ring and moves nothing; an arrow only where a moved focus is the point.
const dshot = async (page, name, key = 'Shift') => {
  await page.keyboard.press(key)
  await page.waitForTimeout(250)
  await shot(page, name)
}
/** Contexts are closed with a limit: a page that cannot be closed must not hold the whole run. */
const closeAll = (...ctxs) => Promise.all(ctxs.map((c) => Promise.race([c.close().catch(() => undefined), new Promise((r) => setTimeout(r, 8000))])))
const armed = (page) => page.waitForTimeout(1200)
const FAKE_PDF = { name: 'broken.pdf', mimeType: 'application/pdf', buffer: Buffer.concat([Buffer.from('%PDF-1.4\n'), Buffer.alloc(2000, 0x20)]) }
const SETUP = JSON.stringify({ room: 'Meeting room Elbe', mode: 'open', server: null })

/** A relay that is not there: `onMsg(message, send, ws)` answers instead of the server (hello is answered by default). */
const fakeRelay = (page, onMsg) =>
  page.routeWebSocket(/\/ws$/, (ws) => {
    const send = (o) => ws.send(JSON.stringify(o))
    ws.onMessage((raw) => {
      const m = JSON.parse(String(raw))
      if (m.type === 'hello') return send({ type: 'hello', proto_version: onMsg.protoVersion ?? 2 })
      onMsg(m, send, ws)
    })
  })

// Test seams in the display page: the PDF worker can be told to fail a page, and the control channel is reachable (a transfer
// that stands still, a failed transfer) without a second device.
const DISPLAY_SEAMS = () => {
  const W = window.Worker
  window.Worker = class extends W {
    postMessage(m, t) {
      if (window.__holdRender && m && m.type === 'render') {
        ;(window.__held ||= []).push([this, m, t])
        return
      }
      if (window.__failRender && m && m.type === 'render') {
        queueMicrotask(() => this.dispatchEvent(new MessageEvent('message', { data: { type: 'error', id: m.id, code: 'bad_size' } })))
        return
      }
      super.postMessage(m, t)
    }
  }
  window.__releaseRender = () => {
    window.__holdRender = false
    for (const [w, m, t] of window.__held || []) W.prototype.postMessage.call(w, m, t)
    window.__held = []
  }
  const PC = window.RTCPeerConnection
  window.RTCPeerConnection = class extends PC {
    constructor(...a) {
      super(...a)
      this.addEventListener('datachannel', (e) => (window.__ch = e.channel))
    }
  }
}

let browser
try {
  browser = await launch()
  const s = await server()
  const sdim = await server({ webDir: dimDist })

  // ---------------- display: every theme and viewport ----------------
  // E2E_SHOTS_ONLY=presenter skips the display part (to redo only the presenter pages)
  // E2E_SHOTS_VIEWS=1280 or =phone, E2E_SHOTS_THEMES=dark,light,oled: only these (to run parts of the set side by side)
  const viewsWanted = (process.env.E2E_SHOTS_VIEWS || '1280,phone').split(',')
  const only = process.env.E2E_SHOTS_ONLY
  for (const [vname, viewport, extra] of only === 'presenter' ? [] : VIEWS.filter(([n]) => viewsWanted.includes(n))) {
    for (const theme of THEMES) {
      const tag = `display-${vname}-${theme}`
      const mkctx = (colorScheme = 'light', { setup = false } = {}) =>
        newContext(browser, { viewport, ...extra, colorScheme }).then(async (c) => {
          await c.addInitScript(
            ([t, su]) => {
              if (!localStorage.getItem('cometcast.display.appearance')) localStorage.setItem('cometcast.display.appearance', t)
              if (su && !localStorage.getItem('cometcast.display.setup')) localStorage.setItem('cometcast.display.setup', su)
            },
            [theme, setup ? SETUP : ''],
          )
          await c.addInitScript(DISPLAY_SEAMS)
          return c
        })
      // wizard
      {
        const c = await mkctx()
        const d = await c.newPage()
        await d.goto(`${s.base}/display`)
        await d.getByTestId('wz-start').waitFor()
        await dshot(d, `${tag}-setup-0-welcome`)
        await d.getByTestId('wz-start').click()
        await dshot(d, `${tag}-setup-1-name`)
        await d.keyboard.press('ArrowUp') // from Next to the text field: the field's focus ring
        await d.waitForTimeout(150)
        await shot(d, `${tag}-focus-setup-name-field`)
        await d.getByTestId('wz-room').fill('X')
        await d.getByTestId('wz-room').fill('')
        await dshot(d, `${tag}-setup-1-name-emptied`, 'ArrowRight')
        await d.getByTestId('wz-next').click()
        await dshot(d, `${tag}-setup-2-mode`)
        await d.getByTestId('wz-next').click()
        await dshot(d, `${tag}-setup-3-address`)
        await d.getByTestId('wz-change').click()
        await d.waitForTimeout(150)
        await dshot(d, `${tag}-setup-3-address-field`)
        await d.getByTestId('wz-addr').fill('ws://public.example.org')
        await d.keyboard.press('Enter')
        await d.getByTestId('wz-addr-error').waitFor()
        await dshot(d, `${tag}-setup-3-address-error`)
        await d.keyboard.press('Escape')
        await d.getByTestId('wz-ok').click()
        await dshot(d, `${tag}-setup-4-done`)
        await c.close()
      }
      // idle and everything over it
      const c = await mkctx()
      const d = await c.newPage()
      await d.goto(`${s.base}/display`)
      await runWizard(d, { mode: 'confirm' })
      await status(d, 'waiting_sender')
      await d.getByTestId('room-code').waitFor()
      await d.waitForTimeout(300)
      await dshot(d, `${tag}-idle`)
      await d.getByTestId('settings-open').click()
      await d.getByTestId('settings-modal').waitFor()
      await d.waitForTimeout(250)
      await dshot(d, `${tag}-settings`)
      await d.getByTestId('settings-mode-locked').click()
      await d.getByTestId('settings-close').click()
      await d.waitForTimeout(250)
      await dshot(d, `${tag}-idle-locked`)
      await d.getByTestId('settings-open').click()
      await d.getByTestId('settings-mode-confirm').click()
      await d.getByTestId('settings-close').click()
      const code = (await d.getByTestId('room-code').innerText()).replace(/\s+/g, '')
      const pc1 = await newContext(browser, { colorScheme: 'light' })
      const p1 = await pc1.newPage()
      await presenterStart(p1, s.base, code, { name: 'Lena' })
      await d.getByTestId('request-modal').waitFor()
      const pc2 = await newContext(browser, { colorScheme: 'light' })
      const p2 = await pc2.newPage()
      await presenterStart(p2, s.base, code, { name: 'Ben' })
      await d.getByTestId('request-queue').waitFor()
      await d.waitForTimeout(300)
      await dshot(d, `${tag}-request-queue`)
      await armed(d)
      await d.getByTestId('request-no').click()
      await d.waitForTimeout(300)
      await dshot(d, `${tag}-request`)
      await d.getByTestId('request-yes').click().catch(() => undefined)
      await armed(d)
      await d.getByTestId('request-yes').click()
      await p2.getByTestId('what-page').waitFor({ timeout: 20000 })
      await d.getByTestId('toast').waitFor()
      await d.waitForTimeout(200)
      await dshot(d, `${tag}-toast`)
      await p2.getByTestId('what-screen').click()
      await p2.getByTestId('presenting-page').waitFor({ timeout: 20000 })
      await d.waitForFunction(() => document.querySelector('[data-testid=remote-video]')?.videoWidth > 0)
      await d.waitForTimeout(1200)
      await shot(d, `${tag}-presenting-rest`) // nothing touched: the controls are not there
      await d.keyboard.press('ArrowRight') // the first key only reveals the controls and puts the focus on End
      await d.waitForTimeout(700)
      await shot(d, `${tag}-presenting-controls`)
      // a transfer that stands still (a partial message sequence on the control channel), then one that fails: pill and toast
      await d.evaluate(() => {
        window.__ch.onmessage({ data: JSON.stringify({ kind: 'image', name: 'holiday-photos.png', mime: 'image/png', size: 4000000 }) })
        window.__ch.onmessage({ data: new ArrayBuffer(16384) })
      })
      await d.getByTestId('transfer').waitFor()
      await dshot(d, `${tag}-transfer`)
      await d.evaluate(() => window.__ch.onmessage({ data: JSON.stringify({ kind: 'image_end' }) }))
      await d.getByTestId('toast').waitFor()
      await d.keyboard.press('ArrowRight')
      await d.waitForTimeout(250)
      await shot(d, `${tag}-toast-with-toolbar`)
      // freeze and black screen (the toast of the failed transfer belongs to the shot above: it has to be gone first)
      await d.getByTestId('toast').waitFor({ state: 'detached', timeout: 6000 })
      await d.getByTestId('tb-freeze').click()
      await dshot(d, `${tag}-frozen`) // controls shown: no badge, a selected Freeze button
      await d.waitForTimeout(6500)
      await shot(d, `${tag}-frozen-rest`) // no input for 6 s: the badge alone, top left
      await d.keyboard.press('Shift') // the controls are hidden again after 6 s
      await d.getByTestId('tb-freeze').click()
      await d.getByTestId('tb-black').click()
      await dshot(d, `${tag}-black`)
      await d.getByTestId('tb-black').click()
      // a PDF: pages on black, a page that cannot be drawn
      await p2.getByTestId('presenting-stop').click()
      await p2.getByTestId('what-file').click()
      // the opening line is the transfer pill until the first page is drawn (the worker is held for the shot)
      await d.evaluate(() => (window.__holdRender = true))
      await p2.getByTestId('file-input').setInputFiles({ name: 'slides.pdf', mimeType: 'application/pdf', buffer: makePdf() })
      await d.locator('[data-testid=transfer]', { hasText: 'slides.pdf' }).waitFor({ timeout: 20000 })
      await dshot(d, `${tag}-pdf-opening`)
      await d.evaluate(() => window.__releaseRender())
      await d.getByTestId('document-canvas').waitFor({ timeout: 20000 })
      await d.waitForFunction(() => document.querySelector('[data-testid=document-canvas]')?.dataset.busy === 'false')
      await dshot(d, `${tag}-presenting-pdf`)
      await d.evaluate(() => (window.__failRender = true))
      await p2.getByTestId('send-next-page').click()
      await d.getByTestId('document-page-error').waitFor()
      await dshot(d, `${tag}-pdf-page-error`)
      await d.evaluate(() => (window.__failRender = false))
      // links: the display asks before it opens the page (a display dialog); a media link; a media link to a local address
      await p2.getByTestId('presenting-stop').click()
      const sendLink = async (url) => {
        await p2.getByTestId('what-link').click()
        await p2.getByTestId('link-url').fill(url)
        await p2.getByTestId('link-go').click()
        await d.getByTestId('link-card').waitFor()
        await armed(d)
      }
      await sendLink('https://example.org/a/path?with=query')
      await dshot(d, `${tag}-linkcard`)
      // the dialog covers the controls: name tag and End are dimmed and a click on End does nothing
      await d.getByTestId('end').click({ force: true, trial: false, timeout: 1500 }).catch(() => undefined)
      assert.ok(await d.getByTestId('link-card').isVisible(), 'a click on End while the link card is open must do nothing')
      const addrColors = await d.getByTestId('link-url').evaluate((e) => [getComputedStyle(e).color, getComputedStyle(e.closest('.cd')).getPropertyValue('--mut').trim()])
      console.log(`${tag}: link address colour ${addrColors[0]}, --mut ${addrColors[1]}`)
      await d.getByTestId('link-dismiss').click()
      await p2.getByTestId('presenting-stop').click()
      await sendLink('https://media.example.org/clip.mp4')
      await dshot(d, `${tag}-linkcard-media`)
      await d.getByTestId('link-dismiss').click()
      await p2.getByTestId('presenting-stop').click()
      await sendLink('http://192.168.1.20/clip.mp4')
      await dshot(d, `${tag}-linkcard-media-local`)
      await d.getByTestId('link-dismiss').click()
      await closeAll(c, pc1, pc2)
      // the relay of a link has to be confirmed first (a dialog card on the canvas, nothing else)
      {
        const c4 = await mkctx('light', { setup: true })
        const dr = await c4.newPage()
        await dr.goto(`${s.base}/display?server=${encodeURIComponent('wss://crafted.example.org')}`)
        await dr.getByTestId('relay-confirm').waitFor()
        await dr.waitForTimeout(200)
        await dshot(dr, `${tag}-relay-confirm`)
        await c4.close()
      }
      // a long file name and the longest sender name: pill, name tag and End side by side without touching
      {
        const c6 = await mkctx()
        const d6 = await c6.newPage()
        await d6.goto(`${s.base}/display`)
        await runWizard(d6, { mode: 'open' })
        await d6.getByTestId('room-code').waitFor()
        const code6 = (await d6.getByTestId('room-code').innerText()).replace(/\s+/g, '')
        const pc6 = await newContext(browser, { colorScheme: 'light' })
        const p6 = await pc6.newPage()
        await presenterStart(p6, s.base, code6, { name: 'Maximilian-Alexander' })
        await p6.getByTestId('what-page').waitFor({ timeout: 20000 })
        await p6.getByTestId('what-screen').click()
        await p6.getByTestId('presenting-page').waitFor({ timeout: 20000 })
        await d6.waitForFunction(() => document.querySelector('[data-testid=remote-video]')?.videoWidth > 0)
        await d6.keyboard.press('Shift') // reveals the controls
        await d6.waitForTimeout(500)
        await d6.evaluate(() => {
          window.__ch.onmessage({ data: JSON.stringify({ kind: 'image', name: 'a-very-long-file-name-from-the-holiday-in-2026-final-version.png', mime: 'image/png', size: 4000000 }) })
          window.__ch.onmessage({ data: new ArrayBuffer(16384) })
        })
        await d6.getByTestId('transfer').waitFor()
        await d6.waitForTimeout(300)
        await dshot(d6, `${tag}-transfer-long`)
        await closeAll(c6, pc6)
      }
      // dimmed (no key: a key press only restores the brightness)
      {
        const c2 = await mkctx()
        const dd = await c2.newPage()
        await dd.goto(`${sdim.base}/display`)
        await runWizard(dd, { mode: 'open' })
        await dd.getByTestId('room-code').waitFor()
        await dd.waitForTimeout(4200)
        await shot(dd, `${tag}-idle-dimmed`)
        await c2.close()
      }
      // a refusal of the server and a page that is out of date: the box at once, with its own body
      for (const [name, opts] of [
        ['offline-busy', { handler: (m, send) => m.type === 'create_room' && send({ type: 'error', code: 'rate_limited', message: '' }) }],
        ['offline-outdated', { protoVersion: 99, handler: () => undefined }],
      ]) {
        const c5 = await mkctx('light', { setup: true })
        const dx = await c5.newPage()
        opts.handler.protoVersion = opts.protoVersion
        await fakeRelay(dx, opts.handler)
        await dx.goto(`${s.base}/display`)
        await dx.getByTestId('offline').waitFor({ timeout: 10000 })
        await dx.waitForTimeout(300)
        await dshot(dx, `${tag}-idle-${name}`)
        await c5.close()
      }
      // offline: a server that goes away
      {
        const sx = await server()
        const c3 = await mkctx()
        const dx = await c3.newPage()
        await dx.goto(`${sx.base}/display`)
        await runWizard(dx, { mode: 'open' })
        await dx.getByTestId('room-code').waitFor()
        sx.proc.kill('SIGTERM')
        // right after the drop: "connecting" (empty code line and QR box); the box when the retry at 3 s has failed
        await dx.locator('[data-testid=qr][data-empty=true]').waitFor({ timeout: 5000 })
        await dshot(dx, `${tag}-idle-connecting`)
        await dx.getByTestId('offline').waitFor({ timeout: 20000 })
        await dx.waitForTimeout(300)
        await dshot(dx, `${tag}-idle-offline`)
        await c3.close()
      }
    }
  }

  // ---------------- presenter: light and dark, two viewports ----------------
  if (only === 'display') {
    console.log(`screenshots written to ${out}`)
    await browser.close()
    for (const sv of servers) sv.proc.kill('SIGTERM')
    rmSync(tmp, { recursive: true, force: true })
    process.exit(0)
  }
  const d0c = await newContext(browser)
  const d0 = await d0c.newPage()
  await d0.goto(`${s.base}/display`)
  await runWizard(d0, { mode: 'open' })
  await d0.getByTestId('room-code').waitFor()
  const open = (await d0.getByTestId('room-code').innerText()).replace(/\s+/g, '')
  const PH_UA = 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36'
  for (const [vname, viewport, extra] of VIEWS) {
    for (const scheme of ['light', 'dark']) {
      const tag = `presenter-${vname}-${scheme}`
      const ua = vname === 'phone' ? { userAgent: PH_UA } : {}
      const mk = () => newContext(browser, { viewport, ...extra, ...ua, colorScheme: scheme })
      let c = await mk()
      let p = await c.newPage()
      await p.goto(`${s.base}/`)
      await p.getByTestId('start-page').waitFor()
      await shot(p, `${tag}-start-typed`)
      await p.keyboard.press('Tab')
      await shot(p, `${tag}-focus-start`)
      await p.getByTestId('start-browser').click()
      await shot(p, `${tag}-start-empty-error`)
      await p.goto(`${s.base}/${open}`)
      await shot(p, `${tag}-start-qr`)
      await p.getByTestId('start-browser').click()
      await p.getByTestId('what-page').waitFor({ timeout: 20000 })
      await shot(p, `${tag}-what`)
      await p.getByTestId('what-file').click()
      await shot(p, `${tag}-file`)
      await p.getByTestId('file-back').click()
      // a shown PDF: "Page n of m" from the display, Previous disabled on page 1, the row right above Stop
      await p.getByTestId('what-file').click()
      await p.getByTestId('file-input').setInputFiles({ name: 'slides.pdf', mimeType: 'application/pdf', buffer: makePdf() })
      await p.getByTestId('presenting-pageof').waitFor({ timeout: 20000 })
      await shot(p, `${tag}-presenting-pdf`)
      await p.getByTestId('presenting-stop').click()
      await p.getByTestId('what-link').click()
      await shot(p, `${tag}-link`)
      await p.getByTestId('link-url').fill('https://example.org/x')
      await p.getByTestId('link-go').click()
      await p.getByTestId('presenting-page').waitFor()
      await shot(p, `${tag}-presenting-link`)
      await p.getByTestId('presenting-stop').click()
      if (vname === '1280') {
        await p.getByTestId('what-screen').click()
        await p.getByTestId('presenting-page').waitFor({ timeout: 20000 })
        await shot(p, `${tag}-presenting-screen`)
        await p.getByTestId('presenting-stop').click()
      } else {
        await p.getByTestId('what-screen').click()
        await shot(p, `${tag}-error-noscreen`)
        await p.getByTestId('error-showLink').click()
        await p.getByTestId('link-back').click()
      }
      // a second presenter in a room that has one: "Someone else is connected"
      {
        const c2 = await mk()
        const p3 = await c2.newPage()
        await presenterStart(p3, s.base, open, { name: 'Second' })
        await p3.getByTestId('error-page').waitFor({ timeout: 20000 })
        await shot(p3, `${tag}-error-occupied`)
        await c2.close()
      }
      await p.getByTestId('disconnect').click()
      await shot(p, `${tag}-bye`)
      await c.close()
      // errors
      c = await mk()
      p = await c.newPage()
      await presenterStart(p, s.base, 'ZZZZZZ', { name: 'x' })
      await p.getByTestId('error-page').waitFor()
      await shot(p, `${tag}-error-code`)
      await c.close()
      // a page that does not match the server: "This page is out of date" (the server's hello is faked)
      c = await mk()
      p = await c.newPage()
      const old = (m, send) => undefined
      old.protoVersion = 99
      await fakeRelay(p, old)
      await presenterStart(p, s.base, open, { name: 'x' })
      await p.getByTestId('error-page').waitFor()
      await shot(p, `${tag}-error-outdated`)
      await c.close()
      // failures that a relay that is not there can tell: expired, timeout, ended, no internet, locked while waiting, other
      const joinFake = (name, onJoin) => async () => {
        const cx = await mk()
        const px = await cx.newPage()
        await fakeRelay(px, (m, send, ws) => m.type === 'join_room' && onJoin(send, ws))
        await presenterStart(px, s.base, 'ABC234', { name: 'x' })
        await px.getByTestId('error-page').waitFor({ timeout: 15000 })
        await shot(px, `${tag}-error-${name}`)
        await cx.close()
      }
      const pending = (send) => send({ type: 'join_pending', check: 47, timeout_secs: 60, room_name: 'Meeting room Elbe' })
      await joinFake('expired', (send) => (pending(send), setTimeout(() => send({ type: 'error', code: 'room_not_found', message: '' }), 400)))()
      await joinFake('timeout', (send) => (pending(send), setTimeout(() => send({ type: 'error', code: 'approval_timeout', message: '' }), 400)))()
      await joinFake('locked-waiting', (send) => (pending(send), setTimeout(() => send({ type: 'error', code: 'room_locked', message: '' }), 400)))()
      await joinFake('ended', (send) => (send({ type: 'joined', room_name: 'Meeting room Elbe' }), setTimeout(() => send({ type: 'peer_left' }), 600)))()
      await joinFake('other', (send) => send({ type: 'error', code: 'forbidden', message: '' }))()
      await joinFake('noInternet', (send, ws) => ws.close({ code: 1006 }))()
      // the display does not answer: the answer is garbage, so the connection fails; the list of STUN/TURN servers decides the "why"
      for (const [name, status_] of [['displayOff', 404], ['displayOff-relay', 500], ['busy-ice', 429]]) {
        const cx = await mk()
        const px = await cx.newPage()
        await px.route('**/ice-servers', (r) => r.fulfill({ status: status_, body: '' }))
        await fakeRelay(px, (m, send) => {
          if (m.type === 'join_room') send({ type: 'joined', room_name: 'Meeting room Elbe' })
          else if (m.type === 'offer') send({ type: 'answer', sdp: 'garbage' })
        })
        await presenterStart(px, s.base, 'ABC234', { name: 'x' })
        await px.getByTestId('error-page').waitFor({ timeout: 15000 })
        await shot(px, `${tag}-error-${name}`)
        await cx.close()
      }
      // the relay of a link has to be confirmed first: its own page, then the start page with the code chip
      c = await mk()
      p = await c.newPage()
      await p.goto(`${s.base}/7UZFHX?server=${encodeURIComponent('wss://crafted.example.org')}`)
      await p.getByTestId('relay-confirm').waitFor()
      await shot(p, `${tag}-relay-confirm`)
      await p.getByTestId('relay-confirm-cancel').click()
      await p.getByTestId('start-code-chip').waitFor()
      await shot(p, `${tag}-relay-cancelled-start`)
      await c.close()
    }
  }
  // a server that is too busy ends in "The server is busy" (a ban is told like a rate limit); the next attempt, after the
  // server closed the connection, must still say busy, not "No internet" (60 s memory). A fresh server per variant.
  for (const [vname, viewport, extra] of VIEWS) {
    for (const scheme of ['light', 'dark']) {
      const sb = await server({ args: ['--mode', 'open', '--set', 'limits.ban_after_failed_joins=2', '--set', 'limits.join_attempts_per_min=1000'] })
      const c = await newContext(browser, { viewport, ...extra, colorScheme: scheme })
      const p = await c.newPage()
      await presenterStart(p, sb.base, 'ZZZZZZ', { name: 'x' })
      for (let i = 0; i < 6; i++) {
        await p.getByTestId('error-page').waitFor({ timeout: 20000 })
        const title = await p.getByTestId('error-title').innerText()
        if (/busy/i.test(title)) break
        await p.getByTestId('error-retype').click()
        await p.getByTestId('start-code').fill('ZZZZZZ')
        await p.getByTestId('start-browser').click()
        await p.waitForTimeout(200)
      }
      assert.match(await p.getByTestId('error-title').innerText(), /busy/i, 'the ban is told as busy')
      await p.getByTestId('error-retry').click() // the second attempt: the server closes the connection
      await p.waitForTimeout(800)
      await p.getByTestId('error-page').waitFor()
      assert.match(await p.getByTestId('error-title').innerText(), /busy/i, 'the second attempt still says busy')
      await shot(p, `presenter-${vname}-${scheme}-error-busy`)
      await c.close()
    }
  }
  // approval flow pages: waiting, denied, locked
  {
    const sa = await server({ args: ['--set', 'join_approval_secs=20'] })
    const dc = await newContext(browser)
    const dp = await dc.newPage()
    await dp.goto(`${sa.base}/display`)
    await runWizard(dp, { mode: 'confirm' })
    await dp.getByTestId('room-code').waitFor()
    const code = (await dp.getByTestId('room-code').innerText()).replace(/\s+/g, '')
    for (const [vname, viewport, extra] of VIEWS) {
      for (const scheme of ['light', 'dark']) {
        const tag = `presenter-${vname}-${scheme}`
        const c = await newContext(browser, { viewport, ...extra, colorScheme: scheme })
        const p = await c.newPage()
        await presenterStart(p, sa.base, code, { name: 'Lena' })
        await p.getByTestId('waiting-page').waitFor()
        await dp.getByTestId('request-modal').waitFor()
        await p.waitForTimeout(1500)
        await shot(p, `${tag}-waiting`)
        await armed(dp)
        await dp.getByTestId('request-no').click()
        await p.getByTestId('error-page').waitFor()
        await shot(p, `${tag}-error-denied`)
        await c.close()
      }
    }
    await dp.getByTestId('settings-open').click()
    await dp.getByTestId('settings-mode-locked').click()
    await dp.getByTestId('settings-close').click()
    for (const [vname, viewport, extra] of VIEWS) {
      for (const scheme of ['light', 'dark']) {
        const c = await newContext(browser, { viewport, ...extra, colorScheme: scheme })
        const p = await c.newPage()
        await presenterStart(p, sa.base, code, { name: 'Lena' })
        await p.getByTestId('error-page').waitFor()
        await shot(p, `presenter-${vname}-${scheme}-error-locked`)
        await c.close()
      }
    }
    await dc.close()
  }
  console.log(`screenshots written to ${out}`)
} finally {
  await browser?.close()
  for (const s of servers) s.proc.kill('SIGTERM')
  rmSync(tmp, { recursive: true, force: true })
}
