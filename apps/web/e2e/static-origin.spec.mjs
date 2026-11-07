// SPDX-License-Identifier: GPL-3.0-or-later
//
// End-to-end test: the web app as plain static files on its OWN origin, the relay on another one.
//
//   Static server S   (node, 127.0.0.1:<p1>)  serves a build made with VITE_BASE=/app/ at /app/
//   Static server X   (node, 127.0.0.1:<p2>)  same files, but its origin is NOT in the relay's allowed_origins
//   Relay R           (cometcast-server, internal mode, no --web-dir, allowed_origins = [S])
//   Relay O           (cometcast-server, open mode: ALTCHA always, allowed_origins = [S])
//
//   1) ?server= : receiver and sender pages from S use R; video plays; the relay field shows only the host
//      and warns that a non-default relay is used.
//   2) config.json next to index.html: no ?server=, the page uses the relay from the file (tag "default", no warning);
//      a config.json that is not valid (ws:// to a public host) is ignored.
//   3) ALTCHA over CORS: against O the receiver fetches /altcha/challenge from another origin and gets a room.
//   4) Origin X is rejected: WebSocket refused (translated error) and fetch() is blocked by CORS; a raw WebSocket
//      with a foreign Origin header and one without Origin behave as documented.
//
// Run: npm run e2e:static  (needs a Rust toolchain; E2E_CHROME=/path/to/chrome selects the browser)
import { createRequire } from 'node:module'
import { spawn, spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, resolve, join, extname } from 'node:path'
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { createServer as createHttp, request as httpRequest } from 'node:http'
import { createServer as createNet } from 'node:net'
import assert from 'node:assert/strict'

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')

const here = dirname(fileURLToPath(import.meta.url))
const webRoot = resolve(here, '..')
const repo = resolve(webRoot, '../..')
const targetDir = process.env.CARGO_TARGET_DIR ?? resolve(repo, 'target')
const tmp = mkdtempSync(join(tmpdir(), 'cometcast-e2e-static-'))
const distDir = join(tmp, 'dist-app')

const freePort = () =>
  new Promise((res, rej) => {
    const srv = createNet()
    srv.once('error', rej)
    srv.listen(0, '127.0.0.1', () => {
      const { port } = srv.address()
      srv.close(() => res(port))
    })
  })

