// SPDX-License-Identifier: GPL-3.0-or-later
// Types of tools/branding.mjs (read by apps/web/vite.config.ts).
export interface PerLang {
  de: string
  en: string
}
export interface Branding {
  publisher_name: string
  publisher_url: string
  privacy_url: PerLang
  imprint_url: PerLang
  source_url: string
  contact: string
  imprint: Record<'name' | 'street' | 'city' | 'representative' | 'email' | 'phone' | 'register' | 'vat_id' | 'content_responsible', string>
}
export const REPO_ROOT: string
export const DEFAULT_BRANDING: string
export const LANGS: string[]
export const IMPRINT_FIELDS: string[]
export const IMPRINT_REQUIRED: string[]
export function brandingPath(env?: Record<string, string | undefined>): string
export function normaliseBranding(raw: unknown, label?: string): Branding
export function loadBrandingFile(file: string): Branding
export function loadBranding(env?: Record<string, string | undefined>): { file: string; branding: Branding }
export function missingImprint(branding: Branding): string[]
