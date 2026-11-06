// SPDX-License-Identifier: GPL-3.0-or-later
import { createI18n } from 'vue-i18n'
import en from '../locales/en.json'
import de from '../locales/de.json'

export type Locale = 'en' | 'de'
export const LOCALES: Locale[] = ['en', 'de']
export const STORAGE_KEY = 'cometcast.locale'

/** de* -> de, everything else -> en. */
export function detectLocale(lang: string | undefined | null): Locale {
  return typeof lang === 'string' && /^de(\b|[-_])/i.test(lang.trim()) ? 'de' : 'en'
}

export function readStoredLocale(): Locale | null {
  try {
    const v = localStorage.getItem(STORAGE_KEY)
    return v === 'en' || v === 'de' ? v : null
  } catch {
    return null
  }
}

export function storeLocale(l: Locale): void {
  try {
    localStorage.setItem(STORAGE_KEY, l)
  } catch {
    /* storage unavailable: choice is simply not remembered */
  }
}

export function initialLocale(): Locale {
  const nav = typeof navigator !== 'undefined' ? navigator.language : 'en'
  return readStoredLocale() ?? detectLocale(nav)
}

export function createAppI18n(locale: Locale = initialLocale()) {
  return createI18n({
    legacy: false,
    locale,
    fallbackLocale: 'en',
    messages: { en, de },
  })
}
