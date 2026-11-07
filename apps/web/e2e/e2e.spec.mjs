// SPDX-License-Identifier: GPL-3.0-or-later
//
// End-to-end test: real server binary + two real Chromium pages (receiver and sender).
// Run: npm run build && npm run e2e   (needs a Rust toolchain and the playwright chromium browser)
//
// By default the sender uses the real getDisplayMedia (it worked headless in the preinstalled
// Chromium with the launch flags below). Set E2E_TEST_VIDEO=1 to use the TEST-ONLY canvas stream
// (?testVideo=1) instead, in case getDisplayMedia cannot be granted in another environment.
import { createRequire } from 'node:module'
import { spawn, spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'
import assert from 'node:assert/strict'

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')

const here = dirname(fileURLToPath(import.meta.url))
const repo = resolve(here, '../../..')
const webDir = resolve(here, '../dist')
const port = Number(process.env.E2E_PORT ?? 18443 + Math.floor(Math.random() * 1000))
const base = `http://127.0.0.1:${port}`
const realDisplay = process.env.E2E_TEST_VIDEO !== '1'

function startServer() {
  const build = spawnSync('cargo', ['build', '-p', 'cometcast-server'], { cwd: repo, stdio: 'inherit' })
  assert.equal(build.status, 0, 'cargo build failed')
  const targetDir = process.env.CARGO_TARGET_DIR ?? resolve(repo, 'target')
  const proc = spawn(resolve(targetDir, 'debug/cometcast-server'), ['--listen', `127.0.0.1:${port}`, '--web-dir', webDir], {
    stdio: 'inherit',
  })
  return proc
}

async function waitHealthy() {
  for (let i = 0; i < 100; i++) {
    try {
      const r = await fetch(`${base}/healthz`)
      if (r.ok) return
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 100))
  }
  throw new Error('server did not become healthy')
}

async function waitFor(fn, what, ms = 15000) {
  const end = Date.now() + ms
  let last
  while (Date.now() < end) {
    try {
      last = await fn()
      if (last) return last
    } catch {
      /* retry */
    }
    await new Promise((r) => setTimeout(r, 150))
  }
  throw new Error(`timeout waiting for: ${what}`)
}

const server = startServer()
let browser
try {
  await waitHealthy()
  browser = await chromium.launch({
    args: [
      '--use-fake-ui-for-media-stream',
      '--auto-select-desktop-capture-source=Entire screen',
      '--use-fake-device-for-media-stream',
      // Without this Chromium hides host IPs behind mDNS names that may not resolve in a sandbox.
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

  // --- Receiver: create room, read the code
  const receiver = await rctx.newPage()
  await receiver.goto(base)
  await receiver.getByRole('button', { name: /Receive on this device/ }).click()
  await receiver.locator('[data-testid=status][data-phase=waiting_sender]').waitFor({ timeout: 10000 })
  const code = (await receiver.getByTestId('room-code').innerText()).trim()
  assert.match(code, /^[A-HJ-NP-Z2-9]{6}$/, 'room code format')
  console.log('room code:', code)

  // --- Sender: wrong code first (error mapped to translated text)
  const sender = await sctx.newPage()
  await sender.goto(realDisplay ? base : `${base}/?testVideo=1`)
  await sender.getByRole('button', { name: /Send screen/ }).click()
  await sender.locator('#room-code').fill('ZZZZZZ')
  await sender.getByRole('button', { name: 'Share screen' }).click()
  await sender.locator('[data-testid=status][data-phase=error]').waitFor({ timeout: 10000 })
  assert.match(await sender.getByTestId('status').innerText(), /No room with this code exists/)

  // --- Sender: correct code
  await sender.locator('#room-code').fill(code)
  await sender.getByRole('button', { name: 'Try again' }).click()
  await sender.locator('[data-testid=status][data-phase=sharing]').waitFor({ timeout: 15000 })
  await receiver.locator('[data-testid=status][data-phase=receiving]').waitFor({ timeout: 15000 })

  // --- Connection check code: both sides derive the same 6 digits from the DTLS fingerprints
  {
    const sCode = (await sender.getByTestId('sas-code').innerText()).trim()
    const rCode = (await receiver.getByTestId('sas-code').innerText()).trim()
    assert.match(sCode, /^\d{3} \d{3}$/, 'sender shows a connection check code')
    assert.equal(rCode, sCode, 'sender and receiver show the same connection check code')
    console.log('connection check code:', sCode)
  }

  // --- Receiver video plays
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

  // --- Stop sharing on the sender: receiver shows "peer left"
  await sender.getByTestId('stop').click()
  assert.equal(await sender.getByTestId('status').getAttribute('data-phase'), 'stopped')
  await receiver.locator('[data-testid=status][data-phase=peer_left]').waitFor({ timeout: 10000 })
  assert.match(await receiver.getByTestId('status').innerText(), /The sender left/)

  console.log('E2E OK')
} finally {
  await browser?.close()
  server.kill('SIGTERM')
}
