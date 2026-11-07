// SPDX-License-Identifier: GPL-3.0-or-later
//
// End-to-end test: real server binary + two real Chromium pages (display and presenter).
// Run: npm run build && npm run e2e   (needs a Rust toolchain and the playwright chromium browser)
//
// By default the presenter uses the real getDisplayMedia (it worked headless in the preinstalled
// Chromium with the launch flags in helpers.mjs). Set E2E_TEST_VIDEO=1 to use the TEST-ONLY canvas
// stream (?testVideo=1) instead, in case getDisplayMedia cannot be granted in another environment.
import assert from 'node:assert/strict'
import { launch, newContext, openDisplay, presenterStart, startServer, status, waitFor, waitHealthy } from './helpers.mjs'

const realDisplay = process.env.E2E_TEST_VIDEO !== '1'
const server = startServer()
const { base } = server
let browser
try {
  await waitHealthy(base)
  browser = await launch()
  const rctx = await newContext(browser, { viewport: { width: 1280, height: 800 } })
  const sctx = await newContext(browser, { viewport: { width: 1280, height: 800 } })

  // --- Display: set up, read the code
  const receiver = await rctx.newPage()
  const code = await openDisplay(receiver, base)
  assert.match(code, /^[A-HJ-NP-Z2-9]{6}$/, 'room code format')
  console.log('room code:', code)

  // --- Presenter: wrong code first (error page)
  const sender = await sctx.newPage()
  const start = realDisplay ? base : `${base}/?testVideo=1`
  await sender.goto(start)
  await sender.getByTestId('start-code').fill('ZZZZZZ')
  await sender.getByTestId('start-browser').click()
  await sender.getByTestId('error-page').waitFor({ timeout: 10000 })
  assert.match(await sender.getByTestId('error-title').innerText(), /This code does not exist right now/)

  // --- Presenter: correct code, then the screen
  await sender.getByTestId('error-retype').click()
  await sender.getByTestId('start-code').fill(code)
  await sender.getByTestId('start-browser').click()
  await sender.getByTestId('what-page').waitFor({ timeout: 15000 })
  await sender.getByTestId('what-screen').click()
  await sender.locator('[data-testid=status][data-phase=sharing]').waitFor({ timeout: 15000 })
  await status(receiver, 'receiving')

  // --- Connection check code: both sides derive the same 6 digits from the DTLS fingerprints
  {
    const sCode = (await sender.getByTestId('sas-code').innerText()).trim()
    const rCode = (await receiver.getByTestId('sas-code').innerText()).trim()
    assert.match(sCode, /^\d{3} \d{3}$/, 'the presenter shows a connection check code')
    assert.equal(rCode, sCode, 'presenter and display show the same connection check code')
    console.log('connection check code:', sCode)
  }

  // --- Display video plays
  const video = receiver.getByTestId('remote-video')
  await waitFor(() => video.evaluate((v) => v.videoWidth > 0), 'videoWidth > 0')
  const t0 = await video.evaluate((v) => v.currentTime)
  const t1 = await waitFor(
    async () => {
      const t = await video.evaluate((v) => v.currentTime)
      return t > t0 + 0.3 ? t : 0
    },
    'currentTime advancing',
  )
  const dims = await video.evaluate((v) => [v.videoWidth, v.videoHeight, v.paused])
  console.log('video:', dims, 'currentTime', t0, '->', t1)
  assert.equal(dims[2], false, 'video is not paused')

  // --- Stop showing: the presenter stays connected, the display goes back to idle with the same code
  await sender.getByTestId('presenting-stop').click()
  await sender.getByTestId('what-page').waitFor()
  await receiver.getByTestId('idle').waitFor({ state: 'visible', timeout: 10000 })
  assert.equal((await receiver.getByTestId('room-code').innerText()).replace(/\s+/g, ''), code)
  assert.equal(await receiver.locator('[data-testid=status]').getAttribute('data-phase'), 'receiving', 'still connected')
  // ... and can show the screen again on the same connection
  await sender.getByTestId('what-screen').click()
  await sender.getByTestId('presenting-page').waitFor({ timeout: 15000 })
  await waitFor(() => video.evaluate((v) => v.videoWidth > 0 && !v.paused), 'video plays again')
  await sender.getByTestId('presenting-stop').click()
  await sender.getByTestId('what-page').waitFor()

  // --- Disconnect: the display reports "peer left"
  await sender.getByTestId('disconnect').click()
  await sender.getByTestId('bye-page').waitFor()
  await status(receiver, 'peer_left', 10000)
  assert.match(await receiver.getByTestId('status').innerText(), /The sender left/)

  console.log('E2E OK')
} finally {
  await browser?.close()
  server.proc.kill('SIGTERM')
}
