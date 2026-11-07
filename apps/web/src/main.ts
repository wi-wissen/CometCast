// SPDX-License-Identifier: GPL-3.0-or-later
import { createApp } from 'vue'
import App from './App.vue'
import { createAppI18n } from './i18n'
import { initRelay } from './composables/useRelay'
import './style.css'

// config.json (optional) is read before anything connects; a missing file or a slow host costs
// at most the short timeout in loadRelayConfig.
void initRelay().finally(() => createApp(App).use(createAppI18n()).mount('#app'))
