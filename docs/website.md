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
| `tools/check-site-links.mjs` | Dependency-free link and metadata check; runs in the workflow before deploy |
| `.github/workflows/pages.yml` | Deploy on push to `main` touching `site/**`, and on manual start |

Local preview: `node tools/build-site.mjs && node tools/check-site-links.mjs && (cd site && python3 -m http.server 8000)`.

The imprint (`/impressum/`, `/en/imprint/`) is a **placeholder with TODO fields**. It must be filled in
by the responsible party before the site goes public; no company data was invented. Likewise the
privacy page needs a proper privacy notice (TODO there). The downloads page copies its status table
from [architecture.md](architecture.md); update both together.

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
- **The web app** (Vue + Rust WASM) is also plain static files, so it **can be published on Pages** as well (planned at
  `/app/`; the links on the site already point there and are placeholders, **UNCERTAIN until that work lands**). The source is the
  web build of `apps/web` (`npm run build`, plus `npm run build:wasm` for PDF), which the release workflow's web build zip
  is meant to provide (UNCERTAIN: no release workflow exists in this repository yet). It is **not** deployed by the current `pages.yml`; extending
  it means adding the built files into the uploaded artifact (and the link checker's `ALLOWED_MISSING` entry for `/app/` can then go).
- **The relay server** (`cometcast-server`, Docker image, self-hosted or run by the project at `relay.cometcast.de`) is the only thing that
  needs a running process: it does signalling. A web app on `cometcast.de` talks to it **cross-origin over `wss://`**. Cross-origin means the
  server's `/altcha/challenge` and `/ice-servers` calls need CORS, which the server does not send today (see
  [server.md](server.md#http-endpoints)). A configurable CORS allow-list and a default relay for the web app are separate work and not part of this change.
- The **Docker image still bundles the web app**, so a self-hoster can run one container (same origin, no internet needed) and does not use Pages.

Relay host operations: [deploy/README.md](../deploy/README.md) and the separate repository
[CometCast-Relay](https://github.com/wi-wissen/CometCast-Relay). The relay host does not exist yet.

## Checks that were run when this was written

Link check (`tools/check-site-links.mjs`) passed; HTML validated with the Nu validator (`vnu-jar`, no messages);
screenshots with Chromium at 390 px and 1280 px width in light and dark showed no horizontal overflow. **Not run:** the
workflow itself on GitHub (needs the Pages setting above), real DNS, screen reader testing, other browsers, a Lighthouse run.
