<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script setup lang="ts">
defineProps<{ title: string; desc: string; disabled?: boolean }>()
</script>

<template>
  <button
    type="button"
    :disabled="disabled"
    class="flex w-full items-center justify-between gap-4 border-b border-line px-4 py-3.5 text-left last:border-b-0 hover:bg-sunk disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:bg-transparent"
  >
    <span class="flex flex-col">
      <span class="font-medium">{{ title }}</span>
      <span class="text-sm text-muted">{{ desc }}</span>
    </span>
    <svg aria-hidden="true" viewBox="0 0 24 24" class="h-[18px] w-[18px] flex-none fill-none stroke-muted stroke-[1.7]" stroke-linecap="round" stroke-linejoin="round">
      <path d="M9 6l6 6-6 6" />
    </svg>
  </button>
</template>
