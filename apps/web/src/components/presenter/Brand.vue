<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { logoFor } from '../display/logos'
// Brand row: logo 28 + "Comet Cast" (spec 5.2). The presenter pages follow the device's setting.
const { t } = useI18n()
const dark = logoFor('dark')
const light = logoFor('light')
</script>

<template>
  <div class="cp-brand">
    <picture>
      <source :srcset="dark" media="(prefers-color-scheme: dark)" />
      <img :src="light" alt="" width="28" height="28" />
    </picture>
    {{ t('app.name') }}
  </div>
</template>
