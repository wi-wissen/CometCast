// SPDX-License-Identifier: GPL-3.0-or-later
//
// The links of the start page footer (spec 15.9, decision N83, N92): source of this version, the licence list
// of this build, and privacy / imprint only where there is something to point to: the pages of the build's
// branding file (branding/README.md), overridden by the operator's config.json (`privacy_url`, `imprint_url`),
// otherwise none. Nothing here depends on the host the page is served from.
import type { Branding } from '../../../../tools/branding.mjs'
import type { LegalUrls } from './relay'

/** Branding of this build (COMETCAST_BRANDING at build time, branding/default.json otherwise). */
export const BRANDING: Branding = __BRANDING__

export function sourceUrl(version: string, branding: Branding = BRANDING): string {
  return `${branding.source_url.replace(/\/$/, '')}/tree/v${version}`
}

/** Address of the licence list of this build (`third-party-licenses.txt` next to index.html). */
export function licencesUrl(base = '/'): string {
  return `${base.endsWith('/') ? base : base + '/'}third-party-licenses.txt`
}

export function legalLinks(lang: 'de' | 'en', cfg: LegalUrls, branding: Branding = BRANDING): LegalUrls {
  const out: LegalUrls = {}
  const privacy = cfg.privacy ?? (branding.privacy_url[lang] || undefined)
  const imprint = cfg.imprint ?? (branding.imprint_url[lang] || undefined)
  if (privacy) out.privacy = privacy
  if (imprint) out.imprint = imprint
  return out
}
