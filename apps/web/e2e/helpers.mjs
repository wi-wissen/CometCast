// SPDX-License-Identifier: GPL-3.0-or-later
//
// Shared pieces of the end-to-end tests: the server, the browser, the display's setup wizard and
// the presenter's start page. The tests use the real server binary and real Chrome/Chromium.
import { createRequire } from 'node:module'
import { spawn, spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'
import assert from 'node:assert/strict'

const require = createRequire(import.meta.url)
export const { chromium } = require('playwright')

export const here = dirname(fileURLToPath(import.meta.url))
export const webRoot = resolve(here, '..')
export const repo = resolve(webRoot, '../..')
export const webDist = resolve(webRoot, 'dist')

export const randomPort = () => Number(process.env.E2E_PORT ?? 18443 + Math.floor(Math.random() * 1000))

/** Builds (once per process) and starts cometcast-server serving `webDir`. Returns { proc, base, port }. */
export function startServer({ webDir = webDist, port = randomPort(), args = [] } = {}) {
  if (!startServer.built) {
    const build = spawnSync('cargo', ['build', '-p', 'cometcast-server'], { cwd: repo, stdio: 'inherit' })
    assert.equal(build.status, 0, 'cargo build failed')
    startServer.built = true
  }
  const targetDir = process.env.CARGO_TARGET_DIR ?? resolve(repo, 'target')
  const proc = spawn(resolve(targetDir, 'debug/cometcast-server'), ['--listen', `127.0.0.1:${port}`, '--web-dir', webDir, ...args], { stdio: 'inherit' })
  return { proc, port, base: `http://127.0.0.1:${port}` }
}

export async function waitHealthy(base) {
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

export async function waitFor(fn, what, ms = 15000) {
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

export async function launch(extraArgs = []) {
  return chromium.launch({
    executablePath: process.env.E2E_CHROME || undefined,
    args: [
      '--use-fake-ui-for-media-stream',
      '--auto-select-desktop-capture-source=Entire screen',
      '--use-fake-device-for-media-stream',
      // Without this Chromium hides host IPs behind mDNS names that may not resolve in a sandbox.
      '--disable-features=WebRtcHideLocalIpsWithMdns',
      ...extraArgs,
    ],
  })
}

export async function newContext(browser, opts = {}) {
  const ctx = await browser.newContext({ locale: 'en-US', viewport: { width: 1280, height: 720 }, ...opts })
  ctx.on('page', (p) => p.on('pageerror', (e) => console.error('pageerror:', e.message)))
  return ctx
}

// --- the display ------------------------------------------------------------------------------------

/** Clicks through the five screens of the setup wizard. `mode` is "open" or "confirm". */
export async function runWizard(page, { room, mode = 'open' } = {}) {
  await page.getByTestId('wz-start').click()
  if (room) await page.getByTestId('wz-room').fill(room)
  await page.getByTestId('wz-next').click()
  await page.getByTestId(`wz-mode-${mode}`).click()
  await page.getByTestId('wz-next').click()
  await page.getByTestId('wz-ok').click()
  await page.getByTestId('wz-finish').click()
}

/** Opens /display and runs the wizard when it shows (a fresh context); does not wait for the code. */
export async function startDisplay(page, base, opts = {}) {
  await page.goto(`${base}/display`)
  await page.locator('[data-testid=setup], [data-testid=idle]').first().waitFor({ timeout: 15000 })
  if (await page.getByTestId('setup').count()) await runWizard(page, opts)
}

/** Opens /display (a fresh context shows the wizard) and waits until the room code is there. */
export async function openDisplay(page, base, opts = {}) {
  await startDisplay(page, base, opts)
  await page.locator('[data-testid=status][data-phase=waiting_sender]').waitFor({ timeout: 15000 })
  return readCode(page)
}

/** The join code on the idle screen, without the space between the groups. */
export async function readCode(page) {
  await waitFor(async () => (await page.getByTestId('room-code').innerText()).trim().length > 0, 'a room code')
  return (await page.getByTestId('room-code').innerText()).replace(/\s+/g, '')
}

export const status = (page, phase, timeout = 15000) => page.locator(`[data-testid=status][data-phase=${phase}]`).waitFor({ timeout })

// --- the presenter ------------------------------------------------------------------------------------

/** Types the code on the start page and continues in the browser. */
export async function presenterStart(page, base, code, { name, path = '' } = {}) {
  await page.goto(`${base}/${path}`)
  if (name) await page.getByTestId('start-name').fill(name)
  if (code) await page.getByTestId('start-code').fill(code)
  await page.getByTestId('start-browser').click()
}

/** Joins in open mode and waits for "What to show". */
export async function presenterJoin(page, base, code, opts = {}) {
  await presenterStart(page, base, code, opts)
  await page.getByTestId('what-page').waitFor({ timeout: 20000 })
}

/** Joins and shows the screen (Chrome's own picker is answered by the launch flags). */
export async function presenterScreen(page, base, code, opts = {}) {
  await presenterJoin(page, base, code, opts)
  await page.getByTestId('what-screen').click()
  await page.getByTestId('presenting-page').waitFor({ timeout: 20000 })
}

/** A presenter that sends a file or link (no screen): joins and opens the link page. */
export async function presenterLinkPage(page, base, code, opts = {}) {
  await presenterJoin(page, base, code, opts)
  await page.getByTestId('what-link').click()
}

/** Mocks navigator.wakeLock in a page (call before goto). Calls are kept in window.__wake. */
export async function mockWakeLock(page) {
  await page.addInitScript(() => {
    window.__wake = []
    window.__wakeHeld = 0
    Object.defineProperty(navigator, 'wakeLock', {
      configurable: true,
      value: {
        request: async (type) => {
          window.__wake.push(`acquire:${type}`)
          window.__wakeHeld++
          let released = false
          const l = new EventTarget()
          return Object.assign(l, {
            release: async () => {
              if (released) return
              released = true
              window.__wakeHeld--
              window.__wake.push('release')
            },
          })
        },
      },
    })
  })
}

/**
 * A presenter page that is connected (see presenterJoin): shows a link, an image or a PDF without a
 * screen. Content can only be replaced by going through "Stop showing" and "What to show".
 */
export function contentSender(page) {
  const pageName = () => page.getByTestId('presenter').getAttribute('data-page')
  const toWhat = async () => {
    const p = await pageName()
    if (p === 'presenting') await page.getByTestId('presenting-stop').click()
    else if (p === 'file' || p === 'link') await page.getByTestId(`${p}-back`).click()
    await page.getByTestId('what-page').waitFor()
  }
  return {
    toWhat,
    async link(url) {
      await toWhat()
      await page.getByTestId('what-link').click()
      await page.getByTestId('link-url').fill(url)
      await page.getByTestId('link-go').click()
    },
    async file(file) {
      await toWhat()
      await page.getByTestId('what-file').click()
      await page.getByTestId('file-input').setInputFiles(file)
    },
    async stop() {
      if ((await pageName()) === 'presenting') await page.getByTestId('presenting-stop').click()
    },
  }
}
