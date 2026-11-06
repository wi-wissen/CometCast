// SPDX-License-Identifier: GPL-3.0-or-later
import { createApp } from 'vue'
import App from './App.vue'
import { createAppI18n } from './i18n'
import './style.css'

createApp(App).use(createAppI18n()).mount('#app')
