# Website (cometcast.de)

The project website is a static site in [`site/`](../site), published with GitHub Pages. English is the default (`/`), German
lives under `/de/`. No cookies, no trackers, nothing loaded from third parties (fonts included). The design is specified in
[design/website.md](design/website.md) (with the agreed look [design/website-pitch.html](design/website-pitch.html)).

| Path | Content |
| --- | --- |
| `site/**/index.html` | Pages. **Generated** by `COMETCAST_BRANDING=branding/cometcast.json node tools/build-site.mjs` from the texts below; edit those, run it, commit both |
| `tools/site/text.en.mjs`, `text.de.mjs` | All page text of the home, download, how-it-works, open-source and contact pages and the interface strings, one file per language. The build **fails** when a key or a block of HTML is missing in one language |
| `tools/site/finder.mjs` | The finder "What do you need?": devices, pictograms and the answer matrix (both languages side by side) |
| `tools/site/pages.mjs` | Page list, paths in both languages, the redirect table (shared with the link checker) |
| `site-content/*.md` | Privacy, imprint and "host it yourself" (`privacy.*.md`, `imprint.*.md`, `self-hosting.*.md`): front matter, headings, paragraphs, lists, notes, inline HTML, and placeholders (`{{imprint.name\|hint}}`, `{{publisher_name}}`, `{{todo:text}}`, `{{#if ...}}`; format described at the top of the renderer in `tools/build-site.mjs`) |
| `branding/*.json` | Publisher name, links and the imprint operator data ([branding/README.md](../branding/README.md)). The domain shown on the display mock is the host of the legal URLs in that file |
| `site/assets/style.css`, `site/assets/site.js` | Written by hand. Light and dark tokens (dark through `prefers-color-scheme`), and the one script of the site (language hint, finder, device highlight on the download page) |
| `site/assets/fonts.css`, `site/assets/fonts/` | **Generated** by the build from the npm `@fontsource` packages in `tools/site/package.json` (Bricolage Grotesque 700/800, Atkinson Hyperlegible 400/700, JetBrains Mono 500/700; latin and latin-ext, woff2). Install them with `npm ci --prefix tools/site`; without them the build keeps the committed copies. OFL 1.1, listed in `THIRD_PARTY_NOTICES.md` and on the Open source page |
| `site/assets/*.svg`, `favicon.svg`, `apple-touch-icon.png` | Copies of the logo files in `assets/logo/` |
| `site/CNAME`, `.nojekyll`, `404.html`, `sitemap.xml`, `robots.txt` | Pages plumbing |
| `tools/check-site-links.mjs` | Dependency-free check: internal links and anchors, hreflang pairs, both languages present, the redirects, nothing from third parties, font files. Runs in the workflow before deploy (`--without-app` for `site/` alone, without flag on the staged artifact that contains `/app/`) |
| `tools/site-screenshots.mjs` | Playwright screenshots of home, how it works and download (EN and DE, light and dark, 1440 and 390 px): `node tools/site-screenshots.mjs OUT_DIR [--base URL]` |
| `.github/workflows/pages.yml` | Installs the font packages, generates the site from the branding file (public build, fails while the imprint is empty), builds the web app (`VITE_BASE=/app/`, WASM PDF renderer, `VITE_DEFAULT_RELAY=wss://relay.cometcast.de/ws`), stages it under `/app/` next to `site/`, and deploys. Started by hand and when a GitHub release is published |

Local preview: `npm ci --prefix tools/site && COMETCAST_BRANDING=branding/cometcast.json node tools/build-site.mjs --offline && node tools/check-site-links.mjs site --without-app && (cd site && python3 -m http.server 8000)`.

## Pages

| | English | German |
| --- | --- | --- |
| Home (hero, finder, three steps) | `/` | `/de/` |
| Download | `/download/` | `/de/download/` |
| How it works (technical) | `/how-it-works/` | `/de/so-funktionierts/` |
| Privacy | `/privacy/` | `/de/datenschutz/` |
| Host it yourself | `/self-hosting/` | `/de/selbst-hosten/` |
| Open source | `/open-source/` | `/de/open-source/` |
| Contact | `/contact/` | `/de/kontakt/` |
| Imprint | `/imprint/` | `/de/impressum/` |

