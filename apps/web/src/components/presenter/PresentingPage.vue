<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import Icon, { type IconName } from '../Icon.vue'
import SasLine from '../SasLine.vue'

// Presenting, presenter side (spec 5.6): icon disc, heading, body, "Stop showing". When the
// browser's own sharing bar is shown at the bottom, 76 px stay free under the button.
const props = defineProps<{
  kind: 'screen' | 'file' | 'link'
  /** The display's room name; absent when it set none: the "On ..." line is then left out (never the code). */
  room?: string
  keepBar: boolean
  sas?: string
  pdf?: boolean
  /** The shown page and the page count, as the display reports them (`page` message, decision 13.31); absent while unknown. */
  page?: number
  total?: number
  /**
   * Pause (docs/design/pause.md): offered only while the screen is shown (`kind === 'screen'`) and `pause` is passed. `requested`
   * is the presenter's own flag as asked (the button reads Resume at once); `presenter` and `display` are what the display
   * reported with `pause_state` (the presenter's flag counts as confirmed only when the display echoed it).
   */
  pause?: { requested: boolean; presenter: boolean; display: boolean }
}>()
const emit = defineEmits<{ stop: []; page: [cmd: 'next_page' | 'prev_page']; pause: []; resume: [] }>()
const { t } = useI18n()
const ICON: Record<string, IconName> = { screen: 'screen', file: 'file', link: 'link' }
// While the page is unknown both buttons stay enabled; once known, Previous is off on page 1 and Next on the last page.
const known = computed(() => Boolean(props.page && props.total))
const onFirst = computed(() => known.value && (props.page ?? 1) <= 1)
// The banner: the presenter's own pause wins (confirmed by the display); else the display's own freeze (pause.md rule 5).
const banner = computed(() => {
  const p = props.pause
  if (props.kind !== 'screen' || !p) return null
  if (p.presenter) return 'pause.presenter.banner'
  if (p.display) return 'pause.presenter.displayFrozen'
  return null
})
const onLast = computed(() => known.value && (props.page ?? 0) >= (props.total ?? 0))
</script>

<template>
  <main class="cp-page" :class="{ keepbar: keepBar }" data-testid="presenting-page">
    <div class="cp-ico acc"><Icon :name="ICON[props.kind]" /></div>
    <h1 class="cp-h" data-testid="presenting-title">{{ t(`presenting.${kind}`) }}</h1>
    <p v-if="room" class="cp-p" data-testid="presenting-where">{{ t('presenting.where', { room }) }}</p>
    <SasLine :code="sas" />
    <div class="cp-spacer"></div>
    <!-- The bottom action area: a shown PDF is turned page by page from here (spec 5.6), "Page n of m" when known, the
         Previous / Next row 8 above Stop. -->
    <div class="cp-bottom">
      <div v-if="kind === 'screen' && pause !== undefined" class="cp-pdf" data-testid="pause-area">
        <p v-if="banner" class="cp-small cp-pausebanner" role="status" data-testid="pause-banner">{{ t(banner) }}</p>
        <button
          type="button"
          class="cp-sbtn"
          :aria-pressed="pause.requested"
          data-testid="presenting-pause"
          @click="pause.requested ? emit('resume') : emit('pause')"
        >
          {{ pause.requested ? t('pause.presenter.resume') : t('pause.presenter.button') }}
        </button>
      </div>
      <div v-if="pdf" class="cp-pdf">
        <p v-if="known" class="cp-small" data-testid="presenting-pageof">{{ t('presenting.pageOf', { n: page, total }) }}</p>
        <div class="cp-pages" role="group" :aria-label="t('sendContent.document.pagesHelp')">
          <button type="button" class="cp-sbtn" :disabled="onFirst" data-testid="send-prev-page" @click="emit('page', 'prev_page')">{{ t('presenting.prevPage') }}</button>
          <button type="button" class="cp-sbtn" :disabled="onLast" data-testid="send-next-page" @click="emit('page', 'next_page')">{{ t('presenting.nextPage') }}</button>
        </div>
      </div>
      <button type="button" class="cp-pbtn" data-testid="presenting-stop" @click="emit('stop')">{{ t('presenting.stop') }}</button>
    </div>
  </main>
</template>
