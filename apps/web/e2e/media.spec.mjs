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
import { contentSender, openDisplay, presenterJoin, status } from './helpers.mjs'

const en = JSON.parse(readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), '../src/locales/en.json'), 'utf8')).receive.media

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
const hits = [] // { path, cookie, referer, origin }
const sockets = new Set()
const mediaServer = createServer((req, res) => {
  const path = new URL(req.url, 'http://x').pathname
  hits.push({ path, cookie: req.headers.cookie ?? null, referer: req.headers.referer ?? null, origin: req.headers.origin ?? null })
  if (path === '/slow.mp4') return // never answers: the loading timeout must release the element
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
mediaServer.on('connection', (s) => {
  sockets.add(s)
  s.on('close', () => sockets.delete(s))
})
await new Promise((r) => mediaServer.listen(0, '127.0.0.1', r))
const mport = mediaServer.address().port
const mhost = `127.0.0.1:${mport}`
const media = (p) => `http://${mhost}${p}`
const hitsFor = (p) => hits.filter((h) => h.path === p).length
/** The notice text the person should read for a key, for this media host. */
const notice = (key) => en.error[key].replaceAll('{host}', mhost)
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

// A tiny PNG and a tiny one-page PDF to replace a playing player with other content.
const png1 = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==', 'base64')
function tinyPdf() {
  const objs = ['<< /Type /Catalog /Pages 2 0 R >>', '<< /Type /Pages /Kids [3 0 R] /Count 1 >>', '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>']
  let out = '%PDF-1.4\n'
  const offs = []
  objs.forEach((o, i) => {
    offs.push(out.length)
    out += `${i + 1} 0 obj\n${o}\nendobj\n`
  })
  const x = out.length
  out += `xref\n0 4\n0000000000 65535 f \n${offs.map((o) => `${String(o).padStart(10, '0')} 00000 n \n`).join('')}trailer\n<< /Root 1 0 R /Size 4 >>\nstartxref\n${x}\n%%EOF\n`
  return Buffer.from(out, 'latin1')
}

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
  // A cookie for the media host sits in the receiver's browser. A credentialed request would send it; the
  // player's anonymous request must not (control request below proves the cookie would otherwise be sent).
  await rctx.addCookies([{ name: 'session', value: 'secret', url: `http://${mhost}/` }])
  const sctx = await mk()
  const receiver = await rctx.newPage()
  const code = await openDisplay(receiver, base)
  const cs = await sctx.newPage()
  await presenterJoin(cs, base, code)
  await status(receiver, 'receiving')
  const content = contentSender(cs)
  const sendLink = (u) => content.link(u)
  const card = receiver.getByTestId('link-card')
  // The link card ignores input for 1 s after it appeared (the same guard as the join request).
  const shown = async () => {
    await card.waitFor({ timeout: 10000 })
    await sleep(1200)
  }
  const player = receiver.getByTestId('media-player')
  const elCount = () => receiver.evaluate(() => document.querySelectorAll('[data-testid=media-element]').length)

  // ---- audio: nothing loads before the click -------------------------------------------------------
  await sendLink(media('/a.wav'))
  await shown()
  const play = receiver.getByTestId('media-play')
  assert.equal((await play.innerText()).trim(), 'Play', 'the button says the word only (decision 13.60)')
  assert.equal(await play.getAttribute('title'), mhost)
  assert.match(await receiver.getByTestId('media-host-warning').innerText(), /Warning/, '127.0.0.1 is an IP literal: warning line')
  await new Promise((r) => setTimeout(r, 1500))
  assert.equal(hits.length, 0, 'no request to the media host before the click')
  assert.equal(await elCount(), 0, 'no media element before the click')
  // Dismiss: still nothing
  await receiver.getByTestId('link-dismiss').click()
  await card.waitFor({ state: 'detached' })
  assert.equal(hits.length, 0)

  // ---- click Play: the audio runs; request carries no cookie and no referrer -----------------------
  await sendLink(media('/a.wav'))
  await shown()
  await play.click()
  await player.waitFor()
  await waitFor(() => receiver.getByTestId('media-element').evaluate((m) => !m.paused && m.currentTime > 0), 'audio playing')
  assert.ok(hitsFor('/a.wav') > 0)
  // Control: a credentialed request to the same host DOES carry the cookie, so the absence below is meaningful.
  await receiver.evaluate((u) => fetch(u, { mode: 'no-cors', credentials: 'include' }).then(() => undefined, () => undefined), media('/ctl.wav'))
  const ctl = await waitFor(() => hits.find((h) => h.path === '/ctl.wav'), 'control request')
  assert.match(ctl.cookie ?? '', /session=secret/, 'control: a credentialed request sends the cookie')
  const playerHits = hits.filter((h) => h.path !== '/ctl.wav')
  assert.ok(playerHits.length > 0)
  assert.ok(playerHits.every((h) => h.cookie === null), 'no cookie sent by the player')
  assert.ok(playerHits.every((h) => h.referer === null), 'no referrer sent (meta referrer, no referrerpolicy attribute)')
  assert.ok(playerHits.every((h) => h.origin === base), `Origin is the app origin: ${playerHits.map((h) => h.origin)}`)
  assert.equal(await receiver.getByTestId('media-element').evaluate((m) => m.hasAttribute('referrerpolicy')), false)
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
  await sleep(500)
  const afterStop = hitsFor('/a.wav')
  await sleep(2500)
  assert.equal(hitsFor('/a.wav'), afterStop, 'no new requests after Stop (connection closed)')

  // ---- Escape inside the player closes it, like Stop ------------------------------------------------------
  await sendLink(media('/a.wav'))
  await shown()
  await play.click()
  await player.waitFor()
  await receiver.keyboard.press('Escape')
  await player.waitFor({ state: 'detached' })
  assert.equal(await elCount(), 0, 'Escape releases the element')
  await card.waitFor({ state: 'detached' })

  // ---- video (webm) ------------------------------------------------------------------------------
  if (webm) {
    await sendLink(media('/v.webm'))
    await shown()
    assert.equal(hitsFor('/v.webm'), 0)
    await play.click()
    await waitFor(() => receiver.getByTestId('media-element').evaluate((m) => !m.paused && m.videoWidth === 160), 'video playing')
    // Sender "stop showing" (cmd stop) closes the player and releases the element
    await content.stop()
    await player.waitFor({ state: 'detached' })
    assert.equal(await elCount(), 0)
  }

  // ---- one player at a time: a new link, an image or a document replaces a playing one ----------------------
  const startPlaying = async (path) => {
    await sendLink(media(path))
    await shown()
    await play.click()
    await player.waitFor()
    await waitFor(() => receiver.getByTestId('media-element').evaluate((m) => !m.paused && m.currentTime > 0), 'audio playing')
  }
  const expectQuiet = async (path, what) => {
    await sleep(500)
    const n = hitsFor(path)
    await sleep(2000)
    assert.equal(hitsFor(path), n, `${what}: no new requests`)
  }
  await startPlaying('/r1.wav')
  await sendLink(media('/a.wav?second=1'))
  await shown()
  assert.equal(await elCount(), 0, 'new link closes the player')
  assert.equal(await player.count(), 0)
  await expectQuiet('/r1.wav', 'after replacement by a link')
  await receiver.getByTestId('link-dismiss').click()

  await startPlaying('/r2.wav')
  await content.file({ name: 't.png', mimeType: 'image/png', buffer: png1 })
  await receiver.getByTestId('received-image').waitFor({ timeout: 10000 })
  assert.equal(await elCount(), 0, 'an image closes the player')
  assert.equal(await player.count(), 0)
  await expectQuiet('/r2.wav', 'after replacement by an image')
  await content.stop() // no close button on the display: the presenter stops it

  await startPlaying('/r3.wav')
  await content.file({ name: 't.pdf', mimeType: 'application/pdf', buffer: tinyPdf() })
  await player.waitFor({ state: 'detached', timeout: 10000 })
  assert.equal(await elCount(), 0, 'a document closes the player')
  await expectQuiet('/r3.wav', 'after replacement by a document')
  await content.stop()

  // ---- errors: translated, host only. Chrome reports a bad file, a 404 and a missing CORS header all as ----
  // MediaError code 4, so all three show the combined "cannot be played here" notice (UNCERTAIN for others).
  const err = receiver.getByTestId('media-error')
  for (const [path, label] of [['/bad.mp4?secret=abc', 'bad file'], ['/missing.mp4?secret=abc', '404'], ['/nocors.wav?secret=abc', 'no CORS']]) {
    await sendLink(media(path))
    await shown()
    await play.click()
    await err.waitFor({ timeout: 15000 })
    const text = (await err.innerText()).replace(/\s+/g, ' ')
    assert.equal(text, notice('unsupported'), `${label}: notice text`)
    assert.ok(text.includes(mhost), text)
    assert.ok(!/secret|bad\.mp4|missing\.mp4|nocors/.test(text), 'no URL details in the notice')
    assert.equal(await elCount(), 0, `${label}: failed player is released`)
    await receiver.getByTestId('link-dismiss').click()
  }

  // ---- loading timeout (20 s): a server that never answers releases the element, network notice ---------
  await sendLink(media('/slow.mp4'))
  await shown()
  await play.click()
  await player.waitFor()
  await err.waitFor({ timeout: 30000 })
  assert.equal((await err.innerText()).replace(/\s+/g, ' '), notice('network'))
  assert.equal(await elCount(), 0, 'timed out player is released')
  await receiver.getByTestId('link-dismiss').click()

  // ---- HLS: native support only (Chromium has none) -----------------------------------------------------
  const hlsNative = await receiver.evaluate(() => document.createElement('video').canPlayType('application/vnd.apple.mpegurl') !== '')
  await sendLink(media('/live/index.m3u8'))
  await shown()
  assert.equal(await receiver.getByTestId('media-play').count(), hlsNative ? 1 : 0)
  if (!hlsNative) await receiver.getByTestId('media-hls-unsupported').waitFor()
  await receiver.getByTestId('link-open').waitFor() // the plain Open choice stays
  await receiver.getByTestId('link-dismiss').click()

  // ---- a normal link has no Play button ---------------------------------------------------------------
  await sendLink('https://example.org/page.html')
  await shown()
  assert.equal(await receiver.getByTestId('media-play').count(), 0)
  await receiver.getByTestId('link-dismiss').click()

  // ---- the sender leaves while playing: the player closes ------------------------------------------------
  await startPlaying('/r4.wav')
  await cs.close()
  await player.waitFor({ state: 'detached', timeout: 15000 })
  assert.equal(await elCount(), 0, 'peer left releases the element')
  await expectQuiet('/r4.wav', 'after the sender left')

  console.log('E2E media OK', webm ? '(audio + video)' : '(audio only, video skipped)', `HLS native: ${hlsNative}`)
} finally {
  await browser?.close()
  server.kill('SIGTERM')
  for (const s of sockets) s.destroy()
  mediaServer.close()
  rmSync(tmp, { recursive: true, force: true })
}
