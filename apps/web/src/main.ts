// SPDX-License-Identifier: GPL-3.0-or-later
import { createApp } from 'vue'
import App from './App.vue'
import { createAppI18n } from './i18n'
import { initRelay } from './composables/useRelay'
import '@fontsource/atkinson-hyperlegible-next/latin-400.css'
import '@fontsource/atkinson-hyperlegible-next/latin-500.css'
import '@fontsource/atkinson-hyperlegible-next/latin-700.css'
import '@fontsource/atkinson-hyperlegible-next/latin-ext-400.css'
import '@fontsource/atkinson-hyperlegible-next/latin-ext-500.css'
import '@fontsource/atkinson-hyperlegible-next/latin-ext-700.css'
import '@fontsource/atkinson-hyperlegible-mono/latin-500.css'
import '@fontsource/atkinson-hyperlegible-mono/latin-700.css'
import './style.css'

// Never inside a frame of another page: a page around the app could cover its buttons (the join
// approval, "Share screen") and hand it the right to capture the screen. Not mounting is the whole
// defence where no header can be sent (static hosting); cometcast-server also forbids framing.
function framed(): boolean {
  try {
    return window.top !== window.self
  } catch {
    return true
  }
}

// config.json (optional) is read before anything connects; a missing file or a slow host costs
// at most the short timeout in loadRelayConfig.
if (!framed()) void initRelay().finally(() => createApp(App).use(createAppI18n()).mount('#app'))