// ---- builds -------------------------------------------------------------------------------------
{
  const b = spawnSync('npx', ['vite', 'build', '--outDir', distDir, '--emptyOutDir'], {
    cwd: webRoot,
    stdio: 'inherit',
    env: { ...process.env, VITE_BASE: '/app/' },
  })
  assert.equal(b.status, 0, 'web build failed')
  const c = spawnSync('cargo', ['build', '-p', 'cometcast-server'], { cwd: repo, stdio: 'inherit' })
  assert.equal(c.status, 0, 'cargo build failed')
}
const indexHtml = readFileSync(join(distDir, 'index.html'), 'utf8')
assert.ok(/src="\/app\/assets\//.test(indexHtml), 'assets are under the /app/ base')

// ---- a plain static file server (no relay, no /ws): what GitHub Pages or nginx would do ----------
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.json': 'application/json', '.png': 'image/png', '.wasm': 'application/wasm' }
const statics = []
async function startStatic(label, root) {
  const port = await freePort()
  const hits = []
  const srv = createHttp((req, res) => {
    const url = new URL(req.url, 'http://x')
    hits.push(url.pathname)
    if (!url.pathname.startsWith('/app/')) return res.writeHead(404).end('not found')
    let rel = url.pathname.slice('/app/'.length) || 'index.html'
    const file = resolve(root, rel)
    if (!file.startsWith(root) || !existsSync(file) || !statSync(file).isFile()) return res.writeHead(404).end('not found')
    res.writeHead(200, { 'content-type': TYPES[extname(file)] ?? 'application/octet-stream' }).end(readFileSync(file))
  })
  await new Promise((r) => srv.listen(port, '127.0.0.1', r))
  statics.push(srv)
  console.log(`static ${label}: http://127.0.0.1:${port}/app/`)
  return { port, origin: `http://127.0.0.1:${port}`, app: `http://127.0.0.1:${port}/app/`, hits }
}
const S = await startStatic('S (allowed origin)', distDir)
const X = await startStatic('X (foreign origin)', distDir)

// ---- relays ---------------------------------------------------------------------------------------
const children = []
async function startRelay(label, extraArgs) {
  const port = await freePort()
  const proc = spawn(resolve(targetDir, 'debug/cometcast-server'), ['--listen', `127.0.0.1:${port}`, ...extraArgs], {
    stdio: ['ignore', 'inherit', 'inherit'],
  })
  children.push(proc)
  const base = `http://127.0.0.1:${port}`
  for (let i = 0; ; i++) {
    try {
      if ((await fetch(`${base}/healthz`)).ok) break
    } catch {
      /* not up yet */
    }
    assert.ok(i < 100, `relay ${label} did not become healthy`)
    await new Promise((r) => setTimeout(r, 100))
  }
  console.log(`relay ${label}: ${base} (${extraArgs.join(' ')})`)
  return { port, base, host: `127.0.0.1:${port}` }
}
const R = await startRelay('R internal', ['--allowed-origins', S.origin])
const O = await startRelay('O open', ['--mode', 'open', '--allowed-origins', S.origin])

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

let browser
try {
  browser = await chromium.launch({
    executablePath: process.env.E2E_CHROME || undefined,
    args: [
      '--use-fake-ui-for-media-stream',
      '--auto-select-desktop-capture-source=Entire screen',
      '--use-fake-device-for-media-stream',
      '--disable-features=WebRtcHideLocalIpsWithMdns',
    ],
  })
  const newPage = async (locale = 'en-US') => {
    const ctx = await browser.newContext({ locale, viewport: { width: 1280, height: 800 } })
    const p = await ctx.newPage()
    p.on('pageerror', (e) => console.error('pageerror:', e.message))
    p.log = { requests: [], sockets: [] }
    p.on('request', (r) => p.log.requests.push(r.url()))
    p.on('websocket', (ws) => p.log.sockets.push(ws.url()))
    return p
  }

  /** Receiver + sender from the same static origin; both must end up connected through `relay`. */
  async function connectPair(query, relayHost, { expectWarning }) {
    const receiver = await newPage()
    await receiver.goto(S.app + query)
    // The relay field names the host only.
    assert.equal(await receiver.getByTestId('relay-host').innerText(), relayHost)
    assert.equal(await receiver.getByTestId('relay-warning').count(), expectWarning ? 1 : 0, 'warning for non-default relay')
    assert.equal(await receiver.getByTestId('relay-tag').innerText(), expectWarning ? 'custom' : 'default')
    await receiver.getByRole('button', { name: /Receive on this device/ }).click()
    await receiver.locator('[data-testid=status][data-phase=waiting_sender]').waitFor({ timeout: 20000 })
    const code = (await receiver.getByTestId('room-code').innerText()).trim()
    assert.match(code, /^[A-HJ-NP-Z2-9]{6}$/)
    // On the receiving page the relay stays visible (read-only).
    assert.equal(await receiver.getByTestId('relay-host').innerText(), relayHost)
    assert.equal(await receiver.getByTestId('relay-input').count(), 0)

    const sender = await newPage()
    await sender.goto(S.app + (query ? `${query}&testVideo=1` : '?testVideo=1'))
    await sender.getByRole('button', { name: /Send screen/ }).click()
    await sender.locator('#room-code').fill(code)
    await sender.getByRole('button', { name: 'Share screen' }).click()
    await sender.locator('[data-testid=status][data-phase=sharing]').waitFor({ timeout: 20000 })
    await receiver.locator('[data-testid=status][data-phase=receiving]').waitFor({ timeout: 20000 })
    const video = receiver.getByTestId('remote-video')
    await waitFor(() => video.evaluate((v) => v.videoWidth > 0), 'videoWidth > 0')
    const t0 = await video.evaluate((v) => v.currentTime)
    await waitFor(async () => (await video.evaluate((v) => v.currentTime)) > t0 + 0.3, 'currentTime advancing')
    for (const p of [receiver, sender]) {
      assert.ok(p.log.sockets.every((u) => u === `ws://${relayHost}/ws`), `WebSocket went to the relay: ${p.log.sockets}`)
      assert.ok(p.log.sockets.length >= 1)
      // The page itself came from the static server; no /ws or /ice-servers request went there.
      assert.ok(p.log.requests.some((u) => u === `http://${relayHost}/ice-servers`), 'ICE list asked from the relay')
      assert.ok(!p.log.requests.some((u) => u.startsWith(S.origin) && /\/(ws|ice-servers|altcha)/.test(u)), 'nothing sent to the static host')
    }
    await sender.context().close()
    await receiver.context().close()
    return code
  }

  console.log('1) ?server= : static origin S -> relay R (cross-origin, allow-listed)')
  await connectPair(`?server=${R.host}`, R.host, { expectWarning: true })
  console.log('   ok')

  console.log('2) config.json next to index.html')
  {
    writeFileSync(join(distDir, 'config.json'), JSON.stringify({ relays: [`ws://${R.host}/ws`] }))
    await connectPair('', R.host, { expectWarning: false })
    console.log('   ok: relay from config.json is the default (no warning)')
    // ?server= still wins over config.json.
    const p = await newPage()
    await p.goto(`${S.app}?server=${encodeURIComponent(`ws://${O.host}`)}`)
    assert.equal(await p.getByTestId('relay-host').innerText(), O.host)
    assert.equal(await p.getByTestId('relay-warning').count(), 1)
    // The UI can change and reset the relay; an invalid one is refused with a translated message.
    await p.getByTestId('relay-reset').click()
    assert.equal(await p.getByTestId('relay-host').innerText(), R.host)
    await p.getByTestId('relay-input').fill('ws://relay.example.org')
    await p.getByTestId('relay-apply').click()
    assert.match(await p.getByTestId('relay-error').innerText(), /only allowed for localhost/)
    await p.getByTestId('relay-input').fill(`127.0.0.1:${O.port}`)
    await p.getByTestId('relay-apply').click()
    assert.equal(await p.getByTestId('relay-host').innerText(), O.host)
    assert.equal(await p.getByTestId('relay-warning').count(), 1)
    await p.context().close()
    // An unusable config.json is ignored (same origin = the static host, which has no relay).
    writeFileSync(join(distDir, 'config.json'), JSON.stringify({ relays: ['ws://relay.example.org/ws'] }))
    const q = await newPage()
    await q.goto(S.app)
    assert.equal(await q.getByTestId('relay-host').innerText(), S.origin.replace('http://', ''))
    await q.context().close()
    writeFileSync(join(distDir, 'config.json'), JSON.stringify({ relays: [`ws://${R.host}/ws`] }))
  }

  console.log('3) open-mode relay: ALTCHA challenge fetched across origins (CORS)')
  {
    const receiver = await newPage()
    await receiver.goto(`${S.app}?server=${O.host}`)
    await receiver.getByRole('button', { name: /Receive on this device/ }).click()
    await receiver.locator('[data-testid=status][data-phase=waiting_sender]').waitFor({ timeout: 20000 })
    assert.ok(receiver.log.requests.includes(`http://${O.host}/altcha/challenge`), 'challenge fetched from the relay')
    await receiver.context().close()
    console.log('   ok')
  }

  console.log('4) origin X (not allowed) is rejected')
  {
    const p = await newPage()
    await p.goto(`${X.app}?server=${R.host}`)
    await p.getByRole('button', { name: /Receive on this device/ }).click()
    await p.locator('[data-testid=status][data-phase=error]').waitFor({ timeout: 20000 })
    assert.match(await p.getByTestId('status').innerText(), /connection to the server was lost or could not be made/)
    // CORS: the browser blocks reading the relay's HTTP answers from this origin ...
    const blocked = await p.evaluate(async (u) => fetch(u).then(() => 'readable', () => 'blocked'), `${R.base}/healthz`)
    assert.equal(blocked, 'blocked')
    await p.context().close()
    // ... and the same call from the allowed origin works.
    const ok = await newPage()
    await ok.goto(S.app)
    assert.equal(await ok.evaluate(async (u) => fetch(u).then((r) => r.text(), () => 'blocked'), `${R.base}/healthz`), 'ok')
    await ok.context().close()
    console.log('   ok: WebSocket refused and fetch blocked for the unlisted origin')
  }

  console.log('5) raw WebSocket checks against R (Node, so the Origin header is ours)')
  {
    // A hand-made HTTP upgrade request, so the Origin header is exactly what we set.
    const upgrade = (origin) =>
      new Promise((res) => {
        {
          const req = httpRequest({
            host: '127.0.0.1',
            port: R.port,
            path: '/ws',
            headers: {
              Connection: 'Upgrade',
              Upgrade: 'websocket',
              'Sec-WebSocket-Version': '13',
              'Sec-WebSocket-Key': Buffer.from('0123456789abcdef').toString('base64'),
              ...(origin ? { Origin: origin } : {}),
            },
          })
          req.on('upgrade', (_r, sock) => { sock.destroy(); res('upgraded') })
          req.on('response', (r) => { r.resume(); res(`http ${r.statusCode}`) })
          req.on('error', () => res('error'))
          req.end()
        }
      })
    assert.equal(await upgrade(S.origin), 'upgraded', 'listed origin')
    assert.equal(await upgrade(X.origin), 'http 403', 'unlisted origin')
    assert.equal(await upgrade('null'), 'http 403', 'Origin: null')
    assert.equal(await upgrade(null), 'upgraded', 'no Origin header (native client)')
    console.log('   ok')
  }

  console.log('STATIC ORIGIN E2E OK')
} finally {
  await browser?.close()
  for (const c of children) c.kill('SIGTERM')
  for (const s of statics) s.close()
  rmSync(tmp, { recursive: true, force: true })
}
