// SPDX-License-Identifier: GPL-3.0-or-later
// Page list, paths and redirects of the website. Shared by tools/build-site.mjs and tools/check-site-links.mjs.
// English is the default under /, German mirrors it under /de/ with German slugs.
export const LANGS = ['en', 'de'];
export const ORIGIN = 'https://cometcast.de';
export const PAGES = ['home', 'download', 'how', 'privacy', 'selfhost', 'open', 'contact', 'imprint'];
export const SLUG = {
  en: { home: '', download: 'download/', how: 'how-it-works/', privacy: 'privacy/', selfhost: 'self-hosting/', open: 'open-source/', contact: 'contact/', imprint: 'imprint/' },
  de: { home: 'de/', download: 'de/download/', how: 'de/so-funktionierts/', privacy: 'de/datenschutz/', selfhost: 'de/selbst-hosten/', open: 'de/open-source/', contact: 'de/kontakt/', imprint: 'de/impressum/' },
};
export const pageUrl = (lang, p) => '/' + SLUG[lang][p];
export const otherLang = (lang) => (lang === 'en' ? 'de' : 'en');

/**
 * Old paths (the German-first site and its /en/ tree) and short links. `to` is a page key; `optional` redirects
 * exist only when that page exists (the imprint page is left out without a branding file).
 * The old German /open-source/ is the English page now, so it cannot redirect.
 */
export const REDIRECTS = [
  { from: '/funktionsweise/', lang: 'de', to: 'how' },
  { from: '/downloads/', lang: 'de', to: 'download' },
  { from: '/datenschutz/', lang: 'de', to: 'privacy' },
  { from: '/kontakt/', lang: 'de', to: 'contact' },
  { from: '/impressum/', lang: 'de', to: 'imprint', optional: true },
  { from: '/en/', lang: 'en', to: 'home' },
  { from: '/en/how-it-works/', lang: 'en', to: 'how' },
  { from: '/en/downloads/', lang: 'en', to: 'download' },
  { from: '/en/privacy/', lang: 'en', to: 'privacy' },
  { from: '/en/open-source/', lang: 'en', to: 'open' },
  { from: '/en/contact/', lang: 'en', to: 'contact' },
  { from: '/en/imprint/', lang: 'en', to: 'imprint', optional: true },
  // short links: /d is the English download page (language hint included), /de/d the German one
  { from: '/d/', lang: 'en', to: 'download' },
  { from: '/de/d/', lang: 'de', to: 'download' },
];
/** /apk/ is generated separately: the latest Android APK of the release, else the download page's Android card. */
export const APK_PATH = '/apk/';
