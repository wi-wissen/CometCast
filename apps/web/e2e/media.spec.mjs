// SPDX-License-Identifier: GPL-3.0-or-later
//
// End-to-end test for media links: a sender sends an ordinary `link` to a .wav / .webm file, the
// receiver shows the card with a Play button and loads nothing until the click; Stop releases the element.
// Real server binary + real Chromium pages + a local media server (generated files, with and without CORS).
// Run: npm run build && E2E_CHROME=/usr/bin/google-chrome npm run e2e:media
// Needs ffmpeg for the .webm file; without it the video part is skipped (and said so).
import { createRequire } from 'node:module'
import { spawn, spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'
import { createServer } from 'node:http'
import { mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import assert from 'node:assert/strict'

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')

const here = dirname(fileURLToPath(import.meta.url))
const repo = resolve(here, '../../..')
const webDir = resolve(here, '../dist')
const port = Number(process.env.E2E_PORT ?? 19443 + Math.floor(Math.random() * 1000))
const base = `http://127.0.0.1:${port}`

// ---- test media -----------------------------------------------------------------------------------
function makeWav(seconds = 20, rate = 8000) {
  const n = seconds * rate
  const b = Buffer.alloc(44 + n)
  b.write('RIFF', 0)
  b.writeUInt32LE(36 + n, 4)
  b.write('WAVEfmt ', 8)
  b.writeUInt32LE(16, 16)
  b.writeUInt16LE(1, 20)
  b.writeUInt16LE(1, 22)
  b.writeUInt32LE(rate, 24)
  b.writeUInt32LE(rate, 28)
  b.writeUInt16LE(1, 32)
  b.writeUInt16LE(8, 34)
  b.write('data', 36)
  b.writeUInt32LE(n, 40)
  for (let i = 0; i < n; i++) b[44 + i] = 128 + Math.round(40 * Math.sin((i / rate) * 2 * Math.PI * 440))
  return b
}
const tmp = mkdtempSync(resolve(tmpdir(), 'cc-media-'))
const wav = makeWav()
let webm = null
{
  const out = resolve(tmp, 'v.webm')
  const r = spawnSync('ffmpeg', ['-v', 'error', '-f', 'lavfi', '-i', 'testsrc=size=160x90:rate=10', '-t', '20', '-c:v', 'libvpx', '-b:v', '50k', out])
  if (r.status === 0) webm = readFileSync(out)
  else console.log('SKIP video: ffmpeg could not make a .webm file')
}

/** Local media server on its own port (= its own origin, so media requests are cross-origin). */
const hits = [] // { path, cookie, referer }
const mediaServer = createServer((req, res) => {
  const path = new URL(req.url, 'http://x').pathname
  hits.push({ path, cookie: req.headers.cookie ?? null, referer: req.headers.referer ?? null })
  const cors = path.startsWith('/nocors') ? {} : { 'access-control-allow-origin': '*', 'access-control-expose-headers': 'content-range,content-length,accept-ranges' }
  const file = path.endsWith('.wav') ? wav : path.endsWith('.webm') ? webm : path.endsWith('bad.mp4') ? Buffer.from('this is not a video'.repeat(100)) : null
  if (!file) {
    res.writeHead(404, cors)
    return res.end()
  }
  const type = path.endsWith('.wav') ? 'audio/wav' : path.endsWith('.webm') ? 'video/webm' : 'video/mp4'
  const m = /bytes=(\d*)-(\d*)/.exec(req.headers.range ?? '')
  if (m) {
    const start = m[1] ? Number(m[1]) : 0
    const end = m[2] ? Math.min(Number(m[2]), file.length - 1) : file.length - 1
    res.writeHead(206, { ...cors, 'content-type': type, 'accept-ranges': 'bytes', 'content-range': `bytes ${start}-${end}/${file.length}`, 'content-length': end - start + 1, 'set-cookie': 'track=1' })
    return res.end(file.subarray(start, end + 1))
  }
  res.writeHead(200, { ...cors, 'content-type': type, 'accept-ranges': 'bytes', 'content-length': file.length, 'set-cookie': 'track=1' })
  res.end(file)
})
await new Promise((r) => mediaServer.listen(0, '127.0.0.1', r))
const mport = mediaServer.address().port
const mhost = `127.0.0.1:${mport}`
const media = (p) => `http://${mhost}${p}`
const hitsFor = (p) => hits.filter((h) => h.path === p).length

function startServer() {
  const build = spawnSync('cargo', ['build', '-p', 'cometcast-server'], { cwd: repo, stdio: 'inherit' })
  assert.equal(build.status, 0, 'cargo build failed')
  const targetDir = process.env.CARGO_TARGET_DIR ?? resolve(repo, 'target')
  return spawn(resolve(targetDir, 'debug/cometcast-server'), ['--listen', `127.0.0.1:${port}`, '--web-dir', webDir], { stdio: 'inherit' })
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

const server = startServer()
let browser
try {
  await waitHealthy()
  browser = await chromium.launch({
    executablePath: process.env.E2E_CHROME || undefined,
    args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream', '--disable-features=WebRtcHideLocalIpsWithMdns'],
  })
  const mk = async () => {
    const ctx = await browser.newContext({ locale: 'en-US', viewport: { width: 1280, height: 800 } })
    ctx.on('page', (p) => p.on('pageerror', (e) => console.error('pageerror:', e.message)))
    return ctx
  }
  const rctx = await mk()
  const sctx = await mk()
  const receiver = await rctx.newPage()
  await receiver.goto(base)
  await receiver.getByRole('button', { name: /Receive on this device/ }).click()
  await receiver.locator('[data-testid=status][data-phase=waiting_sender]').waitFor({ timeout: 10000 })
  const code = (await receiver.getByTestId('room-code').innerText()).trim()
  const cs = await sctx.newPage()
  await cs.goto(base)
  await cs.getByRole('button', { name: /Send file or link/ }).click()
  await cs.locator('#room-code').fill(code)
  await cs.getByRole('button', { name: 'Connect' }).click()
  await cs.locator('[data-testid=status][data-ready=true]').waitFor({ timeout: 15000 })
  await receiver.locator('[data-testid=status][data-phase=receiving]').waitFor({ timeout: 15000 })

  const sendLink = async (u) => {
    await cs.locator('#link-url').fill(u)
    await cs.getByRole('button', { name: 'Send link' }).click()
  }
  const card = receiver.getByTestId('link-card')
  const player = receiver.getByTestId('media-player')
  const elCount = () => receiver.evaluate(() => document.querySelectorAll('[data-testid=media-element]').length)

  // ---- audio: nothing loads before the click -------------------------------------------------------
  await sendLink(media('/a.wav'))
  await card.waitFor({ timeout: 10000 })
  const play = receiver.getByTestId('media-play')
  assert.match(await play.innerText(), new RegExp(`Play from ${mhost.replace('.', '\\.')}`))
  await new Promise((r) => setTimeout(r, 1500))
  assert.equal(hits.length, 0, 'no request to the media host before the click')
  assert.equal(await elCount(), 0, 'no media element before the click')
  // Dismiss: still nothing
  await receiver.getByTestId('link-dismiss').click()
  await card.waitFor({ state: 'detached' })
  assert.equal(hits.length, 0)

  // ---- click Play: the audio runs; request carries no cookie and no referrer -----------------------
  await sendLink(media('/a.wav'))
  await card.waitFor({ timeout: 10000 })
  await play.click()
  await player.waitFor()
  await waitFor(() => receiver.getByTestId('media-element').evaluate((m) => !m.paused && m.currentTime > 0), 'audio playing')
  assert.ok(hitsFor('/a.wav') > 0)
  assert.ok(hits.every((h) => h.cookie === null), 'no cookie sent')
  assert.ok(hits.every((h) => h.referer === null), 'no referrer sent')
  assert.equal(await receiver.getByTestId('media-element').evaluate((m) => m.crossOrigin), 'anonymous')
  assert.equal(await receiver.getByTestId('media-element').evaluate((m) => m.controls), true)
  // Stop releases the element
  const handle = await receiver.getByTestId('media-element').elementHandle()
  await receiver.getByTestId('media-stop').click()
  await player.waitFor({ state: 'detached' })
  assert.equal(await elCount(), 0)
  assert.equal(await handle.evaluate((m) => m.paused), true, 'released element is paused')
  assert.equal(await handle.evaluate((m) => m.hasAttribute('src')), false)
  await card.waitFor({ state: 'detached' })

  // ---- video (webm) ------------------------------------------------------------------------------
  if (webm) {
    await sendLink(media('/v.webm'))
    await card.waitFor({ timeout: 10000 })
    assert.equal(hitsFor('/v.webm'), 0)
    await play.click()
    await waitFor(() => receiver.getByTestId('media-element').evaluate((m) => !m.paused && m.videoWidth === 160), 'video playing')
    // Sender "stop showing" (cmd stop) closes the player and releases the element
    await cs.getByTestId('stop-showing').click()
    await player.waitFor({ state: 'detached' })
    assert.equal(await elCount(), 0)
  }

  // ---- one player at a time: a new link replaces a playing one --------------------------------------
  await sendLink(media('/a.wav'))
  await card.waitFor({ timeout: 10000 })
  await play.click()
  await player.waitFor()
  await sendLink(media('/a.wav?second=1'))
  await card.waitFor({ timeout: 10000 })
  assert.equal(await elCount(), 0, 'new content closes the player')
  await receiver.getByTestId('link-dismiss').click()

  // ---- errors: translated, host only -----------------------------------------------------------------
  await sendLink(media('/bad.mp4?secret=abc'))
  await card.waitFor({ timeout: 10000 })
  await play.click()
  const err = receiver.getByTestId('media-error')
  await err.waitFor({ timeout: 15000 })
  const text = await err.innerText()
  assert.ok(text.includes(mhost), text)
  assert.ok(!/secret|bad\.mp4/.test(text), 'no URL details in the notice')
  assert.equal(await elCount(), 0, 'failed player is released')
  await receiver.getByTestId('link-dismiss').click()

  await sendLink(media('/nocors.wav'))
  await card.waitFor({ timeout: 10000 })
  await play.click()
  await err.waitFor({ timeout: 15000 })
  assert.ok(!/nocors/.test(await err.innerText()))
  await receiver.getByTestId('link-dismiss').click()

  // ---- HLS: native support only (Chromium has none) -----------------------------------------------------
  const hlsNative = await receiver.evaluate(() => document.createElement('video').canPlayType('application/vnd.apple.mpegurl') !== '')
  await sendLink(media('/live/index.m3u8'))
  await card.waitFor({ timeout: 10000 })
  assert.equal(await receiver.getByTestId('media-play').count(), hlsNative ? 1 : 0)
  if (!hlsNative) await receiver.getByTestId('media-hls-unsupported').waitFor()
  await receiver.getByTestId('link-open').waitFor() // the plain Open choice stays
  await receiver.getByTestId('link-dismiss').click()

  // ---- a normal link has no Play button ---------------------------------------------------------------
  await sendLink('https://example.org/page.html')
  await card.waitFor({ timeout: 10000 })
  assert.equal(await receiver.getByTestId('media-play').count(), 0)
  await receiver.getByTestId('link-dismiss').click()

  console.log('E2E media OK', webm ? '(audio + video)' : '(audio only, video skipped)', `HLS native: ${hlsNative}`)
} finally {
  await browser?.close()
  server.kill('SIGTERM')
  mediaServer.close()
  rmSync(tmp, { recursive: true, force: true })
}
