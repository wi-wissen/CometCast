// SPDX-License-Identifier: GPL-3.0-or-later
//
// End-to-end test of the display and the presenter pages (docs/design/first-run-and-display.md):
// real server binary + real Chrome. Covers: the setup wizard and what it stores, the idle screen,
// Settings (join mode, appearance, persisted), the join request dialog (queue, guard, Yes / No /
// No and block), presenting full bleed with the controls revealed by input, End (new code),
// Locked, wake lock (navigator.wakeLock is mocked and counted), idle dimming (a second build with a
// short delay, build-time only) and the presenter's error pages.
//
// Run: npm run build && E2E_CHROME=/usr/bin/google-chrome npm run e2e:display
// Screenshots: E2E_SHOTS=/tmp/shots writes a PNG of each state (not used by the checks).
import { spawnSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import assert from 'node:assert/strict'
import { launch, mockWakeLock, newContext, openDisplay, presenterJoin, presenterLinkPage, presenterScreen, presenterStart, readCode, startServer, status, waitFor, waitHealthy, webRoot } from './helpers.mjs'

const shots = process.env.E2E_SHOTS
if (shots) mkdirSync(shots, { recursive: true })
const shot = async (page, name) => {
  if (shots) await page.screenshot({ path: join(shots, `${name}.png`) })
}

const servers = []
async function server(opts) {
  const s = startServer(opts)
  servers.push(s)
  await waitHealthy(s.base)
  return s
}

// The idle-dim delay can only be shortened by a build made for the test (VITE_E2E_DIM_MS); the
// normal build has no such setting and no URL parameter can change it.
const tmp = mkdtempSync(join(tmpdir(), 'cometcast-e2e-display-'))
const dimDist = join(tmp, 'dist-dim')
{
  const b = spawnSync('npx', ['vite', 'build', '--outDir', dimDist, '--emptyOutDir'], { cwd: webRoot, stdio: 'inherit', env: { ...process.env, VITE_E2E_DIM_MS: '1500' } })
  assert.equal(b.status, 0, 'dim build failed')
}

const opacity = (page, id) => page.getByTestId(id).evaluate((el) => Number(getComputedStyle(el).opacity))
const bg = (page) => page.getByTestId('display').evaluate((el) => getComputedStyle(el).backgroundColor)
const focused = (page) => page.evaluate(() => document.activeElement?.getAttribute('data-testid'))
/** The request dialog ignores input for 1 s after it appeared. */
const armed = (page) => page.waitForTimeout(1200)

let browser
try {
  browser = await launch()

  // ======================================================================================
  // 1. Setup wizard, idle screen, Settings, appearance, persistence
  // ======================================================================================
  {
    const s = await server()
    const ctx = await newContext(browser)
    const d = await ctx.newPage()
    await d.goto(`${s.base}/display`)
    await d.getByTestId('setup').waitFor()
    assert.equal(await d.getByTestId('wz-steps').count(), 0, 'no step dots on the welcome screen')
    assert.equal(await focused(d), 'wz-start')
    assert.equal(await bg(d), 'rgb(14, 22, 48)', 'Dark is the default (#0E1630)')
    // Focus ring rule (7.2): the ring is there on load; the first mouse input hides it; the next key press shows it again
    const ring = () => d.getByTestId('wz-start').evaluate((el) => getComputedStyle(el).outlineStyle)
    assert.equal(await ring(), 'solid', 'the focus ring shows on load')
    await shot(d, 'display-1280-dark-setup-0-welcome')
    await d.mouse.move(200, 200)
    await d.mouse.move(210, 205)
    await d.mouse.click(200, 200)
    assert.equal(await ring(), 'none', 'no ring after mouse input')
    await d.keyboard.press('ArrowRight')
    assert.equal(await ring(), 'solid', 'the ring is back after a key press')
    await d.getByTestId('wz-start').click()
    assert.equal(await focused(d), 'wz-next')
    await shot(d, 'display-1280-dark-setup-1-name')
    // Back goes one step back; on the welcome screen nothing happens
    await d.getByTestId('wz-back').click()
    await d.getByTestId('wz-start').waitFor()
    await d.keyboard.press('Escape')
    await d.getByTestId('wz-start').waitFor()
    await d.getByTestId('wz-start').click()
    // the first suggestion is selected and the custom field is empty; typing deselects the chips, a chip clears the field
    assert.equal(await d.getByTestId('wz-room').inputValue(), '', 'the custom field starts empty')
    assert.equal(await d.locator('.cd-chip[aria-pressed=true]').innerText(), 'Meeting room Elbe')
    await d.getByTestId('wz-room').fill('Annex')
    assert.equal(await d.locator('.cd-chip[aria-pressed=true]').count(), 0, 'typing deselects every chip')
    await d.getByRole('button', { name: 'Workshop' }).click()
    assert.equal(await d.getByTestId('wz-room').inputValue(), '', 'choosing a chip clears the field')
    assert.equal(await d.locator('.cd-chip[aria-pressed=true]').innerText(), 'Workshop')
    await d.keyboard.press('Escape') // Back key = one step back
    await d.getByTestId('wz-start').waitFor()
    await d.getByTestId('wz-start').click()
    await d.getByTestId('wz-next').click()
    // Step 2: Open is preselected (D1)
    assert.equal(await d.getByTestId('wz-mode-open').getAttribute('aria-checked'), 'true')
    assert.equal(await focused(d), 'wz-mode-open')
    await shot(d, 'display-1280-dark-setup-2-mode')
    await d.getByTestId('wz-mode-confirm').click()
    await d.getByTestId('wz-next').click()
    assert.equal((await d.getByTestId('wz-domain').innerText()).trim(), `127.0.0.1:${s.port}`)
    assert.equal(await focused(d), 'wz-ok')
    assert.equal((await d.getByTestId('wz-source').innerText()).trim(), 'Set by the server of this page.', 'the page\'s own server')
    await shot(d, 'display-1280-dark-setup-3-address')
    // "Other address" with one relay opens the field in place; Back cancels it, an address typed by hand says so
    await d.getByTestId('wz-change').click()
    assert.equal((await d.locator('label[for=wz-addr]').innerText()).trim(), 'Server address')
    assert.equal(await focused(d), 'wz-addr')
    await d.keyboard.press('Escape')
    await d.getByTestId('wz-domain').waitFor()
    await d.getByTestId('wz-change').click()
    await d.getByTestId('wz-addr').fill(`127.0.0.1:${s.port}`)
    await d.keyboard.press('Enter')
    assert.equal((await d.getByTestId('wz-source').innerText()).trim(), 'Entered by hand.')
    await shot(d, 'display-1280-dark-setup-3-address-typed')
    await d.getByTestId('wz-ok').click()
    assert.equal(await focused(d), 'wz-finish')
    await shot(d, 'display-1280-dark-setup-4-done')
    await d.getByTestId('wz-finish').click()

    await status(d, 'waiting_sender')
    const code1 = await readCode(d)
    assert.match(code1, /^[A-HJ-NP-Z2-9]{6}$/)
    assert.equal((await d.getByTestId('room-name').innerText()).trim(), 'Workshop')
    assert.equal((await d.getByTestId('mode-pill').innerText()).trim(), 'Approval on')
    assert.equal((await d.getByTestId('address').innerText()).trim(), `127.0.0.1:${s.port}`)
    assert.equal(await d.locator('[data-testid=qr] svg').getAttribute('aria-label'), `QR: http://127.0.0.1:${s.port}/${code1}`)
    assert.equal(await focused(d), 'settings-open', 'on idle the focus starts on the Settings button')
    // no store or download link exists yet: the footer names no store
    assert.equal((await d.getByTestId('idle-foot').innerText()).trim(), 'No download needed.')
    await shot(d, 'display-1280-dark-idle')

    // The wizard is gone after a restart; room and default mode are kept
    await d.reload()
    await status(d, 'waiting_sender')
    assert.equal(await d.getByTestId('setup').count(), 0)
    assert.equal((await d.getByTestId('room-name').innerText()).trim(), 'Workshop')
    assert.equal((await d.getByTestId('mode-pill').innerText()).trim(), 'Approval on')

    // --- Settings
    await d.getByTestId('settings-open').click()
    await d.getByTestId('settings-modal').waitFor()
    assert.equal(await focused(d), 'settings-mode-confirm', 'initial focus: the tile of the current join mode')
    assert.equal(await d.getByTestId('settings-mode-confirm').getAttribute('aria-checked'), 'true')
    assert.equal((await d.getByTestId('settings-mode-desc').innerText()).trim(), 'You let each person in one by one.')
    await shot(d, 'display-1280-dark-settings')
    // arrows move in rows: Down goes to the same column of the next row, Up back, no wrap
    await d.keyboard.press('ArrowDown')
    assert.equal(await focused(d), 'settings-look-light')
    await d.keyboard.press('ArrowDown')
    assert.equal(await focused(d), 'settings-look-light', 'no wrap at the last row')
    await d.keyboard.press('ArrowUp')
    await d.keyboard.press('ArrowUp')
    assert.equal(await focused(d), 'settings-close')
    await d.keyboard.press('Escape')
    await d.getByTestId('settings-modal').waitFor({ state: 'detached' })
    assert.equal(await focused(d), 'settings-open', 'the focus returns to the Settings button')
    await d.keyboard.press('m') // Menu
    await d.getByTestId('settings-modal').waitFor()
    // Open: applied at once, dialog stays open, and it becomes the default for the next start
    await d.getByTestId('settings-mode-open').click()
    await d.locator('[data-testid=mode-pill]:has-text("Open")').waitFor()
    assert.equal(await d.getByTestId('settings-modal').count(), 1)
    // Appearance: Light, OLED black, Automatic
    await d.getByTestId('settings-look-light').click()
    assert.equal(await d.getByTestId('display').getAttribute('data-theme'), 'light')
    assert.equal(await bg(d), 'rgb(255, 255, 255)')
    await shot(d, 'display-1280-light-settings')
    await d.getByTestId('settings-look-oled').click()
    assert.equal(await d.getByTestId('display').getAttribute('data-theme'), 'oled')
    assert.equal(await bg(d), 'rgb(0, 0, 0)', 'OLED black is #000000')
    // every surface is black, separated only by borders (--panel and --panel2 are #000000)
    assert.equal(await d.getByTestId('settings-mode-locked').evaluate((el) => getComputedStyle(el).backgroundColor), 'rgb(0, 0, 0)')
    assert.equal(await d.locator('.cd-wz').evaluate((el) => getComputedStyle(el).backgroundColor), 'rgb(0, 0, 0)')
    // a selected tile: black fill (no tint), amber border (--acc #D29E48)
    for (const id of ['settings-mode-open', 'settings-look-oled']) {
      const st = await d.getByTestId(id).evaluate((el) => [getComputedStyle(el).backgroundColor, getComputedStyle(el).borderTopColor])
      assert.deepEqual(st, ['rgb(0, 0, 0)', 'rgb(210, 158, 72)'], `${id}: selected, black with an amber border`)
    }
    await shot(d, 'display-1280-oled-settings')
    await d.getByTestId('settings-look-auto').click()
    await d.emulateMedia({ colorScheme: 'light' })
    await waitFor(async () => (await d.getByTestId('display').getAttribute('data-theme')) === 'light', 'Automatic follows a light system')
    await d.emulateMedia({ colorScheme: 'dark' })
    await waitFor(async () => (await d.getByTestId('display').getAttribute('data-theme')) === 'dark', 'Automatic follows a dark system')
    await d.getByTestId('settings-look-oled').click()
    await d.getByTestId('settings-close').click()
    await shot(d, 'display-1280-oled-idle')
    // Locked: code at 35 %, the note, QR caption
    await d.getByTestId('settings-open').click()
    await d.getByTestId('settings-mode-locked').click()
    await d.locator('[data-testid=mode-pill]:has-text("Locked")').waitFor()
    await d.getByTestId('settings-close').click()
    assert.equal((await d.getByTestId('locked-note').innerText()).trim(), 'Locked right now.')
    assert.equal((await d.getByTestId('qr-caption').innerText()).trim(), 'Locked')
    await shot(d, 'display-1280-oled-idle-locked')

    // appearance and default mode survive a reload; Locked does not
    await d.reload()
    await status(d, 'waiting_sender')
    assert.equal(await d.getByTestId('display').getAttribute('data-theme'), 'oled', 'appearance is kept on this device')
    assert.equal(await d.evaluate(() => localStorage.getItem('cometcast.display.appearance')), 'oled')
    assert.equal((await d.getByTestId('mode-pill').innerText()).trim(), 'Open', 'the default mode applies again; Locked is not remembered')
    await ctx.close()
  }

  // ======================================================================================
  // 2. Approval: request -> Yes -> presenting full bleed -> reveal on input -> End -> new code
  // ======================================================================================
  {
    const s = await server()
    const dctx = await newContext(browser)
    const d = await dctx.newPage()
    await mockWakeLock(d)
    const code = await openDisplay(d, s.base, { room: 'Room 204', mode: 'confirm' })
    assert.deepEqual(await d.evaluate(() => window.__wake), [], 'no wake lock while idle (D5)')

    const pctx = await newContext(browser, { colorScheme: 'light' })
    const p = await pctx.newPage()
    await presenterStart(p, s.base, code, { name: 'Lena' })
    await p.getByTestId('waiting-page').waitFor()
    const check = (await p.getByTestId('waiting-check').innerText()).trim()
    assert.match(check, /^\d{2}$/)
    assert.match(await p.getByTestId('waiting-left').innerText(), /expires in \d+ s/)
    // the display's room name came through the protocol; the code is not shown in its place
    assert.equal((await p.getByTestId('waiting-room').innerText()).trim(), 'Room 204')
    assert.equal((await p.getByTestId('waiting-page').innerText()).includes(code), false)
    await shot(p, 'presenter-1280-light-waiting')

    // the request over idle
    await d.getByTestId('request-modal').waitFor()
    assert.equal((await d.getByTestId('request-name').innerText()).trim(), '“Lena”')
    assert.equal((await d.getByTestId('request-check').innerText()).trim(), check, 'same check number on both sides')
    assert.equal(await d.getByRole('alertdialog').getAttribute('aria-label'), 'Request: “Lena”')
    assert.equal(await focused(d), 'request-yes')
    await shot(d, 'display-1280-dark-request')
    // a press right after it appeared answers nothing (1 s guard)
    await d.getByTestId('request-yes').click()
    await d.waitForTimeout(150)
    assert.equal(await d.getByTestId('request-modal').count(), 1, 'ignored during the guard')
    await armed(d)
    await d.getByTestId('request-yes').click()
    await d.getByTestId('request-modal').waitFor({ state: 'detached' })
    assert.equal(await focused(d), 'settings-open', 'the focus goes back to the Settings button')

    await p.getByTestId('what-page').waitFor({ timeout: 20000 })
    assert.equal((await p.getByTestId('connected-to').innerText()).trim(), 'Connected to “Room 204”')
    assert.equal(await p.getByTestId('sas-line').count(), 0, 'no security code on "What to show"')
    await shot(p, 'presenter-1280-light-what')
    await p.getByTestId('what-screen').click()
    await p.getByTestId('presenting-page').waitFor({ timeout: 20000 })
    assert.equal((await p.getByTestId('presenting-where').innerText()).trim(), 'On “Room 204”.')
    const presenterSas = await waitFor(async () => (await p.getByTestId('sas-code').innerText()).trim(), 'the security code on the presenter page')
    assert.match((await p.getByTestId('sas-line').innerText()).trim(), /^Security code \d{3} \d{3}\. The screen shows the same code under your name\.$/)
    await shot(p, 'presenter-1280-light-presenting')

    // presenting: full bleed, nothing else at rest, the screen kept awake
    await status(d, 'receiving')
    const video = d.getByTestId('remote-video')
    await waitFor(() => video.evaluate((v) => v.videoWidth > 0 && v.currentTime > 0), 'video plays')
    const box = await d.getByTestId('stage').boundingBox()
    assert.deepEqual([box.x, box.y, box.width, box.height], [0, 0, 1280, 720], 'the picture fills the screen edge to edge')
    assert.equal(await d.getByTestId('idle').isVisible(), false, 'no code, room name or mode while presenting')
    await waitFor(async () => (await opacity(d, 'byline')) === 0, 'controls hidden at rest')
    assert.equal(await opacity(d, 'toolbar-root'), 0, 'the annotation toolbar is hidden at rest too')
    assert.deepEqual(await d.evaluate(() => window.__wake), ['acquire:screen'], 'wake lock acquired when presenting starts')
    await shot(d, 'display-1280-dark-presenting-rest')

    // input reveals the name tag and End (400 ms fade), they hide again 5 s after the last input
    await d.mouse.move(400, 300)
    await d.mouse.move(420, 310)
    await waitFor(async () => (await opacity(d, 'byline')) === 1, 'name tag fades in')
    assert.match((await d.getByTestId('byline').innerText()), /“Lena” is showing/)
    assert.equal((await d.getByTestId('end').innerText()).trim(), 'End')
    await waitFor(async () => (await opacity(d, 'toolbar-root')) === 1, 'the toolbar fades in with the name tag')
    // second line of the name tag: the same security code as on the presenter page
    assert.equal((await d.getByTestId('sas-code').innerText()).trim(), presenterSas)
    assert.match((await d.getByTestId('sas-line').innerText()).trim(), /^Security code \d{3} \d{3}$/)
    await shot(d, 'display-1280-dark-presenting-controls')
    await waitFor(async () => (await opacity(d, 'byline')) === 0, 'controls hide again', 9000)
    // the first key press only reveals (and focuses End); the second one activates it
    await d.keyboard.press('ArrowRight')
    await waitFor(async () => (await opacity(d, 'byline')) === 1, 'a key reveals')
    assert.equal(await focused(d), 'end')
    assert.equal(await d.getByTestId('stage').isVisible(), true, 'the first key only reveals')
    // Back hides, a click anywhere shows again
    await d.keyboard.press('Escape')
    await waitFor(async () => (await opacity(d, 'byline')) === 0, 'Back hides the controls')
    await d.mouse.click(500, 300)
    await waitFor(async () => (await opacity(d, 'byline')) === 1, 'a click reveals')

    // End: new code, back to the idle screen, toast, wake lock released, presenter told
    await d.getByTestId('end').click()
    await d.getByTestId('idle').waitFor({ state: 'visible' })
    await waitFor(async () => (await readCode(d)) !== code, 'a new code after End')
    assert.equal((await d.getByTestId('toast').innerText()).trim(), 'Session ended · new code')
    assert.equal(await d.evaluate(() => window.__wakeHeld), 0, 'wake lock released')
    assert.deepEqual(await d.evaluate(() => window.__wake), ['acquire:screen', 'release'])
    await p.getByTestId('error-page').waitFor()
    assert.equal((await p.getByTestId('error-title').innerText()).trim(), 'Presentation ended')
    await shot(p, 'presenter-1280-light-error-ended')

    // Stop showing returns to "What to show" and leaves the display on idle with the same code
    const code2 = await readCode(d)
    await presenterStart(p, s.base, code2, { name: 'Lena' })
    await d.getByTestId('request-modal').waitFor()
    await armed(d)
    await d.getByTestId('request-yes').click()
    await p.getByTestId('what-page').waitFor({ timeout: 20000 })
    await p.getByTestId('what-screen').click()
    await p.getByTestId('presenting-page').waitFor()
    await waitFor(() => d.getByTestId('stage').isVisible().then((v) => v && d.getByTestId('idle').isVisible().then((i) => !i)), 'presenting again')
    await p.getByTestId('presenting-stop').click()
    await p.getByTestId('what-page').waitFor()
    await d.getByTestId('idle').waitFor({ state: 'visible', timeout: 10000 })
    assert.equal(await readCode(d), code2, 'the same code after the presenter stopped')
    await waitFor(() => d.evaluate(() => window.__wakeHeld === 0), 'wake lock released when the presenter stops')
    await p.getByTestId('disconnect').click()
    await p.getByTestId('bye-page').waitFor()
    assert.equal((await p.getByTestId('bye-page').innerText()).includes('Disconnected'), true)
    await waitFor(async () => (await d.getByTestId('toast').innerText()).includes('disconnected'), 'toast: disconnected')
    await dctx.close()
    await pctx.close()
  }

  // ======================================================================================
  // 3. No, No and block, the queue
  // ======================================================================================
  {
    const s = await server()
    const dctx = await newContext(browser)
    const d = await dctx.newPage()
    const code = await openDisplay(d, s.base, { mode: 'confirm' })
    const mkp = async () => {
      const c = await newContext(browser)
      return { c, p: await c.newPage() }
    }
    // two requests: "+1 waiting", the next one takes the dialog after an answer, the guard starts again
    const a = await mkp()
    const b = await mkp()
    await presenterStart(a.p, s.base, code, { name: 'Anna' })
    await d.getByTestId('request-modal').waitFor()
    await presenterStart(b.p, s.base, code, { name: 'Ben' })
    await d.getByTestId('request-queue').waitFor()
    assert.equal((await d.getByTestId('request-queue').innerText()).trim(), '+1 waiting')
    assert.equal((await d.getByTestId('request-name').innerText()).trim(), '“Anna”', 'first come, first served')
    await shot(d, 'display-1280-dark-request-queue')
    await armed(d)
    await d.getByTestId('request-no').click()
    await waitFor(async () => (await d.getByTestId('request-name').innerText()).trim() === '“Ben”', 'the next request')
    assert.equal(await d.getByTestId('request-queue').count(), 0)
    assert.equal(await focused(d), 'request-yes')
    await d.getByTestId('request-yes').click() // guard again: ignored
    await d.waitForTimeout(150)
    assert.equal(await d.getByTestId('request-modal').count(), 1)
    assert.equal((await d.getByTestId('toast').innerText()).trim(), '“Anna” denied')
    // Anna: "Not allowed" with a countdown on "Ask again"
    await a.p.getByTestId('error-page').waitFor()
    assert.equal((await a.p.getByTestId('error-title').innerText()).trim(), 'Not allowed')
    assert.match((await a.p.getByTestId('error-ask').innerText()).trim(), /^Ask again in \d+ s$/)
    assert.equal(await a.p.getByTestId('error-ask').isDisabled(), true)
    await shot(a.p, 'presenter-1280-light-error-denied')
    // Ben: No and block; his later attempts are not shown on the display and he sees "Not allowed"
    await armed(d)
    await d.getByTestId('request-block').click()
    await d.getByTestId('request-modal').waitFor({ state: 'detached' })
    assert.equal((await d.getByTestId('toast').innerText()).trim(), '“Ben” denied and blocked')
    await b.p.getByTestId('error-page').waitFor()
    assert.equal((await b.p.getByTestId('error-title').innerText()).trim(), 'Not allowed')
    const c2 = await mkp()
    await presenterStart(c2.p, s.base, code, { name: 'Ben again' })
    await c2.p.getByTestId('error-page').waitFor()
    assert.equal((await c2.p.getByTestId('error-title').innerText()).trim(), 'Not allowed', 'a blocked address is refused at once')
    await d.waitForTimeout(1500)
    assert.equal(await d.getByTestId('request-modal').count(), 0, 'the blocked attempt is not shown on the display')
    for (const x of [a, b, c2]) await x.c.close()
    await dctx.close()
  }

  // ======================================================================================
  // 4. Withdrawing, Locked, a wrong code
  // ======================================================================================
  {
    const s = await server()
    const dctx = await newContext(browser)
    const d = await dctx.newPage()
    const code = await openDisplay(d, s.base, { mode: 'confirm' })
    const mkp = async (opts) => {
      const c = await newContext(browser, opts)
      return { c, p: await c.newPage() }
    }
    // withdraw: the request leaves the display, toast
    const w = await mkp()
    await presenterStart(w.p, s.base, code, { name: 'Wren' })
    await d.getByTestId('request-modal').waitFor()
    await w.p.getByTestId('withdraw').click()
    await w.p.getByTestId('start-page').waitFor()
    await d.getByTestId('request-modal').waitFor({ state: 'detached' })
    await waitFor(async () => (await d.getByTestId('toast').innerText()).includes('withdrew the request'), 'toast: withdrew')
    // The Menu key does nothing while a request is open; Settings (and so Locked) opens over idle only
    const x = await mkp()
    await presenterStart(x.p, s.base, code, { name: 'Xia' })
    await d.getByTestId('request-modal').waitFor()
    await d.keyboard.press('m')
    assert.equal(await d.getByTestId('settings-modal').count(), 0)
    await armed(d)
    await d.getByTestId('request-no').click()
    await d.getByTestId('request-modal').waitFor({ state: 'detached' })
    // A request closes Settings (one dialog at a time), the focus goes to Yes, and back to the Settings button after it
    await d.getByTestId('settings-open').click()
    await d.getByTestId('settings-modal').waitFor()
    const y = await mkp()
    await presenterStart(y.p, s.base, code, { name: 'Yan' })
    await d.getByTestId('request-modal').waitFor()
    assert.equal(await d.getByTestId('settings-modal').count(), 0, 'only the request dialog is visible')
    assert.equal(await focused(d), 'request-yes')
    await armed(d)
    await d.getByTestId('request-no').click()
    await d.getByTestId('request-modal').waitFor({ state: 'detached' })
    assert.equal(await d.getByTestId('settings-modal').count(), 0)
    assert.equal(await focused(d), 'settings-open', 'the focus goes back to the Settings button')
    await d.getByTestId('settings-open').click()
    await d.getByTestId('settings-mode-locked').click()
    await d.locator('[data-testid=mode-pill]:has-text("Locked")').waitFor()
    await d.getByTestId('settings-close').click()
    // a new presenter: "Locked right now", the code was right
    const z = await mkp()
    await presenterStart(z.p, s.base, code, { name: 'Zed' })
    await z.p.getByTestId('error-page').waitFor()
    assert.equal((await z.p.getByTestId('error-title').innerText()).trim(), 'Locked right now')
    assert.match(await z.p.getByTestId('error-why').innerText(), /not taking anyone new/)
    // a wrong code
    const q = await mkp()
    await presenterStart(q.p, s.base, 'ZZZZZZ')
    await q.p.getByTestId('error-page').waitFor()
    assert.equal((await q.p.getByTestId('error-title').innerText()).trim(), 'This code does not exist right now')
    await q.p.getByTestId('error-retype').click()
    await q.p.getByTestId('start-code').waitFor()
    for (const m of [w, x, y, z, q]) await m.c.close()
    await dctx.close()
  }

  // ======================================================================================
  // 5. Link: presenting a link keeps the screen awake; Stop showing releases it
  // ======================================================================================
  {
    const s = await server()
    const dctx = await newContext(browser)
    const d = await dctx.newPage()
    await mockWakeLock(d)
    const code = await openDisplay(d, s.base) // Open (default)
    const pctx = await newContext(browser)
    const p = await pctx.newPage()
    await presenterLinkPage(p, s.base, code, { name: 'Lena' })
    await shot(p, 'presenter-1280-light-link')
    await p.getByTestId('link-url').fill('https://example.org/path')
    await p.getByTestId('link-go').click()
    await p.getByTestId('presenting-page').waitFor()
    assert.equal((await p.getByTestId('presenting-title').innerText()).trim(), 'The screen shows your link')
    await d.getByTestId('stage').waitFor({ state: 'visible' })
    await waitFor(() => d.evaluate(() => window.__wake.includes('acquire:screen')), 'wake lock for a link')
    await d.mouse.move(100, 100)
    assert.match(await d.getByTestId('byline').innerText(), /“Lena” is showing/)
    await p.getByTestId('presenting-stop').click()
    await p.getByTestId('what-page').waitFor()
    await d.getByTestId('idle').waitFor({ state: 'visible' })
    await waitFor(() => d.evaluate(() => window.__wakeHeld === 0), 'released')
    await dctx.close()
    await pctx.close()
  }

  // ======================================================================================
  // 6. Idle dimming (a build with a short delay; the normal build has none)
  // ======================================================================================
  {
    const s = await server({ webDir: dimDist })
    const dctx = await newContext(browser)
    const d = await dctx.newPage()
    await openDisplay(d, s.base, { mode: 'confirm' })
    assert.equal(await opacity(d, 'dim'), 0)
    await waitFor(async () => (await opacity(d, 'dim')) > 0.55, 'the idle screen dims to 40 % brightness', 8000)
    await shot(d, 'display-1280-dark-idle-dimmed')
    // the first key press only restores; the next one works
    await d.keyboard.press('m')
    await waitFor(async () => (await opacity(d, 'dim')) < 0.1, 'brightness restored', 2000)
    assert.equal(await d.getByTestId('settings-modal').count(), 0, 'the first key only restores')
    await d.keyboard.press('m')
    await d.getByTestId('settings-modal').waitFor()
    await d.keyboard.press('Escape')
    // mouse movement also restores, and the countdown starts again
    await waitFor(async () => (await opacity(d, 'dim')) > 0.55, 'dims again', 8000)
    await d.mouse.move(300, 300)
    await waitFor(async () => (await opacity(d, 'dim')) < 0.1, 'restored by the mouse', 2000)
    // a join request restores it as well
    await waitFor(async () => (await opacity(d, 'dim')) > 0.55, 'dims once more', 8000)
    const code = await readCode(d)
    const pctx = await newContext(browser)
    const p = await pctx.newPage()
    await presenterStart(p, s.base, code, { name: 'Lena' })
    await d.getByTestId('request-modal').waitFor()
    await waitFor(async () => (await opacity(d, 'dim')) < 0.1, 'restored by a request', 2000)
    // no URL parameter shortens the delay in any build
    await dctx.close()
    await pctx.close()
  }

  // ======================================================================================
  // 7. Presenter pages: start, QR path, German, phone, dark
  // ======================================================================================
  {
    const s = await server()
    const dctx = await newContext(browser)
    const d = await dctx.newPage()
    const code = await openDisplay(d, s.base)
    // start page, typed
    const pctx = await newContext(browser, { colorScheme: 'light' })
    const p = await pctx.newPage()
    await p.goto(s.base)
    await p.getByTestId('start-page').waitFor()
    assert.equal(await p.getByTestId('start-code').getAttribute('placeholder'), 'XXX XXX')
    await shot(p, 'presenter-1280-light-start')
    await p.getByTestId('start-browser').click()
    assert.equal((await p.getByTestId('start-code-error').innerText()).trim(), 'Please enter the code shown large on the screen.')
    assert.match(await p.getByTestId('detected').innerText(), /^Detected: Laptop, /)
    // no download link exists yet: the laptop card ends after the detected line, no app button, no "or"
    assert.equal(await p.getByTestId('start-app').count(), 0)
    assert.equal(await p.getByTestId('start-or').count(), 0)
    // "be the screen itself": {dom}/display is a visible link to /display in the same tab
    const link = p.getByTestId('be-display-link')
    assert.equal(await link.getAttribute('href'), '/display')
    assert.equal((await link.innerText()).trim(), `127.0.0.1:${s.port}/display`)
    assert.equal(await link.evaluate((el) => getComputedStyle(el).textDecorationLine), 'underline')
    await p.keyboard.press('Shift') // keyboard input, so that the browser shows the ring for a scripted focus
    await link.focus()
    assert.notEqual(await link.evaluate((el) => getComputedStyle(el).outlineStyle), 'none', 'the link shows the focus outline')
    // the code field shows every typed character; the wrong code is the error page, no server call
    await p.getByTestId('start-code').fill('k7o m2w')
    assert.equal(await p.getByTestId('start-code').inputValue(), 'K7O M2W')
    await p.getByTestId('start-browser').click()
    await p.getByTestId('error-page').waitFor()
    assert.equal((await p.getByTestId('error-why').innerText()).trim(), 'Usually a typo. A code has 6 characters and never contains O, 0, I or 1.')
    await shot(p, 'presenter-1280-light-error-code')
    await p.getByTestId('error-retype').click()
    // the QR path: the code is filled in
    await p.goto(`${s.base}/${code}`)
    assert.equal((await p.getByTestId('start-code-chip').innerText()).replace(/\s+/g, ''), code)
    assert.match(await p.getByTestId('code-chip').innerText(), /from the QR code/)
    await p.getByTestId('start-browser').click()
    await p.getByTestId('what-page').waitFor({ timeout: 20000 })
    await p.getByTestId('disconnect').click()
    // dark presenter, German (Sie)
    const gctx = await newContext(browser, { colorScheme: 'dark', locale: 'de-DE' })
    const g = await gctx.newPage()
    await g.goto(`${s.base}/${code}`)
    assert.equal((await g.getByTestId('start-page').locator('h1').innerText()).trim(), 'Mit einem Bildschirm verbinden')
    assert.equal((await g.locator('label[for=start-name]').innerText()).trim(), 'Ihr Name')
    assert.equal(await g.locator('.cp').evaluate((el) => getComputedStyle(el).backgroundColor), 'rgb(20, 23, 29)', 'dark presenter page (#14171D)')
    await shot(g, 'presenter-1280-dark-start-de')
    // phone: Android, Google Play wording, no screen sharing in the browser
    const phone = await newContext(browser, {
      viewport: { width: 390, height: 844 },
      isMobile: true,
      hasTouch: true,
      colorScheme: 'light',
      userAgent: 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36',
    })
    const m = await phone.newPage()
    await m.goto(`${s.base}/${code}`)
    assert.equal(await m.getByTestId('start-app').count(), 0, 'no Google Play link exists yet: no app button')
    assert.equal(await m.getByTestId('start-or').count(), 0)
    assert.equal((await m.getByTestId('detected').innerText()).trim(), 'Detected: Android phone, Chrome')
    assert.equal(await m.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), true, 'no horizontal scrolling on a phone')
    await shot(m, 'presenter-390-light-start')
    await m.getByTestId('start-browser').click()
    await m.getByTestId('what-page').waitFor({ timeout: 20000 })
    // No store listing exists yet: the row does not promise an app (spec 5.5)
    assert.match(await m.getByTestId('what-screen').innerText(), /Not possible in a phone browser/)
    await shot(m, 'presenter-390-light-what')
    await m.getByTestId('what-screen').click()
    assert.equal((await m.getByTestId('error-title').innerText()).trim(), 'Screen sharing does not work in this browser')
    await shot(m, 'presenter-390-light-error-noscreen')
    await m.getByTestId('error-showLink').click()
    await m.getByTestId('link-page').waitFor()
    for (const x of [dctx, pctx, gctx, phone]) await x.close()
  }

  // ======================================================================================
  // 8. Fix round 2: chip comes back, reconnect schedule, closed share dialog, black screen ends with the presentation
  // ======================================================================================
  {
    // The custom name field emptied again: the chip chosen before typing is selected again (decision 13.39).
    const s = await server()
    const ctx = await newContext(browser)
    const d = await ctx.newPage()
    await d.goto(`${s.base}/display`)
    await d.getByTestId('wz-start').click()
    await d.locator('.cd-chip', { hasText: 'Workshop' }).click()
    await d.getByTestId('wz-room').fill('X')
    assert.equal(await d.locator('.cd-chip.on').count(), 0)
    await d.getByTestId('wz-room').fill('')
    assert.deepEqual(await d.locator('.cd-chip.on').allInnerTexts(), ['Workshop'])
    // The card is at the same place in every step (top edge 64 reference px), the welcome card is centred
    const top = async () => (await d.locator('.cd-wz').boundingBox()).y
    const y1 = await top()
    await d.getByTestId('wz-next').click()
    assert.ok(Math.abs((await top()) - y1) < 1, 'the card does not jump between steps 1 and 2')
    await d.getByTestId('wz-next').click()
    assert.ok(Math.abs((await top()) - y1) < 1, 'nor between steps 2 and 3')
    await ctx.close()
  }
  {
    // A drop under 3 s never shows the box (retries at 1 s, 2 s, 3 s), checked by a series every 250 ms.
    const first = await server()
    const ctx = await newContext(browser)
    const d = await ctx.newPage()
    await openDisplay(d, first.base)
    first.proc.kill('SIGTERM')
    await d.locator('[data-testid=qr][data-empty=true]').waitFor({ timeout: 5000 })
    await new Promise((r) => setTimeout(r, 400))
    const back = startServer({ port: first.port })
    servers.push(back)
    await waitHealthy(back.base)
    const seen = []
    for (let i = 0; i < 16; i++) {
      seen.push(await d.getByTestId('offline').count())
      await d.waitForTimeout(250)
    }
    assert.ok(seen.every((n) => n === 0), `the offline box never showed while the server was away for about a second: ${seen}`)
    await readCode(d)
    await ctx.close()
  }
  {
    // The share dialog is closed without a choice: back on "What to show", no error page.
    const s = await server()
    const dctx = await newContext(browser)
    const d = await dctx.newPage()
    const code = await openDisplay(d, s.base)
    const pctx = await newContext(browser)
    await pctx.addInitScript(() => {
      navigator.mediaDevices.getDisplayMedia = () => Promise.reject(new DOMException('closed', 'NotAllowedError'))
    })
    const p = await pctx.newPage()
    await presenterJoin(p, s.base, code, { name: 'Lena' })
    await p.getByTestId('what-screen').click()
    await p.waitForTimeout(500)
    assert.equal(await p.getByTestId('error-page').count(), 0, 'no error page')
    assert.equal(await p.getByTestId('what-page').count(), 1, 'back on "What to show"')
    await dctx.close()
    await pctx.close()
  }
  {
    // The black screen ends with the presentation: stop, show again, the picture is visible.
    const s = await server()
    const dctx = await newContext(browser)
    const d = await dctx.newPage()
    const code = await openDisplay(d, s.base)
    const pctx = await newContext(browser)
    const p = await pctx.newPage()
    await presenterScreen(p, s.base, code, { name: 'Lena' })
    await status(d, 'receiving')
    await d.waitForFunction(() => document.querySelector('[data-testid=remote-video]')?.videoWidth > 0)
    await d.keyboard.press('ArrowRight')
    await d.getByTestId('tb-black').click()
    await d.getByTestId('black-screen').waitFor()
    await p.getByTestId('presenting-stop').click()
    await d.getByTestId('idle').waitFor()
    await p.getByTestId('what-screen').click()
    await p.getByTestId('presenting-page').waitFor({ timeout: 20000 })
    await d.waitForFunction(() => document.querySelector('[data-testid=remote-video]')?.videoWidth > 0)
    assert.equal(await d.getByTestId('black-screen').count(), 0, 'the next presentation starts visible')
    await dctx.close()
    await pctx.close()
  }

  console.log('E2E DISPLAY OK')
} finally {
  await browser?.close()
  for (const s of servers) s.proc.kill('SIGTERM')
  rmSync(tmp, { recursive: true, force: true })
}
