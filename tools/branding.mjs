// SPDX-License-Identifier: GPL-3.0-or-later
// Loads and validates a branding file (branding/README.md). Shared by tools/build-site.mjs,
// tools/gen-branding.mjs and apps/web/vite.config.ts. Plain Node, no dependencies.
import { existsSync, readFileSync } from 'node:fs';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
export const DEFAULT_BRANDING = join(REPO_ROOT, 'branding', 'default.json');
export const LANGS = ['de', 'en'];
export const IMPRINT_FIELDS = ['name', 'street', 'city', 'representative', 'email', 'phone', 'register', 'vat_id', 'content_responsible'];
/** Imprint fields a public website build cannot do without (section 5 DDG: provider, address, e-mail). */
export const IMPRINT_REQUIRED = ['name', 'street', 'city', 'email'];
const TOP = ['publisher_name', 'publisher_url', 'privacy_url', 'imprint_url', 'source_url', 'contact', 'imprint'];
const URL_KEYS = ['publisher_url', 'privacy_url', 'imprint_url', 'source_url'];

/** The file named by COMETCAST_BRANDING (relative: from the current folder, else the repository root), else the default. */
export function brandingPath(env = process.env) {
  const p = (env.COMETCAST_BRANDING ?? '').trim();
  if (!p) return DEFAULT_BRANDING;
  if (isAbsolute(p)) return p;
  const here = resolve(p);
  return existsSync(here) ? here : resolve(REPO_ROOT, p);
}

function str(v, what) {
  if (v === undefined || v === null) return '';
  if (typeof v !== 'string') throw new Error(`${what} must be a string`);
  return v.trim();
}

function httpsUrl(v, what) {
  const s = str(v, what);
  if (!s) return '';
  let u;
  try {
    u = new URL(s);
  } catch {
    throw new Error(`${what} is not a valid URL: ${JSON.stringify(s)}`);
  }
  if (u.protocol !== 'https:' || u.username || u.password) throw new Error(`${what} must be an https URL without credentials: ${JSON.stringify(s)}`);
  return s;
}

/** Validates parsed JSON and returns the normalised shape: URL fields per language, every key present. */
export function normaliseBranding(raw, label = 'branding') {
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) throw new Error(`${label}: must be a JSON object`);
  for (const k of Object.keys(raw)) if (!TOP.includes(k)) throw new Error(`${label}: unknown key "${k}" (known: ${TOP.join(', ')})`);
  const out = {};
  for (const k of ['publisher_name', 'contact']) out[k] = str(raw[k], `${label}: ${k}`);
  for (const k of URL_KEYS) {
    const v = raw[k];
    if (k === 'source_url' || k === 'publisher_url') {
      out[k] = httpsUrl(v, `${label}: ${k}`);
      continue;
    }
    // per language: a string (same page for every language) or { de, en }
    const o = {};
    if (v !== null && typeof v === 'object' && !Array.isArray(v)) {
      for (const l of Object.keys(v)) if (!LANGS.includes(l)) throw new Error(`${label}: ${k} has unknown language "${l}"`);
      for (const l of LANGS) o[l] = httpsUrl(v[l], `${label}: ${k}.${l}`);
    } else {
      for (const l of LANGS) o[l] = httpsUrl(v, `${label}: ${k}`);
    }
    out[k] = o;
  }
  if (!out.source_url) throw new Error(`${label}: source_url must be set (the GPL asks for a way to the source)`);
  const imp = raw.imprint ?? {};
  if (typeof imp !== 'object' || Array.isArray(imp) || imp === null) throw new Error(`${label}: imprint must be an object`);
  for (const k of Object.keys(imp)) if (!IMPRINT_FIELDS.includes(k)) throw new Error(`${label}: unknown imprint field "${k}" (known: ${IMPRINT_FIELDS.join(', ')})`);
  out.imprint = {};
  for (const k of IMPRINT_FIELDS) out.imprint[k] = str(imp[k], `${label}: imprint.${k}`);
  return out;
}

export function loadBrandingFile(file) {
  let text;
  try {
    text = readFileSync(file, 'utf8');
  } catch (e) {
    throw new Error(`cannot read branding file ${file}: ${e.message}`);
  }
  let raw;
  try {
    raw = JSON.parse(text);
  } catch (e) {
    throw new Error(`${file}: invalid JSON: ${e.message}`);
  }
  return normaliseBranding(raw, file);
}

/** Loads the branding file selected by the environment. */
export function loadBranding(env = process.env) {
  const file = brandingPath(env);
  return { file, branding: loadBrandingFile(file) };
}

/** Required imprint fields that are still empty. */
export function missingImprint(branding) {
  return IMPRINT_REQUIRED.filter((k) => !branding.imprint[k]);
}
