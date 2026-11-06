// SPDX-License-Identifier: GPL-3.0-or-later
//
// Interop test for content (images, links, PDF documents) between the native Rust peers and the
// browser web app, over the WebRTC DataChannel `control` (docs/web-m2.md).
//   A) WEB sender (content-only mode) -> `cometcast-cli receive`: an image arrives byte for byte, a link
//      is printed and never opened.
//   B) `cometcast-cli send-file` / `send-link` -> WEB receiver: the image decodes (<img> naturalWidth > 0), the
//      link card appears, a PDF gives the translated "cannot show documents yet" notice (no web renderer yet).
//   C) `cometcast-cli send-file x.pdf --then ...` -> `cometcast-cli receive`: the PDF opens (rendered in the
//      sandboxed worker process), pages turn with the page commands, stop ends it.
// Starts the real cometcast-server, the real CLI binary and a real Chromium.
// Run: npm run build && npm run e2e:content   (needs a Rust toolchain and Chrome/Chromium for playwright)
import { createRequire } from 'node:module'
import { spawn, spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'
import { deflateSync } from 'node:zlib'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { createServer } from 'node:net'
import assert from 'node:assert/strict'

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
const pdfPath = resolve(repo, 'crates/cc-docs/tests/data/classroom.pdf')
const work = mkdtempSync(resolve(tmpdir(), 'cometcast-content-'))

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

const withTimeout = (p, ms, what) => {
  let timer
  const limit = new Promise((_, rej) => {
    timer = setTimeout(() => rej(new Error(`timeout: ${what}`)), ms)
  })
  return Promise.race([p, limit]).finally(() => clearTimeout(timer))
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

/** A gradient RGB PNG, built by hand so that the test needs no image library. */
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
  ihdr[8] = 8
  ihdr[9] = 2
  const rows = []
  for (let y = 0; y < height; y++) {
    const row = Buffer.alloc(1 + width * 3)
    for (let x = 0; x < width; x++) {
      row[1 + x * 3] = (x * 7 + y * 13) & 255
      row[2 + x * 3] = (x * 3) & 255
      row[3 + x * 3] = (y * 5) & 255
    }
    rows.push(row)
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(Buffer.concat(rows))),
    chunk('IEND', Buffer.alloc(0)),
  ])
}

/** The same 64-bit FNV-1a fingerprint the CLI prints for received images. */
function fnv1a64(buf) {
  let h = 0xcbf29ce484222325n
  for (const b of buf) h = ((h ^ BigInt(b)) * 0x100000001b3n) & 0xffffffffffffffffn
  return h.toString(16).padStart(16, '0')
}

