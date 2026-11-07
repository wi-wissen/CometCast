// SPDX-License-Identifier: GPL-3.0-or-later
import { loadEnv } from 'vite'
import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

/**
 * Public base path of the built app: `VITE_BASE` (e.g. `/app/` when the files are served at
 * https://cometcast.de/app/). Default `/`. Must be an absolute path: relative bases (`./`) are not
 * supported because the PDF worker builds its URLs from it (src/core/pdfWorker.ts).
 */
export function normalizeBase(raw: string | undefined): string {
  const b = (raw ?? '').trim()
  if (!b) return '/'
  if (!/^\/[A-Za-z0-9._~\-/]*$/.test(b) || b.includes('//') || b.includes('..')) {
    throw new Error(`VITE_BASE must be an absolute path such as /app/ (got ${JSON.stringify(raw)})`)
  }
  return b.endsWith('/') ? b : `${b}/`
}

export default defineConfig(({ mode }) => ({
  base: normalizeBase(loadEnv(mode, '.', 'VITE_').VITE_BASE),
  plugins: [vue(), tailwindcss()],
  test: {
    environment: 'jsdom',
    include: ['tests/**/*.test.ts'],
  },
}))
