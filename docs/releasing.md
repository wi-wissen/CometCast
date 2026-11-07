# Releasing

**UNCERTAIN: the release workflow (`.github/workflows/release.yml`) has never run on GitHub.** It was checked with `actionlint` and the helper scripts were run locally, nothing else. Every job is **UNCERTAIN until its first real run** (see the table below). Do the first run as a dry run, then fix what breaks.

A release is started by typing a version number into the "Release" workflow (Actions tab, "Run workflow"). The workflow builds every app, then creates a **draft** GitHub Release with all files attached. Nothing is committed: the version is written into the checkout inside each runner by `tools/set-version.mjs`. The draft points at the commit the run started from; GitHub creates the tag when you publish the draft.

## Inputs

| Input | Meaning |
| --- | --- |
| `version` | Semantic version without a leading `v`: `0.1.0`, `1.0.0-rc.1`. No `+build` part. The run fails at once when it is not valid semver, when the tag `v<version>` already exists, or when a release (even a draft) for it exists. |
| `dry_run` | Builds everything and uploads the files and notes as a workflow artefact `dry-run-bundle` (kept 7 days). Creates no release, no tag, and pushes no Docker image. A real release may only be started from the default branch; a dry run may be started from any branch. |
| `skip` | Optional comma-separated list of jobs to leave out (no spaces): `linux,windows,macos,android,ios,tvos,web,server,docker,meta`. Useful while one platform is known to be broken. |

The jobs are independent (`fail-fast: false` everywhere). When one fails, the others still finish, the draft release is still created with what exists, the notes list the failed jobs, and the run is marked red at the end. A second run waits for the first (concurrency group `release`, never cancelled).

## Version numbers

`node tools/set-version.mjs VERSION --build-number N` (N is the workflow run number) sets:

- `Cargo.toml` (`[workspace.package]`) and the workspace entries in `Cargo.lock`, so `cargo build --locked` keeps working.
- `apps/flutter/pubspec.yaml` as `MAJOR.MINOR.PATCH+N`. Android `versionName`/`versionCode`, iOS and macOS `CFBundleShortVersionString`/`CFBundleVersion` and the Windows version resource all derive from it. The script fails if those files stop pointing at Flutter's values. **A pre-release suffix (`-rc.1`) is not put into the Flutter version** because Apple and Windows need numbers; the tag, Cargo, npm, the Docker tag and the file names keep the full version.
- `apps/web/package.json` and `package-lock.json`.
- `apps/apple-tv/project.yml` (`MARKETING_VERSION`, `CURRENT_PROJECT_VERSION`).
- `deploy/docker-compose.yml` (image `ghcr.io/<owner>/cometcast-server:<version>`).
- `crates/cc-ffi/Cargo.toml` `default` feature, in the runner only, to `h264-libloading` for the Flutter builds (see "H.264" below).

Try it in a scratch copy: `git stash -u; node tools/set-version.mjs 0.2.0 && git diff --stat; git checkout -- . && git stash pop`. Self-test: `node --test tools/set-version.test.mjs`.

## What a release contains

