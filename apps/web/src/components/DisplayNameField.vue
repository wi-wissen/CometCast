<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { cleanName, loadDisplayName, MAX_NAME_CHARS, saveDisplayName } from '../core/joinPrefs'

// The name that the host sees when it has to approve a join. Optional; remembered in this browser.
defineProps<{ disabled?: boolean }>()
const { t } = useI18n()
const name = ref(loadDisplayName())
function onInput(e: Event) {
  const el = e.target as HTMLInputElement
  name.value = cleanName(el.value)
  saveDisplayName(name.value)
}
</script>

<template>
  <div class="flex flex-col gap-1">
    <label for="display-name" class="text-sm font-medium">{{ t('displayName.label') }}</label>
    <input
      id="display-name"
      :value="name"
      :disabled="disabled"
      type="text"
      autocomplete="nickname"
      :maxlength="MAX_NAME_CHARS"
      :placeholder="t('displayName.placeholder')"
      aria-describedby="display-name-hint"
      class="w-full max-w-xs rounded-cc border border-line2 bg-surface px-3 py-2"
      @input="onInput"
    />
    <p id="display-name-hint" class="text-sm text-muted">{{ t('displayName.hint') }}</p>
  </div>
</template>
