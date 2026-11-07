// SPDX-License-Identifier: GPL-3.0-or-later
// Round 5, web part (docs/design/fix-list-round5.md, W1-W3): the join address, the footer line, the operator pages.
import { describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { createAppI18n } from '../src/i18n'
import { joinUrl } from '../src/core/qr'
import { joinAddress, loadRelayConfig, parseLegalConfig, resolveRelay } from '../src/core/relay'
import { DEFAULT_BRANDING, loadBrandingFile } from '../../../tools/branding.mjs'
import { legalLinks, licencesUrl, sourceUrl } from '../src/core/legal'
import StartPage from '../src/components/presenter/StartPage.vue'
import IdleScreen from '../src/components/display/IdleScreen.vue'
import en from '../src/locales/en.json'
import de from '../src/locales/de.json'
import deSie from '../src/locales/de-sie.json'
import spec from '../../../docs/design/first-run-and-display.md?raw'

vi.mock('../src/config/stores', () => ({
  STORE_URLS: { googlePlay: 'https://play.example/app', appStore: '', desktop: '' },
  appUrlFor: () => '',
  anyAppLink: () => true,
}))

const site = { protocol: 'https:', host: 'cometcast.de', search: '' }
const BUILD = 'wss://relay.cometcast.de/ws'

describe('W1 join address (N85)', () => {
  it('cometcast.de with the built-in relay: cometcast.de/app and the QR URL https://cometcast.de/app/{CODE}', () => {
    const c = resolveRelay({ loc: site, buildDefault: BUILD })
    const j = joinAddress(c, site, '/app/')
    expect(j).toEqual({ dom: 'cometcast.de/app', scheme: 'https' })
    expect(joinUrl(j.dom, 'K7Q M2W', j.scheme)).toBe('https://cometcast.de/app/K7QM2W')
  })
  it('a self-hosted server is unchanged: its own host, also with a port', () => {
    const own = { protocol: 'https:', host: 'cast.musterfirma.de', search: '' }
    expect(joinAddress(resolveRelay({ loc: own }), own, '/')).toEqual({ dom: 'cast.musterfirma.de', scheme: 'https' })
    const lan = { protocol: 'http:', host: '192.168.1.5:8443', search: '' }
    expect(joinAddress(resolveRelay({ loc: lan }), lan, '/')).toEqual({ dom: '192.168.1.5:8443', scheme: 'http' })
  })
  it('a confirmed ?server= relay shows that relay\'s host', () => {
    const loc = { ...site, search: '?server=wss://cast.example.org/ws' }
    const c = resolveRelay({ loc, buildDefault: BUILD })
    expect(joinAddress(c, loc, '/app/')).toEqual({ dom: 'cast.example.org', scheme: 'https' })
  })
  it('a relay from config.json that is another server: the page host plus the base', () => {
    const c = resolveRelay({ loc: site, configRelays: [{ wsUrl: 'wss://r.example/ws', httpBase: 'https://r.example', host: 'r.example', insecure: false, source: 'config' }] })
    expect(joinAddress(c, site, '/app/').dom).toBe('cometcast.de/app')
  })
  it('the idle footer writes the join address once (idle.appLine has {dom}/app)', () => {
    const props = { theme: 'dark' as const, room: 'Elbe', mode: 'open' as const, code: 'K7QM2W', dom: 'cometcast.de/app', view: 'online' as const, cause: 'network' as const, dimmed: false, scheme: 'https' as const }
    const w = mount(IdleScreen, { global: { plugins: [createAppI18n('en')] }, props })
    expect(w.get('[data-testid=idle-foot]').text()).toContain('Google Play · cometcast.de/app')
    expect(w.get('[data-testid=idle-foot]').text()).not.toContain('/app/app')
    expect(w.get('[data-testid=address]').text()).toBe('cometcast.de/app')
    expect(w.get('svg[role=img]').attributes('aria-label')).toBe('QR: https://cometcast.de/app/K7QM2W')
    const own = mount(IdleScreen, { global: { plugins: [createAppI18n('en')] }, props: { ...props, dom: 'cast.musterfirma.de' } })
    expect(own.get('[data-testid=idle-foot]').text()).toContain('cast.musterfirma.de/app')
  })
})

describe('W3 operator pages and footer (N83, 15.9)', () => {
  it('config.json: only https pages without credentials count', () => {
    expect(parseLegalConfig('{"privacy_url":"https://a.example/p","imprint_url":"https://a.example/i"}')).toEqual({ privacy: 'https://a.example/p', imprint: 'https://a.example/i' })
    expect(parseLegalConfig('{"privacy_url":"http://a.example/p","imprint_url":"javascript:alert(1)"}')).toEqual({})
    expect(parseLegalConfig('{"privacy_url":"https://u:p@a.example/p"}')).toEqual({})
    expect(parseLegalConfig('not json')).toEqual({})
  })
  it('a config.json with only operator pages is usable (not "failed")', async () => {
    const f = (async () => new Response('{"privacy_url":"https://a.example/p"}')) as unknown as typeof fetch
    const r = await loadRelayConfig({ fetchImpl: f })
    expect(r.failed).toBe(false)
    expect(r.relays).toEqual([])
    expect(r.legal.privacy).toBe('https://a.example/p')
  })
  it('legal links: config.json over the branding file over none; the host does not matter', () => {
    const none = loadBrandingFile(DEFAULT_BRANDING)
    const official = loadBrandingFile(DEFAULT_BRANDING.replace('default.json', 'cometcast.json'))
    expect(legalLinks('de', {}, official)).toEqual({ privacy: 'https://cometcast.de/datenschutz/', imprint: 'https://cometcast.de/impressum/' })
    expect(legalLinks('en', {}, official)).toEqual({ privacy: 'https://cometcast.de/en/privacy/', imprint: 'https://cometcast.de/en/imprint/' })
    expect(legalLinks('en', {}, none)).toEqual({})
    expect(legalLinks('en', { privacy: 'https://m.example/dsgvo' }, none)).toEqual({ privacy: 'https://m.example/dsgvo' })
    expect(legalLinks('en', { privacy: 'https://m.example/dsgvo' }, official)).toEqual({ privacy: 'https://m.example/dsgvo', imprint: 'https://cometcast.de/en/imprint/' })
  })
  it('addresses of the footer', () => {
    expect(sourceUrl('1.2.3')).toBe('https://github.com/wi-wissen/CometCast/tree/v1.2.3')
    expect(sourceUrl('1.2.3', { ...loadBrandingFile(DEFAULT_BRANDING), source_url: 'https://git.example/me/cast/' })).toBe('https://git.example/me/cast/tree/v1.2.3')
    expect(licencesUrl('/app/')).toBe('/app/third-party-licenses.txt')
    expect(licencesUrl('/')).toBe('/third-party-licenses.txt')
  })
  const mk = (legal = {}, lang: 'en' | 'de' = 'en') => mount(StartPage, { global: { plugins: [createAppI18n(lang)] }, props: { qrCode: null, dom: 'cast.musterfirma.de', busy: false, legal } })
  it('self-hosted without config: licence, source and licence list; no privacy or imprint', () => {
    const w = mk()
    const f = w.get('[data-testid=start-footer]')
    expect(f.text()).toBe('Free software (GPL-3.0) · Source code · Licences')
    expect(w.get('[data-testid=foot-licences]').attributes('target')).toBe('_blank')
    expect(w.get('[data-testid=foot-licences]').attributes('href')).toMatch(/third-party-licenses\.txt$/)
    expect(w.get('[data-testid=foot-source]').attributes('href')).toMatch(/^https:\/\/github\.com\/wi-wissen\/CometCast\/tree\/v\d/)
    expect(w.find('[data-testid=foot-privacy]').exists()).toBe(false)
    expect(w.find('[data-testid=foot-imprint]').exists()).toBe(false)
  })
  it('with the operator\'s config: privacy and imprint appear; links are p-primary (cp-link)', () => {
    const w = mk({ privacy: 'https://m.example/p', imprint: 'https://m.example/i' }, 'de')
    expect(w.get('[data-testid=start-footer]').text()).toBe('Freie Software (GPL-3.0) · Quelltext · Lizenzen · Datenschutz · Impressum')
    expect(w.get('[data-testid=foot-privacy]').attributes('href')).toBe('https://m.example/p')
    expect(w.get('[data-testid=foot-privacy]').classes()).toContain('cp-link')
    expect(w.get('[data-testid=start-footer]').classes()).toContain('cp-small')
  })
})

describe('copy of 15.13 (about.*, web part)', () => {
  const rows = spec.split('\n').filter((l) => /^\| `about\.(footer|source|privacy|imprint|licences)`/.test(l))
  it('every web row is in the three files word for word', () => {
    expect(rows.length).toBe(5)
    for (const line of rows) {
      const c = line.trim().replace(/^\||\|$/g, '').split(' | ').map((x) => x.trim())
      const key = c[0].replace(/`/g, '').replace(/ \(web\)$/, '').replace(/^about\./, '')
      const sie = c[3] === 'same as du' ? c[2] : c[3]
      expect((en as any).about[key], `en:${key}`).toBe(c[1])
      expect((de as any).about[key], `de:${key}`).toBe(c[2])
      expect((deSie as any).about[key], `sie:${key}`).toBe(sie)
    }
  })
})
