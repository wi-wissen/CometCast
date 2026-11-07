<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import Icon, { type IconName } from '../Icon.vue'
import { APPEARANCES, type Appearance } from '../../core/appearance'
import { JOIN_MODES, type JoinMode } from '../../core/signaling'

// Settings (spec 4.3): join mode and appearance, nothing else. Changes apply at once; the dialog
// stays open after a change; there is no Save button.
const props = defineProps<{ mode: JoinMode; appearance: Appearance }>()
const emit = defineEmits<{ mode: [mode: JoinMode]; appearance: [a: Appearance]; close: [] }>()
const { t } = useI18n()
const root = ref<HTMLElement | null>(null)

const ICONS: Record<JoinMode, IconName> = { open: 'open', confirm: 'approval', locked: 'locked' }
const MODE_KEY: Record<JoinMode, string> = { open: 'mode.open', confirm: 'mode.approval', locked: 'mode.locked' }
const DESC_KEY: Record<JoinMode, string> = {
  open: 'settings.join.openText',
  confirm: 'settings.join.approvalText',
  locked: 'settings.join.lockedText',
}
const desc = computed(() => t(DESC_KEY[props.mode]))

// Initial focus: the tile of the current join mode.
onMounted(async () => {
  await nextTick()
  root.value?.querySelector<HTMLElement>('[data-testid="settings-mode-' + props.mode + '"]')?.focus()
})
</script>

<template>
  <div class="cd-modal" data-testid="settings-modal">
    <div ref="root" class="cd-wz" role="dialog" aria-modal="true" :aria-label="t('settings.title')">
      <div class="cd-rh">
        <h2>{{ t('settings.title') }}</h2>
        <button type="button" class="cd-xbtn" data-row="0" data-testid="settings-close" @click="emit('close')">
          <Icon name="close" />{{ t('settings.close') }}
        </button>
      </div>
      <div class="cd-sec">
        <div class="cd-rh"><h2>{{ t('settings.join.label') }}</h2></div>
        <div class="cd-modes" role="radiogroup" :aria-label="t('settings.join.label')">
          <button
            v-for="m in JOIN_MODES"
            :key="m"
            type="button"
            role="radio"
            class="cd-mt"
            :class="{ on: mode === m }"
            :aria-checked="mode === m"
            data-row="1"
            :data-testid="`settings-mode-${m}`"
            @click="emit('mode', m)"
          >
            <Icon :name="ICONS[m]" />{{ t(MODE_KEY[m]) }}
          </button>
        </div>
        <p class="cd-mdesc" data-testid="settings-mode-desc">{{ desc }}</p>
      </div>
      <div class="cd-sec">
        <div class="cd-rh"><h2>{{ t('settings.appearance') }}</h2></div>
        <div class="cd-looks" role="radiogroup" :aria-label="t('settings.appearance')">
          <button
            v-for="a in APPEARANCES"
            :key="a"
            type="button"
            role="radio"
            class="cd-mt"
            :class="{ on: appearance === a }"
            :aria-checked="appearance === a"
            data-row="2"
            :data-testid="`settings-look-${a}`"
            @click="emit('appearance', a)"
          >
            {{ t(`appearance.${a}`) }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