| Job | Files | Notes |
| --- | --- | --- |
| `linux` | `cometcast-linux-x86_64-V.tar.gz`, `cometcast_V_amd64.deb` | Flutter bundle with `cc-docs-worker` and `cc-office-worker` (built by the app's CMake step). No AppImage, no arm64. |
| `windows` | `cometcast-windows-x86_64-V.zip` | The Flutter Windows project does **not** build the Rust core itself, so the job builds `cc_ffi.dll` and both workers with cargo and copies them next to the exe. Whether the app then works is unverified. No MSI/installer. |
| `macos` | `cometcast-macos-universal-V*.zip`, `.dmg` | arm64 + x86_64 Rust core joined with `lipo`, copied into the `.app` (same reason as Windows). |
| `android` | `cometcast-android-V*.apk`, `.aab` | arm and arm64, Rust core by cargo-ndk through Gradle. |
| `ios` | `cometcast-ios-V-UNSIGNED.xcarchive.zip`, optional signed `.ipa` | **The iOS project has no Rust core integration** (README: iOS is not built). The archive is a build check, not a working app. |
| `tvos` | `cometcast-tvos-V-UNSIGNED.xcarchive.zip` | `apps/apple-tv` has only an XcodeGen spec; the project is generated in the job. The spec says it was written without Xcode. |
| `web` | `cometcast-web-V.zip` | WASM PDF renderer (`npm run build:wasm`) plus `npm run build`. |
| `server` | `cometcast-server-V-<target>.tar.gz` / `.zip` | linux x86_64 and aarch64, windows x86_64, macOS arm64 and x86_64. The web app is a separate file (`COMETCAST_WEB_DIR`). |
| `docker`, `docker-manifest` | image `ghcr.io/<owner>/cometcast-server:V` (and `latest` for non-pre-releases) | One native build per architecture (amd64, arm64), pushed by digest and joined. Uses `deploy/Dockerfile`. No image SBOM/provenance. |
| `meta` | `cometcast-sbom-rust-V.tar.gz`, `cometcast-sbom-web-V.cdx.json`, `cometcast-deploy-V.tar.gz`, `LICENSE`, `THIRD_PARTY_NOTICES.md` | CycloneDX SBOMs. Dart/Flutter dependencies are not covered. |
| `release` | `SHA256SUMS`, notes | Checksums over every attached file. |

Every archive (tar.gz, zip, dmg, deb, the macOS `.app`) carries `LICENSE`, `THIRD_PARTY_NOTICES.md` and a `VERSION` file. APK, AAB and xcarchive files cannot, so both files are also attached to the release on their own. Note that `THIRD_PARTY_NOTICES.md` itself says nothing is bundled yet and must be reviewed before a public release.

## Secrets (names only)

Set them under Settings, Secrets and variables, Actions. All are optional. A job whose secrets are missing still runs and produces an unsigned file, with a warning and a marker in the file name.

| Secret | Used by | Effect when missing |
| --- | --- | --- |
| `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD` (all four) | android | APK and AAB keep the **debug key** and get the suffix `-debugkey`. Do not distribute them. |
| `MACOS_CERT_P12_BASE64`, `MACOS_CERT_PASSWORD`, `MACOS_SIGN_IDENTITY` | macos | The app is only ad-hoc signed (file name suffix `-adhoc-signed`); users must allow it manually. |
| `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD` (plus the macOS signing secrets) | macos | Not notarised (suffix `-not-notarized`). |
| `IOS_CERT_P12_BASE64`, `IOS_CERT_PASSWORD`, `IOS_PROVISIONING_PROFILE_BASE64` (and the optional repository variable `IOS_EXPORT_METHOD`, default `app-store-connect`) | ios | Only the unsigned `.xcarchive.zip` is produced, no `.ipa`. |

Without secrets these remain unsigned in any case: Linux files, the Windows zip and exe, server binaries, the web zip, the Docker image (pushed with the built-in `GITHUB_TOKEN`, no extra secret), the tvOS archive. Secrets are only passed to the steps that need them and are never printed; keystores and keychains are deleted at the end of the job.

The workflow uses only the built-in `GITHUB_TOKEN`. Permissions are empty at the top and minimal per job: `contents: read` almost everywhere, `packages: write` for the Docker jobs, `contents: write` only for the `release` job.

## Dry run

1. Actions, Release, Run workflow. Choose the branch you want to test, type a version such as `0.0.1-dryrun.1`, tick `dry_run`.
2. Wait. Every job runs; no tag, release or image is created. The job summary of `release` shows the notes and the checksums.
3. Download `dry-run-bundle` from the run page and look at the files.
4. For a first look at one platform only, use `skip` to leave out the others.

A dry run still uses runner minutes on macOS (10x multiplier) and Arm runners; the Android, iOS and Docker jobs are the longest.

## Releasing for real

1. Make sure `main` is what you want to ship (the real run refuses other branches).
2. Run the workflow with `dry_run` off. It checks that the version is new and valid.
3. Open the draft release (Releases page). Read the build-status table, check which files are marked unsigned, download and start the apps you can test.
4. Publish the draft. **Publishing creates the tag `v<version>` on the commit the run started from.** If a job failed and you want its file, fix it and start a new run with a new version; a draft from a failed run can be deleted.

## Tag protection (set once, by an admin)

The workflow never pushes a tag; the tag appears when a person publishes the draft. To keep tags trustworthy, add a repository ruleset (Settings, Rules, Rulesets, New tag ruleset): target `refs/tags/v*`, enforcement active, rules "Restrict deletions" and "Restrict updates" (and "Restrict creations" to maintainers via the bypass list). Without it anyone with write access can move or delete a release tag. This is a GitHub setting, not a file in the repository, so nothing here enforces it.

## H.264 (read before shipping)

`docs/capture.md` says releases must not contain the OpenH264 source build. The Flutter CMake and Gradle steps run a plain `cargo build -p cc-ffi`, which uses the crate's default feature `h264-source`. The workflow therefore switches that default to `h264-libloading` in the runner (`set-version.mjs --h264 libloading`). Owner's decision: releases use Cisco's prebuilt binary and never build OpenH264 from source. The release does not bundle the library: on first use the app asks the user, downloads Cisco's file from `https://ciscobinary.openh264.org/` and verifies a pinned SHA-256 (crate `cc-codec`, section "Runtime codec" in `docs/capture.md`). So the "ships without video" situation no longer applies to the Flutter apps on Linux, Windows and macOS (UNCERTAIN: only the Linux x86_64 download and load was run; Windows and macOS were not). Offline networks: the administrator copies the exact file from `docs/capture.md` into a folder named by `COMETCAST_CODEC_DIR`, or sets `COMETCAST_CODEC_AUTO=1` to let the apps download without asking. **Still true:** the `cometcast-cli` release variant has no consent screen (it finds the library in the places above but does not download it); and Android needs a decision about Google Play policy (see `docs/capture.md`). The server and the web app do not use OpenH264. **Uncertain:** the legal questions in `docs/capture.md` are open (the binary is downloaded from Cisco by the user's device, it is not redistributed by this project).

**When `openh264` or `openh264-sys2` changes in Cargo.lock,** copy the file names and SHA-256 values from `src/blobs/hashes.txt` of the new `openh264-sys2` (in `~/.cargo/registry/src/*/`) into `crates/cc-codec/src/table.rs`, update the version in the docs, and run `cargo test -p cc-codec --features download -- --ignored` (downloads from Cisco).

## Not pinned or not covered

- Actions are pinned to full commit SHAs (resolved 2026-11-07 with `git ls-remote`, the comment names the tag). Bump them by hand or with Dependabot.
- Not pinned: Homebrew `xcodegen` (tvOS job), the Xcode, NDK and Android SDK versions the runner image ships, the `ubuntu-latest`/`macos-15`/`windows-latest` images. Flutter (3.47.9), cargo-ndk (4.1.2), cargo-cyclonedx (0.5.10) and `@cyclonedx/cyclonedx-npm` (6.0.1) are pinned.
- No build caches are used, so a release never reuses a cache that a pull request could have written. Builds are slower for it.

## Verification state (all UNCERTAIN until the first real run)

| Job | State |
| --- | --- |
| prepare (version checks, script self-tests) | UNCERTAIN: script tests run locally, the git/gh checks not run |
| linux | UNCERTAIN: packaging commands run locally with a fake bundle; the Flutter release build was never run (README: `flutter build linux --release` is not verified) |
| windows | UNCERTAIN: nothing run |
| macos | UNCERTAIN: nothing run (signing, lipo, loading the dylib) |
| android | UNCERTAIN: nothing run; the release Gradle path and the re-signing steps are untested |
| ios | UNCERTAIN: nothing run; no Rust core in the iOS project |
| tvos | UNCERTAIN: nothing run; project spec never built |
| web | UNCERTAIN: nothing run in this form (the same commands run in `web.yml`, also unverified on GitHub) |
| server | UNCERTAIN: nothing run in this form |
| docker / docker-manifest | UNCERTAIN: the Dockerfile ran on amd64 only (see its header); arm64 and the push-by-digest join never ran |
| meta | cargo-cyclonedx was run locally and produced one `.cdx.json` per crate; the npm SBOM, the deploy bundle and the job itself never ran |
| release | UNCERTAIN: `gh release create --draft --generate-notes --notes-file` (the notes file is expected to be placed before the generated list) never ran |

The workflow file passes `actionlint` 1.7.7 with `shellcheck` enabled.
