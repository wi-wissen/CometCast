// SPDX-License-Identifier: GPL-3.0-or-later
//
// Interop test between the native Rust peers and the browser web app (milestone M1).
//   A) browser sender  ->  `cometcast-cli receive`   (Rust receiver must count RTP packets)
//   B) `cometcast-cli send`  ->  browser receiver    (Chromium must decode the Rust VP8 stream)
// Starts the real cometcast-server, the real CLI binary and a real Chromium.
// Run: npm run build && npm run e2e:native   (needs a Rust toolchain and the playwright chromium)
import { createRequire } from 'node:module'
import { spawn, spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'
import assert from 'node:assert/strict'
import { openDisplay, presenterScreen } from './helpers.mjs'
import { createServer } from 'node:net'

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')

const here = dirname(fileURLToPath(import.meta.url))
const repo = resolve(here, '../../..')
const webDir = resolve(here, '../dist')
// Ask the OS for a free port by binding port 0, then release it for the server.
// (Small race window between close and the server's bind; acceptable for a local test.)
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

const build = spawnSync('cargo', ['build', '-p', 'cometcast-server', '-p', 'cometcast-cli'], {
  cwd: repo,
  stdio: 'inherit',
})
assert.equal(build.status, 0, 'cargo build failed')

const server = spawn(resolve(targetDir, 'debug/cometcast-server'), ['--listen', `127.0.0.1:${port}`, '--web-dir', webDir], {
  stdio: 'inherit',
})

/** Starts the CLI and gives access to its stdout lines. */
function startCli(args) {
  const proc = spawn(resolve(targetDir, 'debug/cometcast-cli'), args, { stdio: ['ignore', 'pipe', 'inherit'] })
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

/** All `stats packets=N ...` lines parsed. */
const statsOf = (lines) =>
  lines
    .map((l) => /^stats packets=(\d+) bytes=(\d+) frames=(\d+) keyframes=(\d+)/.exec(l))
    .filter(Boolean)
    .map((m) => ({ packets: +m[1], bytes: +m[2], frames: +m[3], keyframes: +m[4] }))

let browser
const children = []
try {
  await waitHealthy()
  browser = await chromium.launch({
    args: [
      '--use-fake-ui-for-media-stream',
      '--auto-select-desktop-capture-source=Entire screen',
      '--use-fake-device-for-media-stream',
      // Without this Chromium hides host IPs behind mDNS names that the Rust side cannot resolve.
      '--disable-features=WebRtcHideLocalIpsWithMdns',
    ],
  })
  const newPage = async () => {
    const ctx = await browser.newContext({ locale: 'en-US', viewport: { width: 1280, height: 800 } })
    const p = await ctx.newPage()
    p.on('pageerror', (e) => console.error('pageerror:', e.message))
    return p
  }

  // ---------- A) browser sender -> native receiver ----------
  console.log('A) browser sender -> cometcast-cli receive')
  const rx = startCli(['receive', '--server', `127.0.0.1:${port}`])
  children.push(rx.proc)
  const codeLine = await waitFor(() => rx.lines.find((l) => l.startsWith('Room code: ')), 'room code from CLI', 10000)
  const code = codeLine.slice('Room code: '.length).trim()
  assert.match(code, /^[A-HJ-NP-Z2-9]{6}$/, 'room code format')

  const sender = await newPage()
  await presenterScreen(sender, base, code)
  await sender.locator('[data-testid=status][data-phase=sharing]').waitFor({ timeout: 20000 })

  // The CLI prints one stats line per second; packet counts must keep growing.
  const grown = await waitFor(() => {
    const s = statsOf(rx.lines)
    if (s.length < 4) return null
    const last = s.slice(-3)
    const increasing = last[0].packets < last[1].packets && last[1].packets < last[2].packets
    return increasing && last[2].packets > 50 ? s : null
  }, 'increasing packet counts in CLI output')
  const final = grown.at(-1)
  console.log('A) final stats:', final)
  assert.ok(final.bytes > 0 && final.frames > 0, 'bytes and frames counted')
  assert.ok(rx.lines.some((l) => /^Video track: video\//.test(l)), 'video track line')
  console.log('A) codec line:', rx.lines.find((l) => l.startsWith('Video track:')))
  // Key frames: the browser sends one at the start. Reported, but a failed detection is only
  // a warning because key frame detection is a quick check (see cc-transport rtp.rs).
  if (final.keyframes < 1) console.warn('A) WARNING: no key frame detected')

  // Stopping the browser sender must end the CLI normally.
  await sender.getByTestId('presenting-stop').click()
  await sender.getByTestId('disconnect').click()
  const code1 = await Promise.race([
    rx.exited,
    new Promise((r) => setTimeout(() => r('timeout'), 10000)),
  ])
  assert.equal(code1, 0, 'CLI receive exits 0 after peer left')
  assert.ok(rx.lines.some((l) => l === 'Sender left.'))
  await sender.context().close()

  // ---------- B) native sender -> browser receiver ----------
  console.log('B) cometcast-cli send -> browser receiver')
  const receiver = await newPage()
  const code2 = await openDisplay(receiver, base)
  const tx = startCli(['send', code2, '--server', `127.0.0.1:${port}`, '--seconds', '12'])
  children.push(tx.proc)
  await receiver.locator('[data-testid=status][data-phase=receiving]').waitFor({ timeout: 20000 })
  const video = receiver.getByTestId('remote-video')
  await waitFor(() => video.evaluate((v) => v.videoWidth > 0), 'videoWidth > 0')
  const dims = await video.evaluate((v) => [v.videoWidth, v.videoHeight])
  const t0 = await video.evaluate((v) => v.currentTime)
  await waitFor(async () => (await video.evaluate((v) => v.currentTime)) > t0 + 0.3, 'currentTime advancing')
  console.log('B) browser decodes the Rust stream:', dims)
  assert.deepEqual(dims, [320, 180], 'test frame size')
  assert.equal(await tx.exited, 0, 'CLI send exits 0 after its duration')

  console.log('E2E NATIVE OK')
} finally {
  for (const c of children) c.kill('SIGTERM')
  await browser?.close()
  server.kill('SIGTERM')
}
