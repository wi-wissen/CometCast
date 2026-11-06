# Comet Cast web app

The no-install variant of Comet Cast: runs in a browser. This is a preview build (v0.1): the UI shell
only, nothing connects yet. The Rust-WASM core will be added later (see `src/core/index.ts`).

Stack: Vue 3, TypeScript, Vite, Tailwind CSS 4, vue-i18n (English and German).

## Commands

```sh
cd apps/web && npm ci && npm run dev
```

- `npm run typecheck` - type-check with vue-tsc
- `npm test` - unit tests (vitest)
- `npm run build` - production build into `dist/`
- `npm run preview` - serve the production build

## Translations

UI strings live in `src/locales/en.json` and `src/locales/de.json`. Both files must have identical keys
(checked by a test). Screen sharing needs a desktop browser and HTTPS (or localhost).

License: GPL-3.0-or-later
