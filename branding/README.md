# Branding

Who publishes a build, and where its privacy notice and imprint live, is **configuration, not source code**.
One small JSON file (the *branding file*) carries it. The web app, the project website, the Flutter apps
and the Apple TV app all read the same schema, so a self-hoster can use their own values without editing source.

| File | Use |
| --- | --- |
| [`default.json`](default.json) | What every build uses when nothing is set. No publisher, no privacy or imprint links (the apps' About screen then shows neither), no website imprint. Only `source_url` is set (the GPL asks for a way to the source). |
| [`cometcast.json`](cometcast.json) | The official values of the project's own builds (cometcast.de, store releases): publisher **Schulverwalter GmbH** and the cometcast.de privacy and imprint pages. The legal details of the imprint are **empty on purpose** (nothing was invented). |

## Schema

All keys are optional except `source_url`; strings are trimmed; URLs must be `https` without credentials; unknown
keys are an error (a typo must not silently drop a legal link). A URL key takes either one string (the same page for
every language) or `{ "de": "...", "en": "..." }`.

| Key | Meaning |
| --- | --- |
| `publisher_name` | Name shown as publisher (About screens; website text that names the steward). |
| `publisher_url` | Home page of the publisher. |
| `privacy_url` | Privacy notice, per language. |
| `imprint_url` | Imprint, per language. |
| `source_url` | Repository of the source (links "Source code"; the web footer links `<source_url>/tree/v<version>`). Must not be empty. |
| `contact` | Optional contact line (website contact page). |
| `imprint` | Operator data of the website's imprint page: `name`, `street`, `city`, `representative`, `email`, `phone`, `register`, `vat_id`, `content_responsible`. Only the website uses it. |

Validation lives in [`tools/branding.mjs`](../tools/branding.mjs) (shared by every consumer).

## Using your own file (self-hosting)

Copy `default.json`, fill in what you publish, and point the build at it with the **build-time** variable
`COMETCAST_BRANDING` (absolute path, or relative to the current folder or the repository root):

```sh
export COMETCAST_BRANDING=/path/to/my-branding.json
# web app (embedded at build time by vite.config.ts)
(cd apps/web && npm run build)
# Flutter and Apple TV: write the constants first, then build as usual
node tools/gen-branding.mjs
# website
node tools/build-site.mjs
# Docker image (the web stage builds the app): the path is below the repository root
docker build --build-arg COMETCAST_BRANDING=branding/my-branding.json -f deploy/Dockerfile ..
```

- **Web app, runtime:** a `config.json` next to `index.html` may also carry `privacy_url` and `imprint_url`
  (https, one string each, every language). They override the branding file, so an operator can change the links
  without rebuilding.
- **Flutter and Apple TV:** [`tools/gen-branding.mjs`](../tools/gen-branding.mjs) writes `apps/flutter/lib/src/branding.g.dart`
  and `apps/apple-tv/Sources/CometCastTV/Branding.g.swift` (constants `Branding.publisherName`, `publisherUrl`,
  `privacyUrlDe/En`, `imprintUrlDe/En`, `sourceUrl`, `contact`; an empty string means "not set": show nothing).
  The committed files are generated from `default.json`; `node tools/gen-branding.mjs --check` verifies that.
  Release jobs regenerate them from the selected file before building.
- **Website:** `tools/build-site.mjs` takes the `imprint` data from the file and the page text from
  [`site-content/`](../site-content) (`imprint.de.md`, `imprint.en.md`, `privacy.de.md`, `privacy.en.md`). With the
  empty default the imprint page, its footer link and its sitemap entry are left out.
- **CI and release jobs:** `release.yml` and `pages.yml` set `COMETCAST_BRANDING` to `branding/cometcast.json`, or to the
  file named by the repository variable `COMETCAST_BRANDING` (that is how a fork uses its own file).

## What must be filled for store and public releases

- **Website (`COMETCAST_PUBLIC_BUILD=1`, set in `pages.yml`):** the build **fails** until `imprint.name`, `imprint.street`,
  `imprint.city` and `imprint.email` are filled. Fill `representative` (legal entities), `register`, `vat_id` and
  `content_responsible` too where the law asks for them (section 5 DDG; have a professional review it), and the privacy
  text in `site-content/privacy.*.md`.
- **Apps and web app:** `publisher_name`, `privacy_url` and `imprint_url` (both languages) must name real, reachable pages
  before store submission; the store listings use the same privacy address. Leaving them empty is valid for a private or
  self-hosted build, but the About screen then shows no publisher and no legal links.
