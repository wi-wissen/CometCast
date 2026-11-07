// SPDX-License-Identifier: GPL-3.0-or-later
//
// Where the presenter's "Get the app" buttons lead. The spec (5.2, 5.3) names the buttons and the
// store pages ("the system store page, not designed by us") but gives no addresses: the listings
// do not exist yet. Until they do these are PLACEHOLDERS. An empty string means "not published
// yet": the page then does not offer the app for that platform at all (no button, no small print,
// no "or" divider, no download note; the idle footer says only "No download needed.") because a
// button must never do nothing and the page must never claim a download that did not start
// (docs/design/first-run-and-display.md, 4.2 and 5.2).

/** Google Play listing of the Android app. PLACEHOLDER until the app is published. */
export const STORE_URL_GOOGLE_PLAY = ''
/** App Store listing of the iOS app. PLACEHOLDER until the app is published. */
export const STORE_URL_APP_STORE = ''
/** Download page for the desktop apps (Windows, macOS, Linux). PLACEHOLDER until the app is published. */
export const DOWNLOAD_URL_DESKTOP = ''

export interface StoreUrls {
  googlePlay: string
  appStore: string
  desktop: string
}

export const STORE_URLS: StoreUrls = {
  googlePlay: STORE_URL_GOOGLE_PLAY,
  appStore: STORE_URL_APP_STORE,
  desktop: DOWNLOAD_URL_DESKTOP,
}

/** The address of the app for a platform (`store` = Platform.store: null means a desktop). Empty = no offer. */
export function appUrlFor(store: 'google-play' | 'app-store' | null, urls: StoreUrls = STORE_URLS): string {
  return store === 'google-play' ? urls.googlePlay : store === 'app-store' ? urls.appStore : urls.desktop
}

/** Does at least one store or download link exist? (The idle footer names stores only then.) */
export function anyAppLink(urls: StoreUrls = STORE_URLS): boolean {
  return Boolean(urls.googlePlay || urls.appStore || urls.desktop)
}
