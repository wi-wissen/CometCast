// SPDX-License-Identifier: GPL-3.0-or-later
import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  test: {
    environment: 'jsdom',
    include: ['tests/**/*.test.ts'],
  },
})
