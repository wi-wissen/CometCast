// SPDX-License-Identifier: GPL-3.0-or-later
//
// End-to-end test for the clients against the server's OPEN mode (ALTCHA always required for
// create_room) and for STUN/TURN lists (GET /ice-servers).
//
// Real cometcast-server processes, the real cometcast-cli binary and a real Chromium:
//   Server A (open mode)         web receiver solves ALTCHA in a Web Worker; web sender joins and video plays;
//                                German UI shows the translated "checking" status; a raw client without ALTCHA
//                                gets altcha_required; CLI receive solves ALTCHA; CLI send -> web receiver.
//   Server B (open, default limits)   repeated room creation ends in a translated rate_limited message.
//   Server C (open, tiny ban limit)   wrong codes end in a translated banned message.
//   Server D (internal mode + TURN)   open mode refuses TURN, so TURN is tested here: both web pages put the
//                                     server's list (with credentials) into RTCPeerConnection, and the CLI
//                                     reports the list it got. The TURN host does not exist; only the list
//                                     handling is tested, not relaying (UNCERTAIN: no coturn was run).
//   Server E (open, self-signed TLS)  CLI receive over wss:// with --ca-file.
//
// Run: npm run build && npm run e2e:open   (needs a Rust toolchain and the playwright chromium)
import { createRequire } from 'node:module'
import { spawn, spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, resolve, join } from 'node:path'
import { createHmac } from 'node:crypto'
import { mkdtempSync, writeFileSync, existsSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import assert from 'node:assert/strict'
import { createServer } from 'node:net'

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')

const here = dirname(fileURLToPath(import.meta.url))
const repo = resolve(here, '../../..')
const webDir = resolve(here, '../dist')
const targetDir = process.env.CARGO_TARGET_DIR ?? resolve(repo, 'target')
const tmp = mkdtempSync(join(tmpdir(), 'cometcast-e2e-open-'))

const freePort = () =>
  new Promise((res, rej) => {
    const srv = createServer()
    srv.once('error', rej)
    srv.listen(0, '127.0.0.1', () => {
      const { port } = srv.address()
      srv.close(() => res(port))
    })
  })

const build = spawnSync('cargo', ['build', '-p', 'cometcast-server', '-p', 'cometcast-cli'], { cwd: repo, stdio: 'inherit' })
assert.equal(build.status, 0, 'cargo build failed')

const children = []
async function startServer(label, extraArgs, { scheme = 'http' } = {}) {
  const port = await freePort()
  const proc = spawn(
    resolve(targetDir, 'debug/cometcast-server'),
    ['--listen', `127.0.0.1:${port}`, '--web-dir', webDir, ...extraArgs],
    { stdio: ['ignore', 'inherit', 'inherit'] },
  )
  children.push(proc)
  const base = `${scheme}://127.0.0.1:${port}`
  if (scheme === 'http') {
    for (let i = 0; ; i++) {
      try {
        if ((await fetch(`${base}/healthz`)).ok) break
      } catch {
        /* not up yet */
      }
      assert.ok(i < 100, `server ${label} did not become healthy`)
      await new Promise((r) => setTimeout(r, 100))
    }
  }
  console.log(`server ${label}: ${base} (${extraArgs.join(' ')})`)
  return { port, base, proc }
}

function startCli(args) {
  const proc = spawn(resolve(targetDir, 'debug/cometcast-cli'), args, { stdio: ['ignore', 'pipe', 'inherit'] })
  children.push(proc)
  const lines = []
  let buf = ''
  proc.stdout.on('data', (d) => {
    buf += d.toString()
    let i
    while ((i = buf.indexOf('\n')) >= 0) {
      lines.push(buf.slice(0, i))
      console.log('  [cli]', buf.slice(0, i))
      buf = buf.slice(i + 1)
    }
  })
  const exited = new Promise((res) => proc.on('exit', (code) => res(code)))
  return { proc, lines, exited }
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

const statsOf = (lines) =>
  lines
    .map((l) => /^stats packets=(\d+) bytes=(\d+) frames=(\d+) keyframes=(\d+)/.exec(l))
    .filter(Boolean)
    .map((m) => ({ packets: +m[1], bytes: +m[2] }))

/** Runs in every page: records status phases (with the shown text) and RTCPeerConnection configs. */
function pageProbe() {
  window.__phases = []
  new MutationObserver(() => {
    const el = document.querySelector('[data-testid=status]')
    if (!el) return
    const phase = el.getAttribute('data-phase')
    if (window.__phases.at(-1)?.phase !== phase) window.__phases.push({ phase, text: el.innerText })
  }).observe(document, { subtree: true, childList: true, attributes: true, characterData: true })
  window.__pcConfigs = []
  const Orig = window.RTCPeerConnection
  window.RTCPeerConnection = class extends Orig {
    constructor(cfg) {
      super(cfg)
      window.__pcConfigs.push(JSON.parse(JSON.stringify(cfg ?? {})))
    }
  }
}

let browser
try {
  browser = await chromium.launch({
    args: [
      '--use-fake-ui-for-media-stream',
      '--auto-select-desktop-capture-source=Entire screen',
      '--use-fake-device-for-media-stream',
      // Without this Chromium hides host IPs behind mDNS names that the Rust side cannot resolve.
      '--disable-features=WebRtcHideLocalIpsWithMdns',
    ],
  })
  /** A page in its own browser context, with request, worker and WebSocket logs. */
  const newPage = async (locale = 'en-US') => {
    const ctx = await browser.newContext({ locale, viewport: { width: 1280, height: 800 } })
    await ctx.addInitScript(pageProbe)
    const p = await ctx.newPage()
    p.on('pageerror', (e) => console.error('pageerror:', e.message))
    const log = { requests: [], workers: [], sent: [] }
    p.on('request', (r) => log.requests.push(new URL(r.url()).pathname))
    p.on('worker', (w) => log.workers.push(w.url()))
    p.on('websocket', (ws) => ws.on('framesent', (f) => log.sent.push(String(f.payload))))
    p.log = log
    return p
  }
  const phases = (p) => p.evaluate(() => window.__phases)
  const startReceiver = async (p, base, button = /Receive on this device/) => {
    await p.goto(base)
    await p.getByRole('button', { name: button }).click()
  }

  // ================= Server A: open mode =================
  // create_room_per_min is raised only because this script creates several rooms from one address;
  // the default limit is exercised on server B.
  const A = await startServer('A open', ['--mode', 'open', '--set', 'limits.create_room_per_min=60'])

  console.log('A1) a client without ALTCHA support is refused')
  {
    const ws = new WebSocket(`ws://127.0.0.1:${A.port}/ws`)
    const msgs = []
    ws.onmessage = (e) => msgs.push(JSON.parse(e.data))
    await new Promise((res, rej) => {
      ws.onopen = res
      ws.onerror = () => rej(new Error('ws error'))
    })
    ws.send(JSON.stringify({ type: 'hello', proto_version: 1, role: 'receiver' }))
    await waitFor(() => msgs.some((m) => m.type === 'hello'), 'hello')
    ws.send(JSON.stringify({ type: 'create_room' }))
    const err = await waitFor(() => msgs.find((m) => m.type === 'error'), 'error message')
    assert.equal(err.code, 'altcha_required')
    ws.close()
  }

  console.log('A2) web receiver solves ALTCHA in a Web Worker; web sender joins; video plays')
  const receiver = await newPage()
  await startReceiver(receiver, A.base)
  await receiver.locator('[data-testid=status][data-phase=waiting_sender]').waitFor({ timeout: 20000 })
  const code = (await receiver.getByTestId('room-code').innerText()).trim()
  assert.match(code, /^[A-HJ-NP-Z2-9]{6}$/, 'room code format')
  {
    const seen = await phases(receiver)
    console.log('   phases:', seen.map((x) => x.phase).join(' > '))
    const checking = seen.find((x) => x.phase === 'checking')
    assert.ok(checking, 'the "checking" phase was shown')
    assert.match(checking.text, /Checking your browser/, 'translated status text')
    assert.ok(seen.findIndex((x) => x.phase === 'checking') < seen.findIndex((x) => x.phase === 'waiting_sender'))
    assert.ok(receiver.log.requests.includes('/altcha/challenge'), 'challenge was fetched')
    assert.equal(receiver.log.requests.filter((p) => p === '/altcha/challenge').length, 1, 'one puzzle only')
    assert.ok(receiver.log.workers.some((u) => /altchaWorker/.test(u)), `solver ran in a Web Worker (${receiver.log.workers})`)
    const creates = receiver.log.sent.map((s) => JSON.parse(s)).filter((m) => m.type === 'create_room')
    assert.equal(creates.length, 2, 'create_room sent twice')
    assert.equal(creates[0].altcha, undefined, 'first create_room has no altcha')
    const payload = JSON.parse(Buffer.from(creates[1].altcha, 'base64').toString('utf8'))
    assert.ok(payload.challenge.signature && Number.isInteger(payload.solution.counter), 'payload shape')
    assert.ok(receiver.log.requests.includes('/ice-servers'), '/ice-servers was asked for (404 on an open server)')
    const cfgs = await receiver.evaluate(() => window.__pcConfigs)
    assert.equal(cfgs.length, 0, 'no peer connection before a sender joined')
  }

  const sender = await newPage()
  await sender.goto(A.base)
  await sender.getByRole('button', { name: /Send screen/ }).click()
  await sender.locator('#room-code').fill(code)
  await sender.getByRole('button', { name: 'Share screen' }).click()
  await sender.locator('[data-testid=status][data-phase=sharing]').waitFor({ timeout: 20000 })
  await receiver.locator('[data-testid=status][data-phase=receiving]').waitFor({ timeout: 20000 })
  const video = receiver.getByTestId('remote-video')
  await waitFor(() => video.evaluate((v) => v.videoWidth > 0), 'videoWidth > 0')
  const t0 = await video.evaluate((v) => v.currentTime)
  await waitFor(async () => (await video.evaluate((v) => v.currentTime)) > t0 + 0.3, 'currentTime advancing')
  console.log('   video plays through the open server')
  {
    // The sender does not solve a puzzle (join_room needs none) and, with no list on this server, uses no ICE servers.
    assert.ok(!sender.log.requests.includes('/altcha/challenge'), 'sender fetched no puzzle')
    assert.deepEqual((await sender.evaluate(() => window.__pcConfigs))[0]?.iceServers ?? [], [], 'no ICE servers on an open server')
  }
  await sender.context().close()
  await receiver.context().close()

  console.log('A3) German UI shows the translated checking status')
  {
    const de = await newPage('de-DE')
    await startReceiver(de, A.base, /Auf diesem Gerät empfangen/)
    await de.locator('[data-testid=status][data-phase=waiting_sender]').waitFor({ timeout: 20000 })
    const seen = await phases(de)
    const checking = seen.find((x) => x.phase === 'checking')
    assert.ok(checking, 'checking phase shown')
    assert.match(checking.text, /Ihr Browser wird geprüft/)
    await de.context().close()
  }

  console.log('A4) cometcast-cli receive solves ALTCHA; web sender -> CLI')
  const rx = startCli(['receive', '--server', `127.0.0.1:${A.port}`])
  const codeLine = await waitFor(() => rx.lines.find((l) => l.startsWith('Room code: ')), 'room code from CLI', 20000)
  const cliCode = codeLine.slice('Room code: '.length).trim()
  assert.match(cliCode, /^[A-HJ-NP-Z2-9]{6}$/)
  assert.ok(!rx.lines.some((l) => l.startsWith('ICE servers')), 'open server offers no ICE servers')
  {
    const s2 = await newPage()
    await s2.goto(A.base)
    await s2.getByRole('button', { name: /Send screen/ }).click()
    await s2.locator('#room-code').fill(cliCode)
    await s2.getByRole('button', { name: 'Share screen' }).click()
    await s2.locator('[data-testid=status][data-phase=sharing]').waitFor({ timeout: 20000 })
    const grown = await waitFor(() => {
      const s = statsOf(rx.lines)
      if (s.length < 3) return null
      const last = s.slice(-3)
      return last[0].packets < last[1].packets && last[1].packets < last[2].packets && last[2].packets > 30 ? last[2] : null
    }, 'increasing packet counts in CLI output')
    console.log('   CLI stats:', grown)
    await s2.getByTestId('stop').click()
    assert.equal(await Promise.race([rx.exited, new Promise((r) => setTimeout(() => r('timeout'), 10000))]), 0, 'CLI receive exits 0')
    await s2.context().close()
  }

  console.log('A5) cometcast-cli send -> web receiver (created with ALTCHA)')
  {
    const r2 = await newPage()
    await startReceiver(r2, A.base)
    await r2.locator('[data-testid=status][data-phase=waiting_sender]').waitFor({ timeout: 20000 })
    const c = (await r2.getByTestId('room-code').innerText()).trim()
    const tx = startCli(['send', c, '--server', `127.0.0.1:${A.port}`, '--seconds', '8'])
    await r2.locator('[data-testid=status][data-phase=receiving]').waitFor({ timeout: 20000 })
    const v = r2.getByTestId('remote-video')
    await waitFor(() => v.evaluate((x) => x.videoWidth > 0), 'videoWidth > 0')
    assert.equal(await tx.exited, 0, 'CLI send exits 0')
    await r2.context().close()
  }

  // ================= Server B: default open limits -> rate_limited =================
  console.log('B) repeated room creation ends in a translated rate_limited message')
  {
    const B = await startServer('B open, default limits', ['--mode', 'open'])
    const p = await newPage()
    let text = ''
    for (let i = 0; i < 12; i++) {
      // Reloading closes the old connection; the next create_room counts against the limit.
      await startReceiver(p, B.base)
      await p.locator('[data-testid=status][data-phase=waiting_sender], [data-testid=status][data-phase=error]').first().waitFor({ timeout: 20000 })
      if ((await p.getByTestId('status').getAttribute('data-phase')) === 'error') {
        text = await p.getByTestId('status').innerText()
        console.log(`   limit reached after ${i + 1} rooms`)
        break
      }
    }
    console.log('   shown:', text.replace(/\s+/g, ' '))
    assert.match(text, /Too many requests from this network/, 'translated rate_limited message')
    await p.context().close()
    B.proc.kill('SIGTERM')
  }

  // ================= Server C: ban =================
  console.log('C) failed joins end in a translated banned message')
  {
    const C = await startServer('C open, ban after 3 failed joins', [
      '--mode', 'open',
      '--set', 'limits.ban_after_failed_joins=3',
      '--set', 'limits.join_attempts_per_min=1000',
    ])
    const p = await newPage()
    await p.goto(C.base)
    await p.getByRole('button', { name: /Send screen/ }).click()
    await p.locator('#room-code').fill('ZZZZZZ')
    await p.getByRole('button', { name: 'Share screen' }).click()
    let text = ''
    for (let i = 0; i < 8; i++) {
      await p.locator('[data-testid=status][data-phase=error]').waitFor({ timeout: 20000 })
      text = await p.getByTestId('status').innerText()
      if (/blocked for a while/.test(text)) break
      await p.getByRole('button', { name: 'Try again' }).click()
      await p.waitForTimeout(200)
    }
    console.log('   shown:', text.replace(/\s+/g, ' '))
    assert.match(text, /blocked for a while/, 'translated banned message')
    await p.context().close()
    C.proc.kill('SIGTERM')
  }

  // ================= Server D: internal mode with TURN =================
  console.log('D) STUN/TURN list from /ice-servers reaches RTCPeerConnection and the CLI')
  {
    const secret = 'e2e-turn-secret-0123456789abcdef'
    const secretFile = join(tmp, 'turn_secret')
    writeFileSync(secretFile, secret, { mode: 0o600 })
    const cfgFile = join(tmp, 'server-d.toml')
    writeFileSync(
      cfgFile,
      [
        'mode = "internal"',
        '[turn]',
        'urls = ["turn:turn.invalid:3478?transport=udp"]',
        'stun_urls = ["stun:turn.invalid:3478"]',
        `secret_file = "${secretFile}"`,
        'ttl_secs = 600',
        '',
      ].join('\n'),
    )
    const D = await startServer('D internal + TURN', ['--config', cfgFile])
    const list = await (await fetch(`${D.base}/ice-servers`)).json()
    assert.equal(list.length, 2, 'server hands out STUN and TURN entries')

    const r = await newPage()
    await startReceiver(r, D.base)
    await r.locator('[data-testid=status][data-phase=waiting_sender]').waitFor({ timeout: 20000 })
    assert.ok(!(await phases(r)).some((x) => x.phase === 'checking'), 'internal mode needs no puzzle')
    const dCode = (await r.getByTestId('room-code').innerText()).trim()
    const s = await newPage()
    await s.goto(D.base)
    await s.getByRole('button', { name: /Send screen/ }).click()
    await s.locator('#room-code').fill(dCode)
    await s.getByRole('button', { name: 'Share screen' }).click()
    await s.locator('[data-testid=status][data-phase=sharing]').waitFor({ timeout: 30000 })
    await r.locator('[data-testid=status][data-phase=receiving]').waitFor({ timeout: 30000 })

    for (const [name, page] of [['receiver', r], ['sender', s]]) {
      assert.ok(page.log.requests.includes('/ice-servers'), `${name} asked for /ice-servers`)
      const cfgs = await page.evaluate(() => window.__pcConfigs)
      assert.equal(cfgs.length, 1, `${name} made one peer connection`)
      const servers = cfgs[0].iceServers
      assert.equal(servers.length, 2, `${name}: two ICE servers in the RTC configuration`)
      assert.deepEqual(servers[0].urls, ['stun:turn.invalid:3478'])
      const turn = servers[1]
      assert.deepEqual(turn.urls, ['turn:turn.invalid:3478?transport=udp'])
      assert.match(turn.username, /^\d+:[0-9a-f]+$/, 'TURN REST user name')
      assert.equal(turn.credential, createHmac('sha1', secret).update(turn.username).digest('base64'), 'credential = HMAC-SHA1(secret, username)')
      assert.equal(turn.ttl, undefined, 'extra "ttl" field is not passed on')
      console.log(`   ${name}: RTCPeerConnection got`, JSON.stringify(servers[1].urls), 'with credentials')
    }
    await s.context().close()
    await r.context().close()

    // Native client: the CLI reports the list it received.
    const rxd = startCli(['receive', '--server', `127.0.0.1:${D.port}`])
    await waitFor(() => rxd.lines.find((l) => l.startsWith('Room code: ')), 'room code from CLI', 20000)
    assert.ok(
      rxd.lines.some((l) => l === 'ICE servers from the server: 2 (1 with TURN credentials)'),
      `CLI reported the ICE servers (${rxd.lines})`,
    )
    rxd.proc.kill('SIGINT')
    await rxd.exited
  }

  // ================= Server E: TLS =================
  console.log('E) cometcast-cli over wss:// with --ca-file (self-signed development certificate)')
  {
    const certOut = join(tmp, 'e-cert.pem')
    const E = await startServer(
      'E open + self-signed TLS',
      ['--mode', 'open', '--tls-self-signed', '--set', `tls.self_signed_cert_out=${certOut}`],
      { scheme: 'https' },
    )
    await waitFor(() => existsSync(certOut), 'certificate file', 10000)
    // Without --ca-file the certificate must be refused.
    const bad = startCli(['receive', '--server', `https://127.0.0.1:${E.port}`])
    assert.notEqual(await bad.exited, 0, 'untrusted certificate is refused')
    const good = startCli(['receive', '--server', `https://127.0.0.1:${E.port}`, '--ca-file', certOut])
    const line = await waitFor(() => good.lines.find((l) => l.startsWith('Room code: ')), 'room code over wss', 20000)
    assert.match(line, /^Room code: [A-HJ-NP-Z2-9]{6}$/)
    good.proc.kill('SIGINT')
    await good.exited
    E.proc.kill('SIGTERM')
  }

  console.log('E2E OPEN OK')
} finally {
  for (const c of children) c.kill('SIGTERM')
  await browser?.close()
  rmSync(tmp, { recursive: true, force: true })
}
