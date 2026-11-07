# Website (cometcast.de)

The project website is a static site in [`site/`](../site), published with GitHub Pages. German is the
default (`/`), English lives under `/en/`. No cookies, no trackers, no external fonts or scripts, no
JavaScript at all.

| Path | Content |
| --- | --- |
| `site/*/index.html`, `site/en/**` | Pages. **Generated** by `node tools/build-site.mjs` from the texts in that file; edit the script, run it, commit both |
| `site/assets/style.css` | Styles (tokens from the web app, system fonts, dark mode through `prefers-color-scheme`) |
| `site/assets/*.svg`, `favicon.svg`, `apple-touch-icon.png` | Copies of the logo files in `assets/logo/` |
| `site/CNAME`, `.nojekyll`, `404.html`, `sitemap.xml`, `robots.txt` | Pages plumbing |
| `tools/check-site-links.mjs` | Dependency-free link and metadata check; runs in the workflow before deploy (`--without-app` for `site/` alone, without flag on the staged artifact that contains `/app/`) |
| `.github/workflows/pages.yml` | Builds the web app (`VITE_BASE=/app/`, WASM PDF renderer, `VITE_DEFAULT_RELAY=wss://relay.cometcast.de/ws`), stages it under `/app/` next to `site/`, and deploys on push to `main` touching `site/**` or the web sources, and on manual start |

Local preview: `node tools/build-site.mjs && node tools/check-site-links.mjs site --without-app && (cd site && python3 -m http.server 8000)`.

The imprint (`/impressum/`, `/en/imprint/`) is a **placeholder with TODO fields**. It must be filled in
by the responsible party before the site goes public; no company data was invented. Likewise the
privacy page needs a proper privacy notice (TODO there). The downloads page copies its status table
from [architecture.md](architecture.md); update both together.

## Before enabling Pages: fill the imprint

- [ ] Fill in the imprint (`/impressum/`, `/en/imprint/`; source: `content.imprint` in `tools/build-site.mjs`). It is a placeholder
      with TODO fields, set to `noindex` and left out of `sitemap.xml`; no data was invented. Remove the placeholder wording and
      the `noindex` only when the real data is in.
- [ ] Fill in the privacy notice (`/datenschutz/`, TODO there).
- [ ] Regenerate (`node tools/build-site.mjs`), run `node tools/check-site-links.mjs site --without-app`, commit.

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
