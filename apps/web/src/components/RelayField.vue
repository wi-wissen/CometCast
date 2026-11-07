<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRelay } from '../composables/useRelay'

// Shows which relay is used (host only) and, on the start page, lets the person change it.
const props = defineProps<{ editable: boolean }>()
const { t } = useI18n()
const relay = useRelay()
const input = ref('')
const error = ref('')
const note = ref('')

const others = computed(() => relay.choices.value.filter((c) => c.wsUrl !== relay.current.value.wsUrl))

function apply(text: string) {
  const code = relay.set(text)
  note.value = ''
  if (code) {
    error.value = t(`relay.errors.${code}`)
    return
  }
  error.value = ''
  input.value = ''
  note.value = t('relay.applied', { host: relay.current.value.host })
}
function reset() {
  relay.reset()
  error.value = ''
  input.value = ''
  note.value = t('relay.resetDone')
}
</script>

<template>
  <!-- Compact, read-only form (next to the back link of the send/receive views): one line, no extra height. -->
  <div v-if="!props.editable" class="flex flex-col items-end gap-1 text-sm" data-testid="relay">
    <p>
      <span class="font-medium">{{ t('relay.label') }}:</span>
      <span class="ml-1 font-mono" data-testid="relay-host">{{ relay.current.value.host }}</span>
      <span class="ml-2 rounded-cc bg-sunk px-1.5 py-0.5 text-xs text-muted" data-testid="relay-tag">
        {{ relay.nonDefault.value ? t('relay.customTag') : t('relay.defaultTag') }}
      </span>
    </p>
    <p v-if="relay.current.value.insecure" class="text-muted">{{ t('relay.insecureNote') }}</p>
    <div
      v-if="relay.nonDefault.value"
      class="rounded-cc border border-warn bg-warn-soft px-3 py-2"
      role="alert"
      data-testid="relay-warning"
    >
      <p class="font-semibold">{{ t('relay.warnTitle') }}</p>
      <p>{{ t('relay.warn', { def: relay.default.value.host, host: relay.current.value.host }) }}</p>
    </div>
  </div>
  <section v-else class="rounded-cc border border-line bg-surface px-3 py-2 text-sm" :aria-label="t('relay.label')" data-testid="relay">
    <p>
      <span class="font-medium">{{ t('relay.label') }}:</span>
      <span class="ml-1 font-mono" data-testid="relay-host">{{ relay.current.value.host }}</span>
      <span class="ml-2 rounded-cc bg-sunk px-1.5 py-0.5 text-xs text-muted" data-testid="relay-tag">
        {{ relay.nonDefault.value ? t('relay.customTag') : t('relay.defaultTag') }}
      </span>
    </p>
    <p v-if="relay.current.value.insecure" class="mt-1 text-muted">{{ t('relay.insecureNote') }}</p>
    <div
      v-if="relay.nonDefault.value"
      class="mt-2 rounded-cc border border-warn bg-warn-soft px-3 py-2"
      role="alert"
      data-testid="relay-warning"
    >
      <p class="font-semibold">{{ t('relay.warnTitle') }}</p>
      <p>{{ t('relay.warn', { def: relay.default.value.host, host: relay.current.value.host }) }}</p>
    </div>

    <template v-if="props.editable">
      <p class="mt-1 text-muted">{{ t('relay.hint') }}</p>
      <form class="mt-2 flex flex-col gap-1.5" @submit.prevent="apply(input)">
        <label for="relay-input" class="font-medium">{{ t('relay.inputLabel') }}</label>
        <div class="flex flex-wrap items-center gap-2">
          <input
            id="relay-input"
            v-model="input"
            type="text"
            inputmode="url"
            autocomplete="off"
            autocapitalize="none"
            spellcheck="false"
            maxlength="300"
            :aria-invalid="error !== ''"
            aria-describedby="relay-input-hint relay-input-error"
            data-testid="relay-input"
            class="min-w-0 flex-1 rounded-cc border border-line2 bg-surface px-3 py-1.5 font-mono"
          />
          <button type="submit" data-testid="relay-apply" class="rounded-cc bg-accent px-3 py-1.5 font-medium text-accent-fg">
            {{ t('relay.apply') }}
          </button>
          <button
            v-if="relay.nonDefault.value"
            type="button"
            data-testid="relay-reset"
            class="rounded-cc border border-line2 bg-surface px-3 py-1.5 font-medium"
            @click="reset"
          >
            {{ t('relay.reset') }}
          </button>
        </div>
        <p id="relay-input-hint" class="text-muted">{{ t('relay.inputHint') }}</p>
        <p id="relay-input-error" class="text-err" role="status" data-testid="relay-error">{{ error }}</p>
        <p v-if="note" class="text-ok" role="status">{{ note }}</p>
      </form>
      <div v-if="others.length > 0" class="mt-1 flex flex-wrap items-center gap-2">
        <span class="font-medium">{{ t('relay.choose') }}:</span>
        <button
          v-for="c in others"
          :key="c.wsUrl"
          type="button"
          class="rounded-cc border border-line2 bg-surface px-2 py-1 font-mono"
          @click="apply(c.wsUrl)"
        >
          {{ c.host }}
        </button>
      </div>
    </template>
  </section>
</template>