Every page has `hreflang` links (`en`, `de`, `x-default` = English) to its counterpart, and the language switch in the header leads to
the counterpart page. **Language hint:** a small bar (shown by `site.js` only) when the browser's first `en` or `de` language is the other one;
closing it or following its link is remembered in `localStorage`; there is never an automatic redirect.

**Redirects** (small pages with meta refresh, canonical and a link, `noindex`; table `REDIRECTS` in `tools/site/pages.mjs`): the old German-first paths
`/funktionsweise/`, `/downloads/`, `/datenschutz/`, `/kontakt/`, `/impressum/` lead to the `/de/` pages, the old `/en/...` paths
(`/en/`, `/en/how-it-works/`, `/en/downloads/`, `/en/privacy/`, `/en/open-source/`, `/en/contact/`, `/en/imprint/`) to the new English paths.
The old German `/open-source/` is the English page now and cannot redirect; the old German home `/` is English now (the language hint helps).
Short links: `/d/` (English download page) and `/de/d/` (German), and `/apk/` (the latest Android APK of the release, else the Android card of the
download page). None of them can collide with a join code (6 characters) or `/app/`.

**Download page.** The build asks `api.github.com/repos/<owner>/<repo>/releases/latest` once (`GITHUB_TOKEN` is used when set, only against the rate limit)
and writes the file links of that release into the page by file-name pattern (names made by `release.yml`: `cometcast-windows-*.zip|msi|exe`,
`cometcast-macos-*.dmg|zip`, `cometcast_*_amd64.deb`, `cometcast-linux-*.tar.gz`, `cometcast-android-*.apk`; files with `debugkey`, `unsigned` or
`unverified` in the name are left out). `--offline` (or `COMETCAST_OFFLINE=1`), no release, or a failed call: every card says "Coming soon". The visitor's browser loads nothing from GitHub
until a download is clicked. App store links: constant `STORES` in `tools/build-site.mjs` (empty until the listings exist). `site.js` moves the card that fits the
visitor's device to the top ("For your Mac"); without JavaScript all cards are listed. Because the page follows the latest release, `pages.yml` also runs when a release is
published (build from the default branch). The committed `site/` is generated with `--offline`.

**Finder without JavaScript.** The static HTML holds the full answer table (every device and screen pair with all ways, each way with the note about the network it needs);
`site.js` hides it and shows the picker (device, connection, screen), in the same markup; the choice can be linked (`#win-appletv`, `#iphone-appletv-diff`).

## Before enabling Pages: fill the imprint

- [ ] Fill in the `imprint` section of `branding/cometcast.json` (`/de/impressum/`, `/imprint/`; text: `site-content/imprint.*.md`). While it is
      incomplete the page is a placeholder with TODO fields, set to `noindex` and left out of `sitemap.xml`; no data was invented. The placeholder wording goes
      away by itself when the required fields are in (`noindex` stays). `pages.yml` refuses to build until then.
- [ ] Fill in the privacy notice (`/de/datenschutz/`, `/privacy/`; TODO in `site-content/privacy.*.md`).
- [ ] Regenerate (`COMETCAST_BRANDING=branding/cometcast.json node tools/build-site.mjs`), run `node tools/check-site-links.mjs site --without-app`, commit.

## What the owner must do once

1. **Settings -> Pages -> Build and deployment -> Source = GitHub Actions** (repository `wi-wissen/CometCast`).
   Then push to `main` (or run the "Pages" workflow by hand). The first run creates the `github-pages` environment.
2. **Custom domain:** Settings -> Pages -> Custom domain = `cometcast.de`. The workflow publishes a `CNAME`
   file already (`site/CNAME`); with Actions deployment GitHub takes the domain from the Pages settings, so enter it there too.
3. **DNS** at the registrar of `cometcast.de`. **UNCERTAIN: written from memory, compare with GitHub's
   current page "Managing a custom domain for your GitHub Pages site" before use.**

   | Name | Type | Value |
   | --- | --- | --- |
   | `cometcast.de` (apex) | A | `185.199.108.153`, `185.199.109.153`, `185.199.110.153`, `185.199.111.153` |
   | `cometcast.de` (apex) | AAAA | `2606:50c0:8000::153`, `2606:50c0:8001::153`, `2606:50c0:8002::153`, `2606:50c0:8003::153` |
   | `www.cometcast.de` | CNAME | `wi-wissen.github.io.` |

   Do not use wildcard DNS records for the domain. Optionally verify the domain in the account/organisation
   settings (Pages -> Add a domain) to prevent takeovers (UNCERTAIN: exact menu name).
