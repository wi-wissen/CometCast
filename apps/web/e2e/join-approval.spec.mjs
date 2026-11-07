// SPDX-License-Identifier: GPL-3.0-or-later
//
// End-to-end test of join approval with the display's request dialog (docs/design/first-run-and-display.md,
// 4.4): real server binary + real Chromium pages (a display and several presenters). Covers: open
// (the default), approval with the check number and a plain-text name, the 1 s input guard, Yes, No,
// No and block, keyboard-only answers, Locked and unlocking, a request nobody answers (timeout), withdrawing,
// the German texts (du), End (new code) and that the default mode is kept on the display device.
// Run: npm run build && E2E_CHROME=/usr/bin/google-chrome npm run e2e:join
//
// Presenters join without showing anything (no screen capture): the test is about the join.
import assert from 'node:assert/strict'
import { launch, newContext, openDisplay, presenterStart, startServer, status, waitFor, waitHealthy } from './helpers.mjs'

const APPROVAL_SECS = 5 // short, so that the timeout case is quick
const server = startServer({ args: ['--set', `join_approval_secs=${APPROVAL_SECS}`] })
const { base } = server
let browser
try {
  await waitHealthy(base)
  browser = await launch()
  const rctx = await newContext(browser)
  const receiver = await rctx.newPage()
  const code = await openDisplay(receiver, base) // first run: Open
  console.log('room code:', code)

  const focused = () => receiver.evaluate(() => document.activeElement?.getAttribute('data-testid'))
  const settings = async (fn) => {
    await receiver.getByTestId('settings-open').click()
    await receiver.getByTestId('settings-modal').waitFor()
    await fn()
    await receiver.getByTestId('settings-close').click()
    await receiver.getByTestId('settings-modal').waitFor({ state: 'detached' })
  }
  const setMode = (mode) =>
    settings(async () => {
      await receiver.getByTestId(`settings-mode-${mode}`).click()
      await receiver.locator(`[data-testid=settings-mode-${mode}][aria-checked=true]`).waitFor({ timeout: 5000 })
    })
  // The request dialog ignores input for 1 s after it appeared.
  const armed = () => receiver.waitForTimeout(1200)
  const newPresenter = async (name, locale = 'en-US') => {
    const ctx = await newContext(browser, { locale, viewport: { width: 1000, height: 800 } })
    return ctx.newPage()
  }
  const connect = (page, name, c = code) => presenterStart(page, base, c, { name })
  const phase = (page, p, timeout = 10000) => status(page, p, timeout)
  const errorTitle = async (page) => (await page.getByTestId('error-title').innerText()).trim()

  // --- 1. First run: the default is Open
  await receiver.getByTestId('settings-open').click()
  assert.equal(await receiver.getByTestId('settings-mode-open').getAttribute('aria-checked'), 'true', 'default mode is Open')
  assert.equal(await receiver.getByTestId('settings-mode-confirm').getAttribute('aria-checked'), 'false')
  await receiver.getByTestId('settings-close').click()

  // --- 2. Open: a presenter joins at once; the display tells the host
  const s1 = await newPresenter('Ana')
  await connect(s1, 'Ana')
  await s1.getByTestId('what-page').waitFor({ timeout: 15000 })
  await status(receiver, 'receiving', 15000)
  assert.equal(await receiver.getByTestId('request-modal').count(), 0, 'no request in open mode')
  await waitFor(async () => /“Ana” connected/.test(await receiver.getByTestId('toast').innerText()), 'toast: connected')
  await s1.getByTestId('disconnect').click()
  await status(receiver, 'peer_left', 10000)
  console.log('open OK')

  // --- 3. Approval on: waiting page with the check number; the request dialog; guard; Yes
  await setMode('confirm')
  const s2 = await newPresenter()
  await connect(s2, 'Bo <b>x</b>')
  await s2.getByTestId('waiting-page').waitFor()
  const check = (await s2.getByTestId('waiting-check').innerText()).trim()
  assert.match(check, /^[1-9][0-9]$/)
  await receiver.getByTestId('request-modal').waitFor({ timeout: 5000 })
  assert.equal(await receiver.getByTestId('request-modal').locator('[role=alertdialog]').count(), 1)
  assert.equal((await receiver.getByTestId('request-check').innerText()).trim(), check, 'same check number on both sides')
  assert.equal(await receiver.locator('[data-testid=request-modal] b, [data-testid=request-modal] img').count(), 0, 'the name is plain text')
  assert.equal((await receiver.getByTestId('request-name').innerText()).trim(), '“Bo <b>x</b>”')
  assert.equal(await focused(), 'request-yes', 'Yes has the focus')
  await receiver.getByTestId('request-yes').click() // inside the guard
  await receiver.waitForTimeout(150)
  assert.equal(await receiver.getByTestId('request-modal').count(), 1, 'a press right after the dialog appeared answers nothing')
  await armed()
  await receiver.getByTestId('request-yes').click()
  await s2.getByTestId('what-page').waitFor({ timeout: 15000 })
  await receiver.getByTestId('request-modal').waitFor({ state: 'detached', timeout: 5000 })
  await waitFor(async () => /“Bo <b>x<\/b>” allowed/.test(await receiver.getByTestId('toast').innerText()), 'toast: allowed')
  console.log('approval + guard + allow OK')

  // --- 4. The presenter leaves; No (one press): told, and has to wait before asking again
  await s2.getByTestId('disconnect').click()
  await status(receiver, 'peer_left', 10000)
  const s3 = await newPresenter()
  await connect(s3, 'Cy')
  await receiver.getByTestId('request-modal').waitFor()
  await armed()
  await receiver.getByTestId('request-no').click()
  await s3.getByTestId('error-page').waitFor()
  assert.equal(await errorTitle(s3), 'Not allowed')
  assert.match(await s3.getByTestId('error-ask').innerText(), /^Ask again in \d+ s$/)
  assert.equal(await s3.getByTestId('error-ask').isDisabled(), true, 'cannot ask again at once')
  assert.equal(await receiver.getByTestId('request-modal').count(), 0)
  console.log('deny OK')

  // --- 5. Keyboard only: arrows move the focus, Enter answers (after the guard)
  const s4 = await newPresenter()
  await connect(s4, 'Di')
  await receiver.getByTestId('request-modal').waitFor()
  await armed()
  await receiver.keyboard.press('ArrowRight') // Yes -> No
  assert.equal(await focused(), 'request-no')
  await receiver.keyboard.press('Enter')
  await s4.getByTestId('error-page').waitFor()
  const s5 = await newPresenter()
  await connect(s5, 'Ed')
  await receiver.getByTestId('request-modal').waitFor()
  await armed()
  assert.equal(await focused(), 'request-yes', 'the focus is back on Yes for the next request')
  await receiver.keyboard.press('Enter')
  await s5.getByTestId('what-page').waitFor({ timeout: 15000 })
  console.log('keyboard OK')

  // --- 6. Locked: later joins hear "locked"; Open again lets them in
  await s5.getByTestId('disconnect').click()
  await status(receiver, 'peer_left', 10000)
  await setMode('locked')
  const s7 = await newPresenter()
  await connect(s7, 'Gus')
  await s7.getByTestId('error-page').waitFor()
  assert.equal(await errorTitle(s7), 'Locked right now')
  assert.match(await s7.getByTestId('error-why').innerText(), /code is right, but this screen is not taking anyone new/)
  assert.equal(await receiver.getByTestId('request-modal').count(), 0)
  await setMode('open')
  await s7.getByTestId('error-retry').click()
  await s7.getByTestId('what-page').waitFor({ timeout: 15000 })
  console.log('lock + unlock OK')

  // --- 7. No and block: a blocked presenter hears "Not allowed" at once and the host is not asked again
  await s7.getByTestId('disconnect').click()
  await status(receiver, 'peer_left', 10000)
  await setMode('confirm')
  const s9 = await newPresenter()
  await connect(s9, 'Ida')
  await receiver.getByTestId('request-modal').waitFor()
  await armed()
  await receiver.getByTestId('request-block').click()
  await s9.getByTestId('error-page').waitFor()
  assert.equal(await errorTitle(s9), 'Not allowed')
  await waitFor(async () => /“Ida” denied and blocked/.test(await receiver.getByTestId('toast').innerText()), 'toast: blocked')
  const s9b = await newPresenter()
  await connect(s9b, 'Ida again')
  await s9b.getByTestId('error-page').waitFor()
  assert.equal(await errorTitle(s9b), 'Not allowed', 'a blocked presenter is refused at once')
  await receiver.waitForTimeout(1200)
  assert.equal(await receiver.getByTestId('request-modal').count(), 0, 'the host is not asked')
  console.log('no and block OK')

  // --- 8. Withdrawing tells the host; German texts on both sides (informal "du")
  // (the blocked address cannot ask again in this session, so the rest uses a fresh display)
  const dctx2 = await newContext(browser, { locale: 'de-DE' })
  const d2 = await dctx2.newPage()
  const server2 = startServer({ args: ['--set', `join_approval_secs=${APPROVAL_SECS}`] })
  try {
    await waitHealthy(server2.base)
    const code2 = await openDisplay(d2, server2.base, { mode: 'confirm' })
    assert.match(await d2.getByTestId('mode-pill').innerText(), /Mit Freigabe/)
    const gctx = await newContext(browser, { locale: 'de-DE' })
    const g = await gctx.newPage()
    await presenterStart(g, server2.base, code2, { name: 'Kim' })
    await g.getByTestId('waiting-page').waitFor()
    assert.match(await g.getByTestId('waiting-page').innerText(), /Warten auf Freigabe/)
    assert.match(await g.getByTestId('waiting-page').innerText(), /Deine Prüfzahl/)
    await d2.getByTestId('request-modal').waitFor()
    assert.match(await d2.getByTestId('request-modal').innerText(), /Anfrage/i)
    assert.match(await d2.getByTestId('request-modal').innerText(), /Nein und sperren/i)
    assert.match(await d2.getByTestId('request-modal').innerText(), /möchte sich verbinden/)
    await g.getByTestId('withdraw').click()
    await d2.getByTestId('request-modal').waitFor({ state: 'detached', timeout: 5000 })
    await waitFor(async () => /hat die Anfrage zurückgezogen/.test(await d2.getByTestId('toast').innerText()), 'German toast')
    // --- 9. Timeout: nobody decides, the presenter is told, the request leaves the screen
    await presenterStart(g, server2.base, code2, { name: 'Jo' })
    await d2.getByTestId('request-modal').waitFor()
    await g.getByTestId('error-page').waitFor({ timeout: (APPROVAL_SECS + 8) * 1000 })
    assert.match(await g.getByTestId('error-title').innerText(), /Keine Antwort/)
    await d2.getByTestId('request-modal').waitFor({ state: 'detached', timeout: 5000 })
    await waitFor(async () => /Anfrage von „Jo“ verfallen/.test(await d2.getByTestId('toast').innerText()), 'German toast: expired')
    console.log('withdraw + timeout + de OK')
    await dctx2.close()
    await gctx.close()
  } finally {
    server2.proc.kill('SIGTERM')
  }

  // --- 10. The default mode is kept on the display device
  assert.equal(await receiver.evaluate(() => JSON.parse(localStorage.getItem('cometcast.display.setup')).mode), 'confirm')
  console.log('E2E join approval OK')
} finally {
  await browser?.close()
  server.proc.kill('SIGTERM')
}
