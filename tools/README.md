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
- `apps/flutter/lib/l10n/app_en.arb` vs `app_de.arb` (keys starting with `@` ignored)
- `apps/apple-tv/Sources/CometCastTV/Localizable.xcstrings` (each key needs `en` and `de`)

Exit code 1 on a real mismatch or unreadable file. A missing file is reported
as "skipped: file missing" and does not fail. Keys are compared only within
each app, never across apps. Runs in CI via `.github/workflows/i18n.yml`.
