// SPDX-License-Identifier: GPL-3.0-or-later
//
// End-to-end test of join approval: real server binary + real Chromium pages (a receiver and
// several senders). Covers: open (the default), confirm with allow (hold) and deny, lock, kick
// (remove), timeout, keyboard operation, and the German texts of the waiting screen.
// Run: npm run build && E2E_CHROME=/usr/bin/google-chrome npm run e2e:join
//
// Senders use the content-only page ("Send file or link"): it needs no screen capture, so the
// test is about the join, not about video.
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
const APPROVAL_SECS = 5 // short, so that the timeout case is quick

function startServer() {
  const build = spawnSync('cargo', ['build', '-p', 'cometcast-server'], { cwd: repo, stdio: 'inherit' })
  assert.equal(build.status, 0, 'cargo build failed')
  const targetDir = process.env.CARGO_TARGET_DIR ?? resolve(repo, 'target')
  return spawn(
    resolve(targetDir, 'debug/cometcast-server'),
    ['--listen', `127.0.0.1:${port}`, '--web-dir', webDir, '--set', `join_approval_secs=${APPROVAL_SECS}`],
    { stdio: 'inherit' },
  )
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

const server = startServer()
let browser
try {
  await waitHealthy()
  browser = await chromium.launch({
    executablePath: process.env.E2E_CHROME || undefined,
    args: ['--disable-features=WebRtcHideLocalIpsWithMdns'],
  })
  const mk = async () => {
    const ctx = await browser.newContext({ locale: 'en-US', viewport: { width: 1280, height: 800 } })
    ctx.on('page', (p) => p.on('pageerror', (e) => console.error('pageerror:', e.message)))
    return ctx
  }
  const rctx = await mk()
  const receiver = await rctx.newPage()
  await receiver.goto(base)
  await receiver.getByRole('button', { name: /Receive on this device/ }).click()
  await receiver.locator('[data-testid=status][data-phase=waiting_sender]').waitFor({ timeout: 10000 })
  const code = (await receiver.getByTestId('room-code').innerText()).trim()
  console.log('room code:', code)

  const checked = (page, mode) => page.getByTestId(`mode-${mode}`).getAttribute('aria-checked')
  const setMode = async (mode) => {
    // While somebody presents the panel is a chip; open it first.
    if (await receiver.getByTestId('join-chip').isVisible().catch(() => false)) {
      await receiver.getByTestId('join-chip').click()
    }
    await receiver.getByTestId(`mode-${mode}`).click()
    await receiver.locator(`[data-testid=mode-${mode}][aria-checked=true]`).waitFor({ timeout: 5000 })
  }
  const newSender = async (name, locale = 'en-US') => {
    const ctx = await browser.newContext({ locale, viewport: { width: 1000, height: 800 } })
    ctx.on('page', (p) => p.on('pageerror', (e) => console.error('pageerror:', e.message)))
    const page = await ctx.newPage()
    await page.goto(base)
    await page.getByRole('button', { name: locale.startsWith('de') ? /Datei oder Link/ : /Send file or link/ }).click()
    if (name) await page.locator('#display-name').fill(name)
    return page
  }
  const connect = async (page) => {
    await page.locator('#room-code').fill(code)
    await page.locator('form').first().locator('button[type=submit]').click()
  }
  const phase = (page, p, timeout = 10000) =>
    page.locator(`[data-testid=status][data-phase=${p}]`).waitFor({ timeout })

  // --- 1. First run: the default is "open"
  assert.equal(await checked(receiver, 'open'), 'true', 'default mode is open')
  assert.equal(await checked(receiver, 'confirm'), 'false')

  // --- 2. Open: a sender joins at once, shows up as connected, and can be removed (kick)
  const s1 = await newSender('Ana')
  await connect(s1)
  await phase(s1, 'sharing', 15000)
  await receiver.locator('[data-testid=status][data-phase=receiving]').waitFor({ timeout: 15000 })
  await receiver.getByTestId('join-chip').click()
  assert.match(await receiver.getByTestId('peer-name').innerText(), /Ana/)
  await receiver.getByTestId('remove-peer').click()
  await phase(s1, 'error')
  assert.match(await s1.getByTestId('status').innerText(), /The host removed you from the room/)
  await receiver.locator('[data-testid=status][data-phase=peer_left]').waitFor({ timeout: 10000 })
  console.log('open + kick OK')

  // --- 3. Confirm: waiting screen with check number; short press does not allow; hold does
  await setMode('confirm')
  const s2 = await newSender('Bo <b>x</b>')
  await connect(s2)
  await phase(s2, 'waiting_approval')
  const check = (await s2.getByTestId('waiting-check').innerText()).trim()
  assert.match(check, /^[1-9][0-9]$/)
  assert.match(await s2.getByTestId('join-waiting').innerText(), /Nothing is sent from your device until then/)
  await receiver.getByTestId('join-request').waitFor({ timeout: 5000 })
  assert.equal((await receiver.getByTestId('request-check').innerText()).trim(), check, 'same check number on both sides')
  assert.equal(await receiver.locator('[data-testid=join-request] img, [data-testid=join-request] b').count(), 0, 'the name is plain text')
  assert.match(await receiver.getByTestId('request-name').innerText(), /Bo <b>x<\/b>/)
  // Nothing took the focus when the request arrived.
  assert.equal(await receiver.evaluate(() => document.activeElement?.hasAttribute('data-allow') ?? false), false)
  const allow = receiver.locator('[data-allow]').first()
  // A short click only shows a hint.
  await allow.click()
  await receiver.getByTestId('hold-hint').waitFor({ timeout: 3000 })
  assert.equal(await s2.getByTestId('status').getAttribute('data-phase'), 'waiting_approval', 'still waiting after a short press')
  // Hold with the mouse.
  const box = await allow.boundingBox()
  await receiver.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
  await receiver.mouse.down()
  await receiver.waitForTimeout(1300)
  await receiver.mouse.up()
  await phase(s2, 'sharing', 15000)
  console.log('confirm + allow (mouse hold) OK')

  // --- 4. Deny (one press): the sender is told, and has to wait before asking again
  await receiver.getByTestId('join-chip').click().catch(() => {})
  await receiver.getByTestId('remove-peer').click()
  await phase(s2, 'error')
  await receiver.locator('[data-testid=status][data-phase=peer_left]').waitFor({ timeout: 10000 })
  const s3 = await newSender('Cy')
  await connect(s3)
  await phase(s3, 'waiting_approval')
  await receiver.getByTestId('request-name').waitFor()
  await receiver.getByTestId('deny').first().click()
  await phase(s3, 'error')
  assert.match(await s3.getByTestId('status').innerText(), /The host did not allow you to join/)
  await s3.getByTestId('denied-cooldown').waitFor({ timeout: 3000 })
  assert.equal(await s3.locator('form').first().locator('button[type=submit]').isDisabled(), true, 'cannot ask again at once')
  assert.equal(await receiver.getByTestId('join-request').count(), 0)
  console.log('deny OK')

  // --- 5. Keyboard: Deny gets the focus after a decision, Allow works with Enter held
  const s4 = await newSender('Di')
  await connect(s4)
  await phase(s4, 'waiting_approval')
  await receiver.getByTestId('join-request').waitFor()
  await receiver.getByTestId('mode-confirm').focus()
  await receiver.getByTestId('deny').first().focus()
  await receiver.keyboard.down('Enter')
  await receiver.waitForTimeout(150)
  await receiver.keyboard.up('Enter') // Enter on "Deny" is one press
  await phase(s4, 'error')
  const s5 = await newSender('Ed')
  await connect(s5)
  await phase(s5, 'waiting_approval')
  await receiver.getByTestId('join-request').waitFor()
  await receiver.locator('[data-allow]').first().focus()
  await receiver.keyboard.down('Enter')
  await receiver.waitForTimeout(1300)
  await receiver.keyboard.up('Enter')
  await phase(s5, 'sharing', 15000)
  console.log('keyboard OK')

  // --- 6. Lock: a waiting request is discarded with a message; later joins hear "locked"
  await receiver.getByTestId('join-chip').click().catch(() => {})
  await receiver.getByTestId('remove-peer').click()
  await receiver.locator('[data-testid=status][data-phase=peer_left]').waitFor({ timeout: 10000 })
  const s6 = await newSender('Flo')
  await connect(s6)
  await phase(s6, 'waiting_approval')
  await receiver.getByTestId('join-request').waitFor()
  await receiver.getByTestId('lock-now').click()
  await receiver.locator('[data-testid=mode-locked][aria-checked=true]').waitFor({ timeout: 5000 })
  await phase(s6, 'error')
  assert.match(await s6.getByTestId('status').innerText(), /locked while you were waiting/)
  assert.match(await receiver.getByTestId('join-notice').innerText(), /1 waiting request\(s\) were discarded/)
  const s7 = await newSender('Gus')
  await connect(s7)
  await phase(s7, 'error')
  assert.match(await s7.getByTestId('status').innerText(), /room code is correct, but the room is locked/)
  // Unlock (open): joining works again.
  await setMode('open')
  await connect(s7)
  await phase(s7, 'sharing', 15000)
  console.log('lock + unlock OK')
  await receiver.getByTestId('join-chip').click().catch(() => {})
  await receiver.getByTestId('remove-peer').click()
  await receiver.locator('[data-testid=status][data-phase=peer_left]').waitFor({ timeout: 10000 })

  // --- 7. Timeout: nobody decides, the sender is told, the request leaves the list
  await setMode('confirm')
  const s8 = await newSender('Hal')
  await connect(s8)
  await phase(s8, 'waiting_approval')
  await receiver.getByTestId('join-request').waitFor()
  await phase(s8, 'error', (APPROVAL_SECS + 8) * 1000)
  assert.match(await s8.getByTestId('status').innerText(), /Nobody answered your request in time/)
  await receiver.locator('[data-testid=join-request]').waitFor({ state: 'detached', timeout: 5000 })
  console.log('timeout OK')

  // --- 8. Cancel while waiting tells the host; German texts (formal "Sie")
  const s9 = await newSender('Ida', 'de-DE')
  await connect(s9)
  await phase(s9, 'waiting_approval')
  assert.match(await s9.getByTestId('join-waiting').innerText(), /Warten auf den Gastgeber/)
  assert.match(await s9.getByTestId('join-waiting').innerText(), /Ihren Beitritt/)
  await receiver.getByTestId('join-request').waitFor()
  await s9.getByTestId('cancel-join').click()
  await receiver.locator('[data-testid=join-request]').waitFor({ state: 'detached', timeout: 5000 })
  console.log('cancel + de OK')

  // --- 9. The chosen mode is remembered in this browser for the next room
  const remembered = await receiver.evaluate(() => localStorage.getItem('cometcast.joinMode'))
  assert.equal(remembered, 'confirm')

  console.log('E2E join approval OK')
} finally {
  await browser?.close()
  server.kill('SIGTERM')
}
