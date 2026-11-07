# Releasing

**UNCERTAIN: the release workflow (`.github/workflows/release.yml`) has never run on GitHub.** It was checked with `actionlint` and the helper scripts were run locally, nothing else. Every job is **UNCERTAIN until its first real run** (see the table below). Do the first run as a dry run, then fix what breaks.

A release is started by typing a version number into the "Release" workflow (Actions tab, "Run workflow"). The workflow builds every app, then creates a **draft** GitHub Release with all files attached. Nothing is committed: the version is written into the checkout inside each runner by `tools/set-version.mjs`. The draft points at the commit the run started from; GitHub creates the tag when you publish the draft.

## Inputs

| Input | Meaning |
| --- | --- |
| `version` | Semantic version without a leading `v`: `0.1.0`, `1.0.0-rc.1`. No `+build` part. The run fails at once when it is not valid semver, when the tag `v<version>` already exists, when any release or draft with that tag exists (`gh release list --json tagName,isDraft`, authenticated, so private repositories work), or, in a real run, when the Docker tag `<image>:<version>` is already on ghcr.io (a pushed version is never overwritten, so a re-run of an already pushed version is blocked). |
| `dry_run` | Builds everything and uploads the files and notes as a workflow artefact `dry-run-bundle` (kept 7 days). Creates no release, no tag, and pushes no Docker image. A real release may only be started from the default branch; a dry run may be started from any branch. |
| `skip` | Optional comma-separated list of jobs to leave out (no spaces): `linux,windows,macos,android,ios,tvos,web,server,docker,meta`. Useful while one platform is known to be broken. Every entry is checked against this list: a typo or a space fails the run at once instead of silently building everything. |

