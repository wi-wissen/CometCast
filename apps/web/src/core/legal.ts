// SPDX-License-Identifier: GPL-3.0-or-later
//
// The links of the start page footer (spec 15.9, decision N83, N92): source of this version, the licence list
// of this build, and privacy / imprint only where there is something to point to: on the cometcast.de
// deployment the website's pages, elsewhere the operator's pages from config.json (`privacy_url`,
// `imprint_url`), otherwise none.
import type { LegalUrls } from './relay'

export const SOURCE_BASE = 'https://github.com/wi-wissen/CometCast/tree/v'
const SITE = 'https://cometcast.de'

export function sourceUrl(version: string): string {
  return `${SOURCE_BASE}${version}`
}

/** Address of the licence list of this build (`third-party-licenses.txt` next to index.html). */
export function licencesUrl(base = '/'): string {
  return `${base.endsWith('/') ? base : base + '/'}third-party-licenses.txt`
}

/** Is this page the cometcast.de deployment? */
export function isProjectSite(hostname: string): boolean {
  return hostname.toLowerCase() === 'cometcast.de'
}

export function legalLinks(hostname: string, lang: 'de' | 'en', cfg: LegalUrls): LegalUrls {
  const site = isProjectSite(hostname)
  const out: LegalUrls = {}
  const privacy = cfg.privacy ?? (site ? (lang === 'de' ? `${SITE}/datenschutz/` : `${SITE}/en/privacy/`) : undefined)
  const imprint = cfg.imprint ?? (site ? (lang === 'de' ? `${SITE}/impressum/` : `${SITE}/en/imprint/`) : undefined)
  if (privacy) out.privacy = privacy
  if (imprint) out.imprint = imprint
  return out
}
