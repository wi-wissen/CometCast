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

// config.json (optional) is read before anything connects; a missing file or a slow host costs
// at most the short timeout in loadRelayConfig.
void initRelay().finally(() => createApp(App).use(createAppI18n()).mount('#app'))
