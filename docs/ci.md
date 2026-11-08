# Continuous integration

While the project is under heavy development the check workflows run **by hand only** (Actions tab,
"Run workflow") to save Actions minutes. Before a release the triggers must be switched back on
(section "Re-enabling"). `promote-latest.yml` and `release.yml` are release workflows and are not
part of this page.

## Workflows

Durations are what the GitHub runs of 2026-11-07 showed (`ubuntu-latest`, cold cache) and are
rough; steps after the first failure were never timed.

| Workflow | Jobs and what they run | Duration | Expected state |
|---|---|---|---|
| `rust.yml` | `check`: `cargo fmt --check`, `clippy --workspace --all-targets -D warnings`, `cargo test --workspace`, the `cc-codec` download feature, `cc-ffi` with `h264-libloading`, clippy for Windows/macOS targets of `cc-miracast*` and `cc-capture`, `cc-proto` for wasm. `wayland`: clippy, tests and the PipeWire loopback test of `cc-capture --features wayland`. `release-variant`: release build of the CLI without the OpenH264 source build and its checks | `check` about 10 to 20 min (not seen to the end), `wayland` about 5 min, `release-variant` about 10 min | Green after the clippy fixes of this branch (verified locally with clippy 1.99). Not run on GitHub since |
| `flutter.yml` | `test`: Rust checks of `cc-ffi`, bridge code up to date, `flutter analyze`, `flutter test`, Linux debug build, smoke and end-to-end tests. `android`: analyze, test, debug APK for arm and arm64, 16 KB alignment check, Kotlin unit tests | `android` about 15 min, `test` not seen to the end | Expected green. The Linux build and the smoke and end-to-end steps of `test` and the Kotlin unit tests have never run on GitHub (they were skipped behind earlier failures) |
| `web.yml` | `npm ci`, typecheck, unit tests, build; a second job with the WASM renderer and the Playwright end-to-end tests | not measured | Expected green; typecheck, unit tests and build were run locally, the end-to-end job was not |
| `i18n.yml` | `tools/check-i18n.test.mjs`, `tools/check-i18n.mjs` | under 1 min | Green (run locally) |
| `appletv-core.yml` | Swift package tests in a `swift:6.0.3` container | not measured | Not examined; needs a Swift toolchain |
| `pages.yml` | `build`: generates `site/` from the branding file (`COMETCAST_BRANDING`, `COMETCAST_PUBLIC_BUILD=1`: fails while the imprint is empty), link check of `site/`, wasm-bindgen CLI, web app build, staging, link check of the staged artifact, upload. `deploy`: only when GitHub Pages is enabled | about 3 min | `build` green. `deploy` needs an owner setting, see below |

### Pages needs an owner setting

All three recorded Pages failures were the step `actions/configure-pages` answering
`Get Pages site failed ... Not Found`. The build and both link checks had passed. The cause is that
GitHub Pages is not enabled for `wi-wissen/CometCast`: a repository owner must set
**Settings, Pages, Build and deployment, Source = GitHub Actions** (more in `docs/website.md`; fill
the imprint first). No code change can do that. The workflow now detects it: `deploy` writes a
warning and a job summary and skips the deploy, so the run shows the real result of the build and the
checks. A manual start with the input `require_pages` ticked fails with the same message instead
(use it for a release, where a skipped deploy must not pass unnoticed).

## Re-enabling the triggers before a release

In each file replace the `on:` block (keep the comment out, it is only there for the manual mode).
These are the blocks that were in place before they were switched off; `workflow_dispatch` stays
so that runs can still be started by hand.

`rust.yml`

```yaml
on:
  push:
    branches: [main]
  pull_request:
  workflow_dispatch:
```

`flutter.yml`

