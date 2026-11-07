// SPDX-License-Identifier: GPL-3.0-or-later
//
// Real screen content from the native sender to the browser receiver:
//   Xvfb + a coloured xterm window  ->  `cometcast-cli send --screen` (X11 capture, H.264)
//   ->  cometcast-server  ->  web app in Chromium (receiver) which must decode the H.264 stream.
// The decoded picture is drawn on a canvas and its colour is compared with the window colour,
// and a second window of another colour must show up after a while.
//
// If the Chromium used here cannot decode H.264 (checked with
// RTCRtpReceiver.getCapabilities), the test says so and ends with "SKIPPED" instead of OK.
// If it claims to support H.264 but nothing is decoded, the test FAILS.
//
// Run: npm run build && npm run e2e:screen
// Needs: a Rust toolchain, Xvfb and xterm (apt-get install xvfb xterm), the playwright chromium.
import { createRequire } from 'node:module'
import { spawn, spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'
import assert from 'node:assert/strict'
import { openDisplay } from './helpers.mjs'
import { createServer } from 'node:net'

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')

const here = dirname(fileURLToPath(import.meta.url))
const repo = resolve(here, '../../..')
const webDir = resolve(here, '../dist')
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
const targetDir = process.env.CARGO_TARGET_DIR ?? resolve(repo, 'target')

const have = (cmd) => spawnSync('sh', ['-c', `command -v ${cmd}`]).status === 0
if (!have('Xvfb') || !have('xterm')) {
  console.log('E2E SCREEN SKIPPED: Xvfb and/or xterm are not installed (apt-get install xvfb xterm).')
  process.exit(0)
}

const build = spawnSync('cargo', ['build', '-p', 'cometcast-server', '-p', 'cometcast-cli'], {
  cwd: repo,
  stdio: 'inherit',
})
assert.equal(build.status, 0, 'cargo build failed')

const children = []
let browser
let server

/** Starts Xvfb on a free display; resolves to the display name such as ':101'. */
function startXvfb(size) {
  return new Promise((res, rej) => {
    // -displayfd 1: Xvfb picks a free display number and prints it to stdout.
    const x = spawn('Xvfb', ['-displayfd', '1', '-screen', '0', `${size}x24`, '-nolisten', 'tcp'], {
      stdio: ['ignore', 'pipe', 'ignore'],
    })
    children.push(x)
    x.once('error', rej)
    x.stdout.once('data', (d) => res(`:${d.toString().trim()}`))
    x.once('exit', () => rej(new Error('Xvfb exited early')))
  })
}

/** An xterm much larger than the screen: it covers the whole root window in one colour. */
function startWindow(display, color) {
  const p = spawn('xterm', ['-display', display, '-bg', color, '-fg', color, '-geometry', '200x80+0+0', '-e', 'sleep', '300'], {
    stdio: 'ignore',
  })
  children.push(p)
  return p
}

function startCli(args, env = {}) {
  const proc = spawn(resolve(targetDir, 'debug/cometcast-cli'), args, {
    stdio: ['ignore', 'pipe', 'inherit'],
    env: { ...process.env, ...env },
  })
  const lines = []
  let buf = ''
  proc.stdout.on('data', (d) => {
    buf += d.toString()
    let i
    while ((i = buf.indexOf('\n')) >= 0) {
      const line = buf.slice(0, i)
      buf = buf.slice(i + 1)
      lines.push(line)
      console.log('  [cli]', line)
    }
  })
  const exited = new Promise((res) => proc.on('exit', (code) => res(code)))
  children.push(proc)
  return { proc, lines, exited }
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

/** Mean RGB of the current video picture, drawn on a canvas. */
const meanColour = (video) =>
  video.evaluate((v) => {
    const c = document.createElement('canvas')
    c.width = 64
    c.height = 48
    const ctx = c.getContext('2d')
    ctx.drawImage(v, 0, 0, c.width, c.height)
    const d = ctx.getImageData(0, 0, c.width, c.height).data
    const sum = [0, 0, 0]
    for (let i = 0; i < d.length; i += 4) {
      sum[0] += d[i]
      sum[1] += d[i + 1]
      sum[2] += d[i + 2]
    }
    const n = d.length / 4
    return sum.map((s) => Math.round(s / n))
  })

const close = (got, want, tol) => got.every((g, i) => Math.abs(g - want[i]) <= tol)

try {
  server = spawn(resolve(targetDir, 'debug/cometcast-server'), ['--listen', `127.0.0.1:${port}`, '--web-dir', webDir], {
    stdio: 'inherit',
  })
  children.push(server)
  await waitHealthy()
  browser = await chromium.launch({
    args: [
      // Without this Chromium hides host IPs behind mDNS names that the Rust side cannot resolve.
      '--disable-features=WebRtcHideLocalIpsWithMdns',
    ],
  })
  const ctx = await browser.newContext({ locale: 'en-US', viewport: { width: 1280, height: 800 } })
  const page = await ctx.newPage()
  page.on('pageerror', (e) => console.error('pageerror:', e.message))
  await page.goto(base)

  // Can this Chromium decode H.264 at all?
  const caps = await page.evaluate(() =>
    RTCRtpReceiver.getCapabilities('video').codecs.filter((c) => /h264/i.test(c.mimeType)).map((c) => c.sdpFmtpLine ?? ''),
  )
  console.log(`Chromium ${browser.version()} H.264 receive capabilities:`, caps.length ? caps : 'none')
  if (caps.length === 0) {
    console.log(
      'E2E SCREEN SKIPPED: this Chromium lists no H.264 in RTCRtpReceiver.getCapabilities(), so it cannot receive the native screen stream. Use a Chrome build with proprietary codecs to run this test.',
    )
    process.exit(0)
  }
  const mode1 = caps.some((f) => /packetization-mode=1/.test(f))
  console.log('packetization-mode=1 offered by the browser:', mode1)

  // Screen to share: 640x480, whole screen red.
  const display = await startXvfb('640x480')
  startWindow(display, '#c81e28')
  await new Promise((r) => setTimeout(r, 1500))

  // Browser receives; native sender sends the screen.
  const code = await openDisplay(page, base)
  const tx = startCli(['send', code, '--server', `127.0.0.1:${port}`, '--screen', '--display', display, '--fps', '15', '--seconds', '25'])
  await page.locator('[data-testid=status][data-phase=receiving]').waitFor({ timeout: 20000 })
  const video = page.getByTestId('remote-video')
  try {
    await waitFor(() => video.evaluate((v) => v.videoWidth > 0), 'videoWidth > 0', 15000)
  } catch (e) {
    throw new Error(
      `Chromium lists H.264 but decoded no picture of the native stream (${e.message}). CLI output: ${tx.lines.join(' | ')}`,
    )
  }
  const dims = await video.evaluate((v) => [v.videoWidth, v.videoHeight])
  console.log('browser decodes the H.264 screen stream, size:', dims)
  assert.deepEqual(dims, [640, 480], 'screen size')

  // The picture has the colour of the first window (tolerance: colour matrix differences).
  const red = [200, 30, 40]
  const first = await waitFor(async () => {
    const m = await meanColour(video)
    return close(m, red, 40) ? m : null
  }, 'picture near red')
  console.log('mean colour of the received picture (expected about', red, '):', first)

  // A second window in another colour covers the first one; the browser must follow.
  const blue = [20, 80, 200]
  startWindow(display, '#1450c8')
  const second = await waitFor(async () => {
    const m = await meanColour(video)
    return close(m, blue, 40) ? m : null
  }, 'picture near blue')
  console.log('mean colour after the change (expected about', blue, '):', second)

  // The video keeps being a live stream (frames still counted by the browser).
  const stats = await page.evaluate(async () => {
    // eslint-disable-next-line no-undef
    const v = document.querySelector('[data-testid=remote-video]')
    return { time: v.currentTime, w: v.videoWidth }
  })
  assert.ok(stats.w === 640)
  assert.ok(
    tx.lines.some((l) => /^sent frames=\d+ polls=\d+ changed=\d+ unchanged=\d+/.test(l)),
    'CLI printed screen statistics',
  )
  const last = tx.lines.filter((l) => l.startsWith('sent frames=')).at(-1)
  console.log('sender statistics:', last)
  assert.equal(await tx.exited, 0, 'CLI send exits 0 after its duration')
  console.log('E2E SCREEN OK')
} finally {
  for (const c of children) c.kill('SIGTERM')
  await browser?.close()
}
