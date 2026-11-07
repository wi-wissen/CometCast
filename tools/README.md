# tools

## check-i18n.mjs

Checks that English and German translations have the same keys. Plain Node 22,
no dependencies.

```sh
node tools/check-i18n.mjs            # check the repo
node tools/check-i18n.mjs --root DIR # check another folder (used by the tests)
node --test tools/check-i18n.test.mjs                   # self-test with temporary fixtures
```

Checks:

- `apps/web/src/locales/en.json` vs `de.json` (nested keys flattened with dots)
- `apps/web/src/locales/de-sie.json` vs `de.json` (same keys and placeholders; `de.json` must have no formal forms)
- `apps/flutter/lib/l10n/app_en.arb` vs `app_de.arb` (keys starting with `@` ignored)
- `apps/apple-tv/Sources/CometCastTV/Localizable.xcstrings` (each key needs `en` and `de`; the Swift tests also check that every key the core can produce and every key the views use exists, and that placeholders agree) and `Localizable-sie.xcstrings` (the formal German set, not loaded by default: same keys, English and `{placeholders}` as the du catalog; the loaded one must have no formal forms)

Exit code 1 on a real mismatch or unreadable file. A missing file is reported
as "skipped: file missing" and does not fail. Keys are compared only within
each app, never across apps. Runs in CI via `.github/workflows/i18n.yml`.

## branding.mjs, gen-branding.mjs, build-site.mjs (and branding.test.mjs)

The branding file mechanism (publisher identity, privacy and imprint links; [branding/README.md](../branding/README.md)).
`branding.mjs` loads and validates the file named by `COMETCAST_BRANDING` (default `branding/default.json`) and is shared by the others and by
`apps/web/vite.config.ts`. `gen-branding.mjs` writes `apps/flutter/lib/src/branding.g.dart` and
`apps/apple-tv/Sources/CometCastTV/Branding.g.swift` (`--check` fails when a committed file is stale; the committed ones are the default's).
`build-site.mjs` generates `site/` (page text in `site-content/`; `COMETCAST_PUBLIC_BUILD=1` fails while required imprint fields are empty; `--out DIR`).
`node --test tools/branding.test.mjs` tests all three.

## set-version.mjs, release-stage.sh, release-summary.mjs, release-guard.sh (and their tests)

Helpers of `.github/workflows/release.yml` (docs/releasing.md). `set-version.mjs VERSION` writes one
version into Cargo, Flutter, web, tvOS and the compose file (nothing is committed; run it in a
scratch copy). `release-stage.sh` adds LICENSE, THIRD_PARTY_NOTICES.md and VERSION to a staging
folder. `release-summary.mjs` writes the build-status part of the release notes. For the `tvos` job: `set-version.mjs` rewrites the **first** `MARKETING_VERSION` and `CURRENT_PROJECT_VERSION` lines of `apps/apple-tv/project.yml` (they must stay once, in the top level `settings`), and the job reads the **first** `PRODUCT_BUNDLE_IDENTIFIER` line (the app target's) and builds `-project CometCastTV.xcodeproj -scheme CometCastTV`; keep these names in step (docs/appletv.md).

```sh
node --test tools/set-version.test.mjs
```
`release-guard.sh` holds the duplicate guards (tag/release/draft exists, Docker version tag exists,
`skip` typos); self-tests: `node --test tools/*.test.mjs` and `bash tools/release-guard.test.sh`.
`release-summary.mjs` fails when no platform artefact exists.

## gen-appletv-strings.mjs, gen-appletv-icons.mjs

Generators for the Apple TV display (`apps/apple-tv`). Plain Node 22, no dependencies; `--check` fails when a generated file differs.

```sh
node tools/gen-appletv-strings.mjs [--check]   # Localizable.xcstrings (du) and Localizable-sie.xcstrings from the web and Flutter locale files and spec table 15.13
node tools/gen-appletv-icons.mjs [--check]     # assets/icons/*.svg and the Apple TV asset catalog images from apps/web/src/components/Icon.vue and assets/logo
node tools/gen-third-party.mjs [--check]       # also copies THIRD_PARTY_LICENSES.md and THIRD_PARTY_NOTICES.md to apps/apple-tv/Resources/Licenses
```