4. **Enforce HTTPS** (Settings -> Pages) once the certificate is issued. GitHub issues it after DNS resolves; this can take
   a while (UNCERTAIN: no fixed time).
5. **Relay host:** keep `relay.cometcast.de` as a separate DNS name (an `A`/`AAAA` record to the relay machine,
   not to GitHub). It is not part of Pages. See below.

## Website, web app and relay

- **The site (`cometcast.de`)** is static files on Pages.
- **The web app** (Vue + Rust WASM) is plain static files and is **published on Pages under `/app/`** by `pages.yml`: it
  runs `npm run build:wasm` and `npm run build` in `apps/web` with `VITE_BASE=/app/` and
  `VITE_DEFAULT_RELAY=wss://relay.cometcast.de/ws`, copies `dist/` to `/app/` next to `site/`, asserts that
  `app/index.html` and the PDF renderer exist, and runs the link checker on the staged directory. (This workflow has not been run on
  GitHub yet.) The app can talk to the relay only once `relay.cometcast.de` exists (see the next point).
- **The relay server** (`cometcast-server`, Docker image, self-hosted or run by the project at `relay.cometcast.de`) is the only thing that
  needs a running process: it does signalling. A web app on `cometcast.de` talks to it **cross-origin over `wss://`**. Cross-origin means the
  server's `/altcha/challenge` and `/ice-servers` calls need CORS: the relay has a configurable `allowed_origins` list
  (set `["https://cometcast.de"]`; also an `Origin` check on `/ws`, see [server.md](server.md#web-app-on-another-origin)).
  The web app picks its relay by `?server=`, then an optional `config.json` next to `index.html`, then the build-time
  `VITE_DEFAULT_RELAY`, then its own origin ([apps/web/README.md](../apps/web/README.md#static-hosting-and-relay-selection)).
  For `https://cometcast.de/app/` build with `VITE_BASE=/app/ VITE_DEFAULT_RELAY=wss://relay.cometcast.de/ws` (or ship a
  `config.json`, example: [deploy/config/web-app-config.example.json](../deploy/config/web-app-config.example.json)).
  Checked locally with the app on a second static origin (`npm run e2e:static`); **UNCERTAIN: real GitHub Pages
  (project path `/app/`, 404 handling for the optional `config.json`) and real DNS were not tried.**
- **Static hosting needs three paths answered with `index.html`.** The pages of the web app are chosen by path, and the QR
  code on the display leads to `https://{dom}/{CODE}`: so **`/display`**, **`/{CODE}`** (a 6-character join code, any code)
  and **`/app`** must all serve the app's `index.html`, otherwise the QR code leads to a 404 (spec
  [first-run-and-display.md](design/first-run-and-display.md), 7.2). `cometcast-server` does this itself. A static host has to
  be told: on GitHub Pages put a copy of `index.html` named `404.html` next to it (Pages serves `404.html` for every unknown
  path, and the app then picks the page from the address). Under a base path such as `/app/` the same three paths are
  `/app/display`, `/app/{CODE}` and `/app`. **cometcast.de does this with its own `site/404.html`:** a small script there fetches
  `/app/index.html` and shows it in place when the path starts with `/app/` (the address stays, the app picks the page from it); every other path
  keeps the website's 404. `tools/check-site-links.mjs` checks that the script is there. **UNCERTAIN: not tried on the real Pages host.**
  The join address of the public relay is `cometcast.de/app` (decision N85), so the QR code leads to `https://cometcast.de/app/{CODE}`.
  `pages.yml` also stages `deploy/well-known/` as `/.well-known/` (app links of the apps).
- The **Docker image still bundles the web app**, so a self-hoster can run one container (same origin, no internet needed) and does not use Pages.

Relay host operations: [deploy/README.md](../deploy/README.md) and the separate repository
[CometCast-Relay](https://github.com/wi-wissen/CometCast-Relay). The relay host does not exist yet.

## Checks that were run when this was written

Link check (`tools/check-site-links.mjs`) passed; HTML validated with the Nu validator (`vnu-jar`, no messages);
screenshots with Chromium at 390 px and 1280 px width in light and dark showed no horizontal overflow. **Not run:** the
workflow itself on GitHub (needs the Pages setting above), real DNS, screen reader testing, other browsers, a Lighthouse run.