```yaml
on:
  push:
    branches: [main]
    paths:
      - 'apps/flutter/**'
      - 'crates/cc-ffi/**'
      - 'crates/cc-receiver/**'
      - 'crates/cc-sender/**'
      - 'crates/cc-transport/**'
      - 'crates/cc-proto/**'
      - 'crates/cc-files/**'
      - 'crates/cc-docs/**'
      - 'crates/cc-capture/**'
      - 'crates/cc-media/**'
      - 'crates/cc-discovery/**'
      - '.github/workflows/flutter.yml'
  pull_request:
    paths:
      - 'apps/flutter/**'
      - 'crates/cc-ffi/**'
      - 'crates/cc-receiver/**'
      - 'crates/cc-sender/**'
      - 'crates/cc-transport/**'
      - 'crates/cc-proto/**'
      - 'crates/cc-files/**'
      - 'crates/cc-docs/**'
      - 'crates/cc-capture/**'
      - 'crates/cc-media/**'
      - 'crates/cc-discovery/**'
      - '.github/workflows/flutter.yml'
  workflow_dispatch:
```

`web.yml`

```yaml
on:
  push:
    paths:
      - 'apps/web/**'
      - 'crates/cc-docs-wasm/**'
      - 'crates/cc-docs/**'
      - 'Cargo.lock'
      - 'tests/fixtures/**'
      - '.github/workflows/web.yml'
  pull_request:
    paths:
      - 'apps/web/**'
      - 'crates/cc-docs-wasm/**'
      - 'crates/cc-docs/**'
      - 'Cargo.lock'
      - 'tests/fixtures/**'
      - '.github/workflows/web.yml'
  workflow_dispatch:
```

`i18n.yml`

```yaml
on:
  push:
  pull_request:
  workflow_dispatch:
```

`appletv-core.yml`

```yaml
on:
  push:
    paths:
      - 'apps/apple-tv/CometCastCore/**'
      - 'apps/apple-tv/Sources/CometCastTV/Localizable.xcstrings'
      - 'tests/fixtures/**'
      - '.github/workflows/appletv-core.yml'
  pull_request:
    paths:
      - 'apps/apple-tv/CometCastCore/**'
      - 'apps/apple-tv/Sources/CometCastTV/Localizable.xcstrings'
      - 'tests/fixtures/**'
      - '.github/workflows/appletv-core.yml'
  workflow_dispatch:
```

`pages.yml` (keep the `require_pages` input)

```yaml
on:
  push:
    branches: [main]
    paths:
      - 'site/**'
      - 'apps/web/**'
      - 'crates/cc-docs-wasm/**'
      - 'crates/cc-docs/**'
      - 'Cargo.lock'
      - 'tools/check-site-links.mjs'
      - '.github/workflows/pages.yml'
  workflow_dispatch:
    inputs:
      require_pages:
        description: 'Fail (instead of skipping the deploy) when GitHub Pages is not enabled for the repository'
        type: boolean
        default: false
```

Before switching on, start every workflow once by hand and check that it is green.

## Known causes of red runs (fixed, keep in mind)

- `rust-toolchain.toml` and the workflows use the newest stable Rust, so a new clippy lint can turn
  `-D warnings` red without any code change (`clippy::chunks_exact_to_as_chunks`, clippy 1.99). Reproduce
  with the same toolchain before pushing: `cargo clippy --workspace --all-targets -- -D warnings`.
- 32-bit ARM Android libraries are not 16 KB aligned by default: `.cargo/config.toml` passes
  `-Wl,-z,max-page-size=16384` for `armv7-linux-androideabi`.

## Docker image without a release

`docker-image.yml` (manual only) builds `deploy/Dockerfile` for linux/amd64 and pushes
`ghcr.io/<owner>/cometcast-server:edge` and `:sha-<commit>`. Inputs: `branding` (empty = default,
`branding/cometcast.json` = official) and `push` (off = build only). Versioned multi-arch images and
`latest` still come only from `release.yml` and `promote-latest.yml`. A pushed edge image gets a signed
build provenance attestation like a release (`actions/attest-build-provenance`; the job has `id-token: write`
and `attestations: write` for it; verify with `gh attestation verify oci://ghcr.io/<owner>/cometcast-server:edge
--owner <owner>`, see `docs/releasing.md`). UNCERTAIN: the workflow never ran
on GitHub. A package on ghcr.io is private at first; make it public in the package settings once.
