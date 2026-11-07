// SPDX-License-Identifier: GPL-3.0-or-later
//
// Writes a PNG of every state of the display and of the presenter pages, for Dark / Light / OLED black
// (display) and light / dark (presenter), at 1280 x 720 and at a phone viewport (390 x 844). It checks
// nothing; it exists so that a designer can compare the pages with the mockup.
// Run: npm run build && E2E_CHROME=/usr/bin/google-chrome E2E_SHOTS=/tmp/shots npm run e2e:shots
import { spawnSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import assert from 'node:assert/strict'
import { launch, newContext, presenterJoin, presenterScreen, presenterStart, runWizard, startServer, status, waitHealthy, webRoot } from './helpers.mjs'

const out = process.env.E2E_SHOTS || '/tmp/shots'
mkdirSync(out, { recursive: true })
const tmp = mkdtempSync(join(tmpdir(), 'cometcast-shots-'))
const dimDist = join(tmp, 'dist-dim')
assert.equal(spawnSync('npx', ['vite', 'build', '--outDir', dimDist, '--emptyOutDir'], { cwd: webRoot, stdio: 'ignore', env: { ...process.env, VITE_E2E_DIM_MS: '1200' } }).status, 0)

const VIEWS = [
  ['1280', { width: 1280, height: 720 }, {}],
  ['phone', { width: 390, height: 844 }, { isMobile: true, hasTouch: true }],
]
const THEMES = ['dark', 'light', 'oled']
const servers = []
const server = async (opts) => {
  const s = startServer(opts)
  servers.push(s)
  await waitHealthy(s.base)
  return s
}
const shot = (page, name) => page.screenshot({ path: join(out, `${name}.png`) })
const armed = (page) => page.waitForTimeout(1200)

let browser
try {
  browser = await launch()
  const s = await server()
  const sdim = await server({ webDir: dimDist })

  // ---------------- display: every theme and viewport ----------------
  for (const [vname, viewport, extra] of VIEWS) {
    for (const theme of THEMES) {
      const tag = `display-${vname}-${theme}`
      const mkctx = (colorScheme = 'light') =>
        newContext(browser, { viewport, ...extra, colorScheme }).then(async (c) => {
          await c.addInitScript((t) => {
            if (!localStorage.getItem('cometcast.display.appearance')) localStorage.setItem('cometcast.display.appearance', t)
          }, theme)
          return c
        })
      // wizard
      {
        const c = await mkctx()
        const d = await c.newPage()
        await d.goto(`${s.base}/display`)
        await d.getByTestId('wz-start').waitFor()
        await shot(d, `${tag}-setup-0-welcome`)
        await d.getByTestId('wz-start').click()
        await shot(d, `${tag}-setup-1-name`)
        await d.getByTestId('wz-next').click()
        await shot(d, `${tag}-setup-2-mode`)
        await d.getByTestId('wz-next').click()
        await shot(d, `${tag}-setup-3-address`)
        await d.getByTestId('wz-ok').click()
        await shot(d, `${tag}-setup-4-done`)
        await c.close()
      }
      // idle and everything over it
      const c = await mkctx()
      const d = await c.newPage()
      await d.goto(`${s.base}/display`)
      await runWizard(d, { mode: 'confirm' })
      await status(d, 'waiting_sender')
      await d.getByTestId('room-code').waitFor()
      await d.waitForTimeout(300)
      await shot(d, `${tag}-idle`)
      await d.getByTestId('settings-open').click()
      await d.getByTestId('settings-modal').waitFor()
      await d.waitForTimeout(250)
      await shot(d, `${tag}-settings`)
      await d.getByTestId('settings-mode-locked').click()
      await d.getByTestId('settings-close').click()
      await d.waitForTimeout(250)
      await shot(d, `${tag}-idle-locked`)
      await d.getByTestId('settings-open').click()
      await d.getByTestId('settings-mode-confirm').click()
      await d.getByTestId('settings-close').click()
      const code = (await d.getByTestId('room-code').innerText()).replace(/\s+/g, '')
      const pc1 = await newContext(browser, { colorScheme: 'light' })
      const p1 = await pc1.newPage()
      await presenterStart(p1, s.base, code, { name: 'Lena' })
      await d.getByTestId('request-modal').waitFor()
      const pc2 = await newContext(browser, { colorScheme: 'light' })
      const p2 = await pc2.newPage()
      await presenterStart(p2, s.base, code, { name: 'Ben' })
      await d.getByTestId('request-queue').waitFor()
      await d.waitForTimeout(300)
      await shot(d, `${tag}-request-queue`)
      await armed(d)
      await d.getByTestId('request-no').click()
      await d.waitForTimeout(300)
      await shot(d, `${tag}-request`)
      await d.getByTestId('request-yes').click().catch(() => undefined)
      await armed(d)
      await d.getByTestId('request-yes').click()
      await p2.getByTestId('what-page').waitFor({ timeout: 20000 })
      await d.getByTestId('toast').waitFor()
      await d.waitForTimeout(200)
      await shot(d, `${tag}-toast`)
      await p2.getByTestId('what-screen').click()
      await p2.getByTestId('presenting-page').waitFor({ timeout: 20000 })
      await d.waitForFunction(() => document.querySelector('[data-testid=remote-video]')?.videoWidth > 0)
      await d.waitForTimeout(1200)
      await shot(d, `${tag}-presenting-rest`)
      await d.mouse.move(200, 200)
      await d.mouse.move(210, 205)
      await d.waitForTimeout(700)
      await shot(d, `${tag}-presenting-controls`)
      await d.getByTestId('end').click({ force: true })
      await c.close()
      await pc1.close()
      await pc2.close()
      // dimmed
      {
        const c2 = await mkctx()
        const dd = await c2.newPage()
        await dd.goto(`${sdim.base}/display`)
        await runWizard(dd, { mode: 'open' })
        await dd.getByTestId('room-code').waitFor()
        await dd.waitForTimeout(4200)
        await shot(dd, `${tag}-idle-dimmed`)
        await c2.close()
      }
      // offline: a server that goes away
      {
        const sx = await server()
        const c3 = await mkctx()
        const dx = await c3.newPage()
        await dx.goto(`${sx.base}/display`)
        await runWizard(dx, { mode: 'open' })
        await dx.getByTestId('room-code').waitFor()
        sx.proc.kill('SIGTERM')
        await dx.getByTestId('offline').waitFor({ timeout: 20000 })
        await dx.waitForTimeout(300)
        await shot(dx, `${tag}-idle-offline`)
        await c3.close()
      }
    }
  }

  // ---------------- presenter: light and dark, two viewports ----------------
  const d0c = await newContext(browser)
  const d0 = await d0c.newPage()
  await d0.goto(`${s.base}/display`)
  await runWizard(d0, { mode: 'open' })
  await d0.getByTestId('room-code').waitFor()
  const open = (await d0.getByTestId('room-code').innerText()).replace(/\s+/g, '')
  for (const [vname, viewport, extra] of VIEWS) {
    for (const scheme of ['light', 'dark']) {
      const tag = `presenter-${vname}-${scheme}`
      const ua = vname === 'phone' ? { userAgent: 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36' } : {}
      const mk = () => newContext(browser, { viewport, ...extra, ...ua, colorScheme: scheme })
      let c = await mk()
      let p = await c.newPage()
      await p.goto(`${s.base}/`)
      await p.getByTestId('start-page').waitFor()
      await shot(p, `${tag}-start-typed`)
      await p.getByTestId('start-browser').click()
      await shot(p, `${tag}-start-empty-error`)
      await p.goto(`${s.base}/${open}`)
      await shot(p, `${tag}-start-qr`)
      await p.getByTestId('start-app').click()
      await shot(p, `${tag}-start-app-clicked`)
      await p.getByTestId('start-browser').click()
      await p.getByTestId('what-page').waitFor({ timeout: 20000 })
      await shot(p, `${tag}-what`)
      await p.getByTestId('what-file').click()
      await shot(p, `${tag}-file`)
      await p.getByTestId('file-back').click()
      await p.getByTestId('what-link').click()
      await shot(p, `${tag}-link`)
      await p.getByTestId('link-url').fill('https://example.org/x')
      await p.getByTestId('link-go').click()
      await p.getByTestId('presenting-page').waitFor()
      await shot(p, `${tag}-presenting-link`)
      await p.getByTestId('presenting-stop').click()
      if (vname === '1280') {
        await p.getByTestId('what-screen').click()
        await p.getByTestId('presenting-page').waitFor({ timeout: 20000 })
        await shot(p, `${tag}-presenting-screen`)
        await p.getByTestId('presenting-stop').click()
      } else {
        await p.getByTestId('what-screen').click()
        await shot(p, `${tag}-error-noscreen`)
        await p.getByTestId('error-showLink').click()
        await p.getByTestId('link-back').click()
      }
      await p.getByTestId('disconnect').click()
      await shot(p, `${tag}-bye`)
      await c.close()
      // errors
      c = await mk()
      p = await c.newPage()
      await presenterStart(p, s.base, 'ZZZZZZ', { name: 'x' })
      await p.getByTestId('error-page').waitFor()
      await shot(p, `${tag}-error-code`)
      await c.close()
    }
  }
  // approval flow pages: waiting, denied, locked, ended
  {
    const sa = await server({ args: ['--set', 'join_approval_secs=20'] })
    const dc = await newContext(browser)
    const dp = await dc.newPage()
    await dp.goto(`${sa.base}/display`)
    await runWizard(dp, { mode: 'confirm' })
    await dp.getByTestId('room-code').waitFor()
    const code = (await dp.getByTestId('room-code').innerText()).replace(/\s+/g, '')
    for (const [vname, viewport, extra] of VIEWS) {
      for (const scheme of ['light', 'dark']) {
        const tag = `presenter-${vname}-${scheme}`
        const c = await newContext(browser, { viewport, ...extra, colorScheme: scheme })
        const p = await c.newPage()
        await presenterStart(p, sa.base, code, { name: 'Lena' })
        await p.getByTestId('waiting-page').waitFor()
        await dp.getByTestId('request-modal').waitFor()
        await p.waitForTimeout(1500)
        await shot(p, `${tag}-waiting`)
        await armed(dp)
        await dp.getByTestId('request-no').click()
        await p.getByTestId('error-page').waitFor()
        await shot(p, `${tag}-error-denied`)
        await c.close()
      }
    }
    await dp.getByTestId('settings-open').click()
    await dp.getByTestId('settings-mode-locked').click()
    await dp.getByTestId('settings-close').click()
    for (const [vname, viewport, extra] of VIEWS) {
      for (const scheme of ['light', 'dark']) {
        const c = await newContext(browser, { viewport, ...extra, colorScheme: scheme })
        const p = await c.newPage()
        await presenterStart(p, sa.base, code, { name: 'Lena' })
        await p.getByTestId('error-page').waitFor()
        await shot(p, `presenter-${vname}-${scheme}-error-locked`)
        await c.close()
      }
    }
    await dc.close()
  }
  console.log(`screenshots written to ${out}`)
} finally {
  await browser?.close()
  for (const s of servers) s.proc.kill('SIGTERM')
  rmSync(tmp, { recursive: true, force: true })
}