The jobs are independent (`fail-fast: false` everywhere). When one fails, the others still finish, the draft release is still created with what exists, the notes list the failed jobs, and the run is marked red at the end. Concurrency group `release`: a running run is never cancelled, but GitHub keeps **at most one waiting run** per group. A second run waits; a third one started meanwhile **cancels the waiting second one** (that is GitHub's rule, `cancel-in-progress: false` does not change it). Start the next run only after the previous one has finished.

The `release` job fails (and creates no draft) when the downloaded files contain **no platform artefact at all** (only LICENSE, SBOMs and the deploy bundle, or nothing): a "release" without any app is never created.

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
| `linux` | `cometcast-linux-x86_64-V.tar.gz`, `cometcast_V_amd64.deb` | Flutter bundle with `cc-docs-worker` and `cc-office-worker` (built by the app's CMake step). No AppImage, no arm64. Built on **ubuntu-22.04** (see "Linux baseline"). `.deb` Depends: `libc6 (>= 2.35), libgtk-3-0 (>= 3.24.33), libglib2.0-0 (>= 2.72), libstdc++6 (>= 12), libgcc-s1 (>= 12)` (hand-written minimums, not derived with `dpkg-shlibdeps`: UNCERTAIN). |
| `windows` | `cometcast-windows-x86_64-V.zip` | The Flutter Windows project does **not** build the Rust core itself, so the job builds `cc_ffi.dll` and both workers with cargo and copies them next to the exe, together with the Visual C++ runtime (`vcruntime140.dll`, `vcruntime140_1.dll`, `msvcp140.dll`, taken from the runner's Visual Studio `VC/Redist/MSVC/*/x64/Microsoft.VC*.CRT`). Whether the app then works is unverified. No MSI/installer. See "Windows runtime". |
| `macos` | `cometcast-macos-universal-V*.zip`, `.dmg` | arm64 + x86_64 Rust core joined with `lipo`, copied into the `.app` (same reason as Windows): `libcc_ffi.dylib` in `Contents/Frameworks`, the workers in `Contents/MacOS`. See "macOS signing". |
| `android` | `cometcast-android-V*.apk`, `.aab` | arm and arm64, Rust core by cargo-ndk through Gradle. |
| `ios` | `cometcast-ios-V-UNSIGNED-NO-CORE.xcarchive.zip`, optional `cometcast-ios-V-NO-CORE.ipa` | **The iOS project has no Rust core integration** (README: iOS is not built). **Even the signed `.ipa` has no core and is NOT shippable**; `NO-CORE` is in both names so nobody mistakes them for a release app. They are a build check. |
| `tvos` | without secrets: `cometcast-tvos-V-UNSIGNED.xcarchive.zip`; with secrets: `cometcast-tvos-V.ipa` and `cometcast-tvos-V.xcarchive.zip` | `apps/apple-tv` has an XcodeGen spec (app target, unit-test target, scheme `CometCastTV`, local package `CometCastCore`, the LiveKit WebRTC package pinned by commit); the project is generated in the job. The spec and the app were written without Xcode and never built; the job needs the network for the 686 MB WebRTC package. Names checked against [appletv.md](appletv.md#project-and-release-workflow). Signing: see "tvOS signing". |
| `web` | `cometcast-web-V.zip` | WASM PDF renderer (`npm run build:wasm`) plus `npm run build`. |
| `server` | `cometcast-server-V-<target>.tar.gz` / `.zip` | linux x86_64 and aarch64, windows x86_64, macOS arm64 and x86_64. The web app is a separate file (`COMETCAST_WEB_DIR`). |
| `docker`, `docker-manifest` | image `ghcr.io/<owner>/cometcast-server:V` (**only** the version tag; `latest` is promoted separately) | One native build per architecture (amd64, arm64), pushed by digest and joined. Uses `deploy/Dockerfile`. No image SBOM/provenance. **The image contains no PDF viewer** (the server does not render PDFs; the browser app does). See "Docker image and latest". |
| `meta` | `cometcast-sbom-rust-V.tar.gz`, `cometcast-sbom-web-V.cdx.json`, `cometcast-deploy-V.tar.gz`, `LICENSE`, `THIRD_PARTY_NOTICES.md` | CycloneDX SBOMs. Dart/Flutter dependencies are not covered. |
| `release` | `SHA256SUMS`, notes | Checksums over every attached file. |

Every archive (tar.gz, zip, dmg, deb, the macOS `.app`) carries `LICENSE`, `THIRD_PARTY_NOTICES.md` and a `VERSION` file. APK, AAB and xcarchive files cannot, so both files are also attached to the release on their own. Note that `THIRD_PARTY_NOTICES.md` itself says nothing is bundled yet and must be reviewed before a public release.

## Secrets (names only) and the `release` Environment

All secrets are optional. A job whose secrets are missing still runs and produces an unsigned file, with a warning and a marker in the file name.

**Signing secrets belong in a GitHub Environment named `release`, not in repository secrets.** The signing jobs (`android`, `macos`, `ios`, `tvos`) declare `environment: release` in a **real** run, so the secrets are handed out only to a job that passed the environment's protection rules. A **dry run uses no environment and therefore no secrets**: every job builds unsigned (and the signing steps never run; the signing path is only exercised by a real run, which is why it stays UNCERTAIN longer). The expression is `environment: ${{ !inputs.dry_run && 'release' || '' }}`; an empty name meaning "no environment" is the commonly used behaviour but **UNCERTAIN until the first run**.

### One-time setup (repository admin)

1. Settings, Environments, **New environment**, name exactly `release`.
2. **Required reviewers**: add at least one person other than the one who usually starts releases (and leave "Prevent self-review" on if the repository has two maintainers). Each signing job then waits for approval; approve once per job in the run page ("Review deployments", you can tick all four environments at once).
3. **Deployment branches and tags**: "Selected branches and tags", add the rule `main`. A job on any other branch cannot use the environment (real runs are limited to the default branch anyway; this makes the secret protection independent of the workflow check).
4. Add the secrets below as **Environment secrets** of `release` (not "Repository secrets"). Optional repository or environment **variables**: `IOS_EXPORT_METHOD`, `TVOS_EXPORT_METHOD`.
5. **Delete any signing secret with the same name from the repository-level secrets.** A repository-level secret is visible to every job and every workflow, dry runs included, and would defeat the point.
6. Check: start a dry run; the signing jobs must not wait for approval and must print the "secrets are missing" warnings. Then a real run must pause in the signing jobs.

| Secret | Used by | Effect when missing |
| --- | --- | --- |
| `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD` (all four) | android | APK and AAB keep the **debug key** and get the suffix `-debugkey`. Do not distribute them. |
| `MACOS_CERT_P12_BASE64`, `MACOS_CERT_PASSWORD`, `MACOS_SIGN_IDENTITY` | macos | The app is only ad-hoc signed (file name suffix `-adhoc-signed`); users must allow it manually. |
| `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD` (plus the macOS signing secrets) | macos | Not notarised (suffix `-not-notarized`). |
| `IOS_CERT_P12_BASE64`, `IOS_CERT_PASSWORD`, `IOS_PROVISIONING_PROFILE_BASE64` | ios | Only the unsigned `-UNSIGNED-NO-CORE.xcarchive.zip`, no `.ipa`. |
| `TVOS_PROVISIONING_PROFILE_BASE64` (certificate: reuses `IOS_CERT_P12_BASE64` and `IOS_CERT_PASSWORD`, same Apple Distribution certificate; optional overrides `TVOS_CERT_P12_BASE64`, `TVOS_CERT_PASSWORD`) | tvos | Only the unsigned `-UNSIGNED.xcarchive.zip`. |

Without secrets these remain unsigned in any case: Linux files, the Windows zip and exe, server binaries, the web zip, the Docker image (pushed with the built-in `GITHUB_TOKEN`, no extra secret). Secrets are only passed to the steps that need them and are never printed; keystores and keychains are deleted at the end of the job (`always()`).

The workflow uses only the built-in `GITHUB_TOKEN`. Permissions are empty at the top and minimal per job: `contents: read` almost everywhere, `packages: write` for the Docker jobs, `contents: write` for the `release` job and for the small `prepare` guard job (a token with only read access cannot see draft releases, so the guard needs write; that job has no checkout of repository code beyond one script and no build step).

### macOS signing (UNCERTAIN until run on a Mac)

- `rust_loader.dart` looks for `libcc_ffi.dylib` in `<exeDir>/../Frameworks` first (that is `Contents/Frameworks` of the `.app`, where the workflow puts it), then `<exeDir>/lib`, then the plain name. Linux (`<exeDir>/lib`), Windows (`lib\`, then next to the exe) and Android/iOS (system loader) are unchanged. The order is unit-tested with injected paths (`apps/flutter/test/rust_loader_test.dart`).
- `Release.entitlements` (app): `app-sandbox`, `network.client` and `network.server` (without them the sandboxed app has no sockets), `files.user-selected.read-only` (file picker). `Helper.entitlements` (the two workers in `Contents/MacOS`): `app-sandbox` + `inherit`; a helper signed without them is killed by macOS when the sandboxed app starts it.
- Signing order, no `--deep`: every `.dylib` and `.framework` in `Contents/Frameworks` (deepest first, including `libcc_ffi.dylib`), then the workers with `Helper.entitlements`, then the app with the app entitlements. `codesign --verify --deep --strict` only verifies.
- The ad-hoc branch (no secrets) signs in the same order with the same entitlements, without hardened runtime.
- **Hardened runtime and library validation:** the Developer ID variant signs with `--options runtime`, which turns on library validation: the app may then load only libraries signed by Apple or by the same team. `libcc_ffi.dylib` is ours (fine), but the Cisco `libopenh264` the user supplies is signed by Cisco, so loading it would be refused. **Only in that variant** the workflow adds `com.apple.security.cs.disable-library-validation` to the app's entitlements (a temporary copy, edited with PlistBuddy; the committed file does not have it). Cost: the app can load any signed library. The codec manager has landed: the library is now a file that the app downloaded from Cisco into the per-user folder and loads from there, so this is the case that matters; Cisco's dylib may be refused without the entitlement, and may still be refused with it if the signature or notarisation rules say so (UNCERTAIN). **Not tested:** whether it is enough, whether the sandbox lets the app read a dylib the user points to, and whether notarisation accepts the entitlement (it normally does).
- Notarisation: `notarytool store-credentials` saves the app-specific password once into the job's temporary keychain as profile `cometcast-notary`; `submit`/`log` use `--keychain-profile`. The password is still an argument of that one `store-credentials` call. The result JSON is checked for `Accepted`.

### tvOS signing (UNCERTAIN: never run)

Like iOS: with the secrets the job imports the certificate into a temporary keychain, installs the profile, runs `xcodebuild archive` with manual signing (`CODE_SIGN_STYLE=Manual`, identity `Apple Distribution`, team and profile name read from the profile) and `xcodebuild -exportArchive` (method from the variable `TVOS_EXPORT_METHOD`, else `IOS_EXPORT_METHOD`, else `app-store-connect`), and attaches the signed `.ipa` and the signed `.xcarchive.zip`. Without them it attaches the unsigned archive only. Keychain and profile are removed with `always()`.

| Item | Name | Required | Notes |
| --- | --- | --- | --- |
| Certificate | `IOS_CERT_P12_BASE64` + `IOS_CERT_PASSWORD` | yes (or the override) | Apple Distribution, shared with iOS |
| Certificate override | `TVOS_CERT_P12_BASE64` + `TVOS_CERT_PASSWORD` | no | only if tvOS uses another certificate |
| Provisioning profile | `TVOS_PROVISIONING_PROFILE_BASE64` | yes | tvOS profile for bundle id `de.cometcast.tv` (from `project.yml`), base64 of the `.mobileprovision` |
| Export method | variable `TVOS_EXPORT_METHOD` / `IOS_EXPORT_METHOD` | no | default `app-store-connect` |

## Dry run

1. Actions, Release, Run workflow. Choose the branch you want to test, type a version such as `0.0.1-dryrun.1`, tick `dry_run`.
2. Wait. Every job runs; no tag, release or image is created. The job summary of `release` shows the notes and the checksums.
3. Download `dry-run-bundle` from the run page and look at the files.
4. For a first look at one platform only, use `skip` to leave out the others.

A dry run does not use the `release` Environment, so it asks for no approval and builds every platform unsigned.

A dry run still uses runner minutes on macOS (10x multiplier) and Arm runners; the Android, iOS and Docker jobs are the longest.

## Releasing for real

1. Make sure `main` is what you want to ship (the real run refuses other branches).
2. Run the workflow with `dry_run` off. It checks that the version is new and valid.
3. Open the draft release (Releases page). Read the build-status table, check which files are marked unsigned, download and start the apps you can test.
4. Publish the draft. **Publishing creates the tag `v<version>` on the commit the run started from.** If a job failed and you want its file, fix it and start a new run with a new version; a draft from a failed run can be deleted.

## Tag protection (set once, by an admin)

The workflow never pushes a tag; the tag appears when a person publishes the draft. To keep tags trustworthy, add a repository ruleset (Settings, Rules, Rulesets, New tag ruleset): target `refs/tags/v*`, enforcement active, rules "Restrict deletions" and "Restrict updates" (and "Restrict creations" to maintainers via the bypass list). Without it anyone with write access can move or delete a release tag. This is a GitHub setting, not a file in the repository, so nothing here enforces it.

## Linux baseline

The Linux desktop bundle, `.deb` and the Linux server binaries are built on **ubuntu-22.04** (the arm64 server on `ubuntu-22.04-arm`), not `ubuntu-latest`. A binary needs a glibc at least as new as the one it was linked against, so the oldest runner image sets the floor: **glibc 2.35, meaning Ubuntu 22.04 and newer, Debian 12 (glibc 2.36) and newer. Debian 11 and Ubuntu 20.04 are not supported.** Decision: 22.04 rather than 24.04 so Debian 12 stays supported; revisit when Ubuntu 22.04 runners are retired (April 2027 end of standard support). The Docker image is independent (its own base image).

Rust is pinned to one exact version, `RUST_VERSION` at the top of `release.yml` (currently 1.97.0, above the workspace `rust-version` 1.92). The composite action `.github/actions/rust-toolchain` runs `rustup toolchain install` and sets `RUSTUP_TOOLCHAIN`, which overrides `rust-toolchain.toml` (that file says `stable`) inside the release jobs only. Bump the one variable by hand and run a dry run. The Dockerfile pins its own compiler.

## Windows runtime

The Windows zip carries `vcruntime140.dll`, `vcruntime140_1.dll` and `msvcp140.dll` next to the exe (app-local deployment of the redistributable DLLs from the runner's Visual Studio). Decision: bundle the DLLs instead of linking the CRT statically (`-C target-feature=+crt-static`), because Flutter's own engine and runner link the dynamic CRT and need these DLLs anyway; a static CRT for only the Rust parts would mix two runtimes without removing the dependency. A clean Windows without the runtime therefore still starts the app. The job fails when one of the three DLLs is not found. Check that the Visual Studio redistributable licence terms suit your distribution. UNCERTAIN: never run.

## Docker image and latest

- The release run pushes **only** `ghcr.io/<owner>/cometcast-server:<version>`. It refuses a version whose tag already exists (checked in `prepare` and again right before the join with `docker buildx imagetools inspect`), so re-running a workflow for an already pushed version fails instead of overwriting the image.
- `latest` is moved by the separate manual workflow **Promote latest** (`.github/workflows/promote-latest.yml`, input `version`), after you published the GitHub release. It refuses: a pre-release version, a release that is a draft or missing, a missing image, and a version **lower** than the one `latest` points to now (the current one is found by comparing digests with the published releases; if it cannot be determined the run fails). The same version again is a no-op. Only the default branch may start it.
- **The image has no PDF viewer.** The server relays and stores files; it does not render or display PDFs. Viewing happens in the clients (the web app renders with the WASM renderer).

## H.264 (read before shipping)

`docs/capture.md` says releases must not contain the OpenH264 source build. The Flutter CMake and Gradle steps run a plain `cargo build -p cc-ffi`, which uses the crate's default feature `h264-source`. The workflow therefore switches that default to `h264-libloading` in the runner (`set-version.mjs --h264 libloading`). Owner's decision: releases use Cisco's prebuilt binary and never build OpenH264 from source. The release does not bundle the library: on first use the app asks the user, downloads Cisco's file from `https://ciscobinary.openh264.org/` and verifies a pinned SHA-256 (crate `cc-codec`, section "Runtime codec" in `docs/capture.md`). So the "ships without video" situation no longer applies to the Flutter apps on Linux, Windows and macOS (UNCERTAIN: only the Linux x86_64 download and load was run; Windows and macOS were not). Offline networks: the administrator copies the exact file from `docs/capture.md` into a folder named by `COMETCAST_CODEC_DIR`, or sets `COMETCAST_CODEC_AUTO=1` to let the apps download without asking. **Still true:** the `cometcast-cli` release variant has no consent screen: `send`/`receive` find only a raw (decompressed) library in `COMETCAST_CODEC_DIR`, so a folder with only Cisco's `.bz2` file needs the app or a one-off `cometcast-cli codec install --yes` (which imports or downloads it); and Android needs a decision about Google Play policy (see `docs/capture.md`). The server and the web app do not use OpenH264. **Uncertain:** the legal questions in `docs/capture.md` are open (the binary is downloaded from Cisco by the user's device, it is not redistributed by this project).

**When `openh264` or `openh264-sys2` changes in Cargo.lock,** copy the file names and SHA-256 values from `src/blobs/hashes.txt` of the new `openh264-sys2` (in `~/.cargo/registry/src/*/`) into `crates/cc-codec/src/table.rs`, update the version in the docs, and run `cargo test -p cc-codec --features download -- --ignored` (downloads from Cisco). The test `table_matches_openh264_sys2_hashes_txt` compares the table with that file (it finds the crate with `cargo metadata`) and fails if you forget. The download and hash results in `docs/capture.md` were obtained once, on one machine (Linux x86_64); repeat them for a release.

## Not pinned or not covered

- Actions are pinned to full commit SHAs (resolved 2026-11-07 with `git ls-remote`, the comment names the tag). Bump them by hand or with Dependabot.
- Not pinned: Homebrew `xcodegen` (tvOS job), the Xcode, NDK and Android SDK versions the runner image ships, the `macos-15`/`windows-latest`/`ubuntu-latest` images (the Linux desktop and Linux server jobs use `ubuntu-22.04` on purpose). Rust (1.97.0), Flutter (3.47.9), cargo-ndk (4.1.2), cargo-cyclonedx (0.5.10) and `@cyclonedx/cyclonedx-npm` (6.0.1) are pinned.
- No build caches are used in the release workflow (the web job's `cache: npm` was removed), so a release never reuses a cache that a pull request could have written. Builds are slower for it. The ordinary CI workflows (`web.yml` etc.) may still cache; that does not feed the release.

## Verification state (all UNCERTAIN until the first real run)

| Job | State |
| --- | --- |
| prepare (version checks, script self-tests) | UNCERTAIN: script tests run locally, the git/gh checks not run |
| validate / prepare (guards) | UNCERTAIN: `tools/release-guard.sh` and `tools/release-summary.mjs` self-tests run locally with stub `git`/`gh`/`docker`; the real calls never ran |
| linux | UNCERTAIN: packaging commands run locally with a fake bundle; the Flutter release build on ubuntu-22.04 was never run (README: `flutter build linux --release` is not verified); the `.deb` Depends were never checked on an install |
| windows | UNCERTAIN: nothing run (the VC++ DLL lookup via vswhere included) |
| macos | UNCERTAIN: nothing run (signing order and entitlements, lipo, loading the dylib from `Contents/Frameworks`, sandbox, notarisation). The loader search order is unit-tested only with injected paths |
| android | UNCERTAIN: nothing run; the release Gradle path and the re-signing steps are untested |
| ios | UNCERTAIN: nothing run; no Rust core in the iOS project (outputs are marked `NO-CORE`) |
| tvos | UNCERTAIN: nothing run; project spec and SwiftUI app never built (written without Xcode; see [appletv.md](appletv.md) and [appletv-device-test-plan.md](appletv-device-test-plan.md)); the signing path never ran; the WebRTC binary's third-party notices are not collected, so no tvOS release should be published yet |
| promote-latest | UNCERTAIN: nothing run |
| web | UNCERTAIN: nothing run in this form (the same commands run in `web.yml`, also unverified on GitHub) |
| server | UNCERTAIN: nothing run in this form |
| docker / docker-manifest | UNCERTAIN: the Dockerfile ran on amd64 only (see its header); arm64 and the push-by-digest join never ran |
| meta | cargo-cyclonedx was run locally and produced one `.cdx.json` per crate; the npm SBOM, the deploy bundle and the job itself never ran |
| release | UNCERTAIN: `gh release create --draft --generate-notes --notes-file` (the notes file is expected to be placed before the generated list) never ran |

The workflow files pass `actionlint` 1.7.7 with `shellcheck` enabled. Local checks: `node --test tools/*.test.mjs`, `bash tools/release-guard.test.sh`, and in `apps/flutter`: `flutter test test/rust_loader_test.dart`.
