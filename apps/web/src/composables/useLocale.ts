// SPDX-License-Identifier: GPL-3.0-or-later
import { watchEffect } from 'vue'
import { useI18n } from 'vue-i18n'
import { storeLocale, type Locale } from '../i18n'

/** Locale switch: persists the choice and keeps <html lang> and <title> in sync. */
export function useLocale() {
  const { locale, t } = useI18n()

  watchEffect(() => {
    document.documentElement.lang = locale.value
    document.title = `${t('app.name')}: ${t('app.tagline')}`
  })

  function setLocale(l: Locale) {
    locale.value = l
    storeLocale(l)
  }

  return { locale, setLocale }
}
