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
//      A relay that only the link asked for is NOT used before the person confirms it in a dialog (no WebSocket,
//      no request to it until then); Cancel falls back to the default; config.json relays need no dialog.
//      Both ends show the same connection check code. A config.json that cannot be used shows a hint.
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
import { runWizard } from './helpers.mjs'

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
    // Pages of the app (/app/display, /app/<CODE>): a static host answers them with index.html
    // (GitHub Pages: a copy of index.html as 404.html).
    if (!/\.[A-Za-z0-9]+$/.test(rel)) rel = 'index.html'
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
const L = await startRelay('L limited', ['--allowed-origins', S.origin, '--set', 'limits.http_per_min=1'])
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

  /** Display + presenter from the same static origin; both must end up connected through `relay`. */
  async function connectPair(query, relayHost, { expectConfirm = false }) {
    // The dialog of a relay that came from ?server= (UI only, no URL parameter can skip it).
    const confirmRelay = async (p) => {
      const dlg = p.getByTestId('relay-confirm')
      if (!expectConfirm) {
        assert.equal(await dlg.count(), 0, 'no dialog for a trusted relay')
        return
      }
      await dlg.waitFor({ timeout: 10000 })
      assert.equal(await p.getByTestId('relay-confirm-host').innerText(), relayHost)
      await new Promise((r) => setTimeout(r, 400))
      assert.deepEqual(p.log.sockets, [], 'no WebSocket before the confirmation')
      assert.ok(!p.log.requests.some((u) => u.startsWith(`http://${relayHost}`)), 'no request to the relay before the confirmation')
      await p.getByTestId('relay-confirm-use').click()
    }
    const receiver = await newPage()
    await receiver.goto(`${S.app}display${query}`)
    // The confirmation comes before everything else (spec 7.2): nothing of the wizard is drawn behind it.
    if (expectConfirm) assert.equal(await receiver.getByTestId('setup').count(), 0, 'no wizard behind the confirmation')
    await confirmRelay(receiver)
    // Setup: the address step names the host only
    await receiver.getByTestId('wz-start').click()
    await receiver.getByTestId('wz-next').click()
    await receiver.getByTestId('wz-next').click()
    assert.equal((await receiver.getByTestId('wz-domain').innerText()).trim(), relayHost)
    await receiver.getByTestId('wz-ok').click()
    await receiver.getByTestId('wz-finish').click()
    await receiver.locator('[data-testid=status][data-phase=waiting_sender]').waitFor({ timeout: 20000 })
    const code = (await receiver.getByTestId('room-code').innerText()).replace(/\s+/g, '')
    assert.match(code, /^[A-HJ-NP-Z2-9]{6}$/)
    // The idle screen shows the address of the relay in use
    assert.equal((await receiver.getByTestId('address').innerText()).trim(), relayHost)
    // A link-supplied relay is never remembered by the setup
    if (expectConfirm) assert.equal(await receiver.evaluate(() => JSON.parse(localStorage.getItem('cometcast.display.setup')).server), null)

    const sender = await newPage()
    await sender.goto(S.app + (query ? `${query}&testVideo=1` : '?testVideo=1'))
    // The presenter page shows the confirmation page instead of the start page; the host appears on it only.
    if (expectConfirm) assert.equal(await sender.getByTestId('start-page').count(), 0, 'no start page behind the confirmation')
    await confirmRelay(sender)
    await sender.getByTestId('start-code').fill(code)
    await sender.getByTestId('start-browser').click()
    await sender.getByTestId('what-page').waitFor({ timeout: 20000 })
    await sender.getByTestId('what-screen').click()
    await sender.locator('[data-testid=status][data-phase=sharing]').waitFor({ timeout: 20000 })
    await receiver.locator('[data-testid=status][data-phase=receiving]').waitFor({ timeout: 20000 })
    const sas = await waitFor(() => sender.getByTestId('sas-code').innerText(), 'connection check code (presenter)')
    assert.match(sas.trim(), /^\d{3} \d{3}$/)
    assert.equal((await waitFor(() => receiver.getByTestId('sas-code').innerText(), 'connection check code (display)')).trim(), sas.trim())
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

  /** The address that the setup wizard offers on a page (no relay is contacted before Finish). */
  async function wizardAddress(url) {
    const p = await newPage()
    await p.goto(url)
    await p.getByTestId('wz-start').click()
    await p.getByTestId('wz-next').click()
    await p.getByTestId('wz-next').click()
    const host = (await p.getByTestId('wz-domain').innerText()).trim()
    await p.context().close()
    return host
  }

  console.log('1) ?server= : static origin S -> relay R (cross-origin, allow-listed)')
  await connectPair(`?server=${R.host}`, R.host, { expectConfirm: true })
  {
    // Cancel: the default relay (here the static host, which has no relay) is used and nothing connects.
    const p = await newPage()
    await p.goto(`${S.app}display?server=${R.host}`)
    await p.getByTestId('relay-confirm-cancel').click()
    await runWizard(p)
    await p.getByTestId('idle').waitFor()
    assert.equal((await p.getByTestId('address').innerText()).trim(), S.origin.replace('http://', ''))
    await new Promise((r) => setTimeout(r, 500))
    assert.deepEqual(p.log.sockets, [], 'no WebSocket after Cancel')
    assert.ok(!p.log.requests.some((u) => u.startsWith(`http://${R.host}`)), 'no request to the declined relay')
    await p.context().close()
  }
  console.log('   ok')

  console.log('2) config.json next to index.html')
  {
    writeFileSync(join(distDir, 'config.json'), JSON.stringify({ relays: [`ws://${R.host}/ws`] }))
    await connectPair('', R.host, { expectConfirm: false })
    console.log('   ok: relay from config.json is the default (no dialog)')
    // ?server= still wins over config.json.
    assert.equal(await wizardAddress(`${S.app}display?server=${encodeURIComponent(`ws://${O.host}`)}`), O.host)
    // An unusable config.json is ignored (same origin = the static host, which has no relay).
    writeFileSync(join(distDir, 'config.json'), JSON.stringify({ relays: ['ws://relay.example.org/ws'] }))
    assert.equal(await wizardAddress(`${S.app}display`), S.origin.replace('http://', ''))
    writeFileSync(join(distDir, 'config.json'), JSON.stringify({ relays: [`ws://${R.host}/ws`] }))
  }

  console.log('3) open-mode relay: ALTCHA challenge fetched across origins (CORS)')
  {
    const receiver = await newPage()
    await receiver.goto(`${S.app}display?server=${O.host}`)
    await receiver.getByTestId('relay-confirm-use').click()
    await runWizard(receiver)
    await receiver.locator('[data-testid=status][data-phase=waiting_sender]').waitFor({ timeout: 20000 })
    assert.ok(receiver.log.requests.includes(`http://${O.host}/altcha/challenge`), 'challenge fetched from the relay')
    await receiver.context().close()
    console.log('   ok')
  }

  console.log('4) origin X (not allowed) is rejected')
  {
    const p = await newPage()
    await p.goto(`${X.app}display?server=${R.host}`)
    // R is the config.json default here, so no dialog.
    await runWizard(p)
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

    // A limited client on the allowed origin can read the status and Retry-After; on origin X it cannot.
    const probe = (page) =>
      page.evaluate(async (u) => {
        await fetch(u).catch(() => null)
        const r = await fetch(u).catch(() => null)
        return r ? { status: r.status, retryAfter: r.headers.get('retry-after') } : 'blocked'
      }, `${L.base}/altcha/challenge`)
    const lp = await newPage()
    await lp.goto(S.app)
    const seen = await probe(lp)
    assert.equal(seen.status, 429, 'rate limited')
    assert.match(seen.retryAfter ?? '', /^\d+$/, 'Retry-After is readable from the allowed origin')
    await lp.context().close()
    const lx = await newPage()
    await lx.goto(X.app)
    assert.equal(await lx.evaluate(async (u) => fetch(u).then(() => 'readable', () => 'blocked'), `${L.base}/healthz`), 'blocked')
    await lx.context().close()
    console.log('   ok: 429 and Retry-After readable from the allowed origin only')
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