let browser
const children = []
try {
  await waitHealthy()
  browser = await chromium.launch({
    args: [
      // Without this Chromium hides host IPs behind mDNS names that the Rust side cannot resolve.
      '--disable-features=WebRtcHideLocalIpsWithMdns',
    ],
  })
  const newPage = async (locale = 'en-US') => {
    const ctx = await browser.newContext({ locale, viewport: { width: 1280, height: 800 } })
    const p = await ctx.newPage()
    p.on('pageerror', (e) => console.error('pageerror:', e.message))
    return p
  }
  const png = makePng(96, 48)

  // ---------- A) web sender (content only) -> native receiver ----------
  console.log('A) web content sender -> cometcast-cli receive')
  const rx = startCli(['receive', '--server', `127.0.0.1:${port}`])
  children.push(rx.proc)
  const codeLine = await waitFor(() => rx.lines.find((l) => l.startsWith('Room code: ')), 'room code from CLI', 10000)
  const code = codeLine.slice('Room code: '.length).trim()

  const sender = await newPage()
  await sender.goto(base)
  await sender.getByRole('button', { name: /Send file or link/ }).click()
  await sender.locator('#room-code').fill(code)
  await sender.getByRole('button', { name: 'Connect' }).click()
  await sender.locator('[data-testid=status][data-ready=true]').waitFor({ timeout: 20000 })

  await sender.getByTestId('image-input').setInputFiles({ name: 'test.png', mimeType: 'image/png', buffer: png })
  const imageLine = await waitFor(() => rx.lines.find((l) => l.startsWith('Content: image ')), 'image line in CLI output')
  assert.ok(imageLine.includes('name="test.png"') && imageLine.includes('mime=image/png'), imageLine)
  assert.ok(imageLine.includes(`bytes=${png.length}`), `size: ${imageLine}`)
  assert.ok(imageLine.includes(`fnv1a64=${fnv1a64(png)}`), `bytes differ: ${imageLine}`)

  await sender.locator('#link-url').fill('https://example.org/path?x=1')
  await sender.getByRole('button', { name: 'Send link' }).click()
  const linkLine = await waitFor(() => rx.lines.find((l) => l.startsWith('Content: link ')), 'link line in CLI output')
  assert.equal(linkLine, 'Content: link url=https://example.org/path?x=1 (not opened)')
  assert.ok(!rx.lines.some((l) => l.startsWith('Content: error')), 'no content errors')
  // The web sender's "stop showing" is a cmd the native receiver understands.
  await sender.getByTestId('stop-showing').click()
  await waitFor(() => rx.lines.includes('Content: stopped'), 'stopped line in CLI output')
  await sender.context().close()
  assert.equal(await withTimeout(rx.exited, 15000, 'CLI receive exit'), 0, 'CLI receive exits 0 after the sender left')

  // ---------- B) native sender -> web receiver ----------
  console.log('B) cometcast-cli send-file / send-link -> web receiver')
  const receiver = await newPage()
  await receiver.goto(base)
  await receiver.getByRole('button', { name: /Receive on this device/ }).click()
  await receiver.locator('[data-testid=status][data-phase=waiting_sender]').waitFor({ timeout: 10000 })
  const code2 = (await receiver.getByTestId('room-code').innerText()).trim()
  const pngFile = resolve(work, 'native.png')
  writeFileSync(pngFile, png)
  const send1 = startCli(['send-file', code2, pngFile, '--server', `127.0.0.1:${port}`])
  children.push(send1.proc)
  const img = receiver.getByTestId('received-image')
  await img.waitFor({ timeout: 20000 })
  const dims = await waitFor(() => img.evaluate((i) => (i.naturalWidth > 0 ? [i.naturalWidth, i.naturalHeight] : 0)), 'image decoded')
  console.log('B) web receiver decoded the native image:', dims)
  assert.deepEqual(dims, [96, 48])
  await waitFor(() => send1.lines.includes('Sent.'), 'send-file reports Sent.')
  // The CLI stays connected (a receiver drops the content when the sender leaves); end it now.
  send1.proc.kill('SIGTERM')

  // A second sender (the same room is reused by the web receiver after the first left).
  await receiver.locator('[data-testid=status][data-phase=peer_left]').waitFor({ timeout: 10000 })
  const send2 = startCli(['send-link', code2, 'https://example.org/native?x=1', '--server', `127.0.0.1:${port}`])
  children.push(send2.proc)
  const card = receiver.getByTestId('link-card')
  await card.waitFor({ timeout: 20000 })
  assert.match(await card.innerText(), /example\.org wants to open a link/)
  await waitFor(() => send2.lines.includes('Sent.'), 'send-link reports Sent.')
  send2.proc.kill('SIGTERM')

  await receiver.locator('[data-testid=status][data-phase=peer_left]').waitFor({ timeout: 10000 })
  const send3 = startCli(['send-file', code2, pdfPath, '--server', `127.0.0.1:${port}`])
  children.push(send3.proc)
  const err = receiver.getByTestId('content-error')
  await err.waitFor({ timeout: 20000 })
  assert.match(await err.innerText(), /cannot show documents yet/)
  console.log('B) web receiver says:', await err.innerText())
  await waitFor(() => send3.lines.includes('Sent.'), 'send-file pdf reports Sent.')
  send3.proc.kill('SIGTERM')
  // German translation of the same notice.
  const de = await newPage('de-DE')
  await de.goto(base)
  await de.getByRole('button', { name: /Auf diesem Gerät empfangen/ }).click()
  await de.locator('[data-testid=status][data-phase=waiting_sender]').waitFor({ timeout: 10000 })
  const code3 = (await de.getByTestId('room-code').innerText()).trim()
  const send4 = startCli(['send-file', code3, pdfPath, '--server', `127.0.0.1:${port}`])
  children.push(send4.proc)
  await de.getByTestId('content-error').waitFor({ timeout: 20000 })
  assert.match(await de.getByTestId('content-error').innerText(), /Dokumente noch nicht anzeigen/)
  await waitFor(() => send4.lines.includes('Sent.'), 'send-file pdf (de) reports Sent.')

  // ---------- C) native sender -> native receiver: PDF and page commands ----------
  console.log('C) cometcast-cli send-file pdf -> cometcast-cli receive')
  const rx2 = startCli(['receive', '--server', `127.0.0.1:${port}`])
  children.push(rx2.proc)
  const code4 = (await waitFor(() => rx2.lines.find((l) => l.startsWith('Room code: ')), 'room code (C)', 10000))
    .slice('Room code: '.length)
    .trim()
  const send5 = startCli(['send-file', code4, pdfPath, '--then', 'next,go:3,prev,stop', '--seconds', '1', '--server', `127.0.0.1:${port}`])
  children.push(send5.proc)
  assert.equal(await withTimeout(send5.exited, 30000, 'send-file pdf (C) exit'), 0)
  assert.equal(await withTimeout(rx2.exited, 30000, 'receive (C) exit'), 0)
  const seen = rx2.lines.filter((l) => l.startsWith('Content: ') && !l.startsWith('Content: transfer'))
  const pages = seen.filter((l) => l.startsWith('Content: page=')).map((l) => Number(/page=(\d+)/.exec(l)[1]))
  assert.ok(seen.some((l) => l === 'Content: document name="classroom.pdf" pages=3'), `document line in ${seen}`)
  assert.deepEqual(pages, [1, 2, 3, 2], 'pages in order')
  assert.equal(seen.at(-1), 'Content: stopped')
  assert.ok(!seen.some((l) => l.startsWith('Content: error')), 'no errors')

  console.log('E2E NATIVE CONTENT OK')
} finally {
  for (const c of children) c.kill('SIGTERM')
  await browser?.close()
  server.kill('SIGTERM')
  rmSync(work, { recursive: true, force: true })
}
