#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Sets ONE release version everywhere it lives, in the working tree (nothing is committed).
// Used by .github/workflows/release.yml inside the CI runner; it can also be run locally,
// ideally in a scratch copy: `git stash` or `git checkout -- .` undoes it.
//
//   node tools/set-version.mjs 1.2.3 [--build-number 42] [--image ghcr.io/owner/cometcast-server]
//                                    [--h264 keep|libloading|source] [--root DIR]
//
// What it changes (and why):
//   Cargo.toml              [workspace.package] version (every crate uses version.workspace)
//   Cargo.lock              version of each workspace crate (so `cargo build --locked` still works)
//   apps/flutter/pubspec.yaml   `version: MAJOR.MINOR.PATCH+BUILD`. Flutter derives from it:
//                           Android versionName/versionCode, iOS/macOS CFBundleShortVersionString /
//                           CFBundleVersion (via FLUTTER_BUILD_NAME / FLUTTER_BUILD_NUMBER) and the
//                           Windows version resource. A pre-release suffix is NOT put here: Apple and
//                           Windows need purely numeric versions.
//   apps/web/package.json, package-lock.json   version
//   apps/apple-tv/project.yml   MARKETING_VERSION and CURRENT_PROJECT_VERSION (XcodeGen settings)
//   deploy/docker-compose.yml   image tag of cometcast-server
//   crates/cc-ffi/Cargo.toml    only with --h264 libloading|source: the `default` feature, so the
//                           Flutter builds (CMake and Gradle run a plain `cargo build`) use that
//                           OpenH264 mode. Release builds must use libloading (docs/capture.md).
//
// It then CHECKS that the files that must follow Flutter's version really do (Android Gradle,
// iOS/macOS Info.plist, Windows Runner.rc) and exits 1 when a file does not contain what it
// should. Plain Node 22, no dependencies.
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// Semantic Versioning 2.0.0 (semver.org), without build metadata (`+...`): the tag, the
// Docker tag and the Flutter build number must not be ambiguous.
export const SEMVER =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*))?$/;

class VersionError extends Error {}

function fail(message) {
  throw new VersionError(message);
}

function parseArgs(argv) {
  const opts = { build: 1, image: null, h264: 'keep', root: null, version: null };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const next = () => {
      if (i + 1 >= argv.length) fail(`${a} needs a value`);
      return argv[++i];
    };
    if (a === '--build-number') opts.build = Number(next());
    else if (a === '--image') opts.image = next();
    else if (a === '--h264') opts.h264 = next();
    else if (a === '--root') opts.root = next();
    else if (a.startsWith('-')) fail(`unknown option ${a}`);
    else if (opts.version === null) opts.version = a;
    else fail(`unexpected argument ${a}`);
  }
  if (opts.version === null) fail('usage: set-version.mjs VERSION [--build-number N] [--image NAME] [--h264 keep|libloading|source] [--root DIR]');
  if (!SEMVER.test(opts.version)) fail(`"${opts.version}" is not a semantic version (MAJOR.MINOR.PATCH[-prerelease], no "+build")`);
  if (!Number.isInteger(opts.build) || opts.build < 1 || opts.build > 2000000000) fail('--build-number must be an integer from 1 to 2000000000');
  if (!['keep', 'libloading', 'source'].includes(opts.h264)) fail('--h264 must be keep, libloading or source');
  if (opts.image !== null && !/^[a-z0-9][a-z0-9._/-]*$/.test(opts.image)) fail(`--image "${opts.image}" is not a lower-case image name`);
  return opts;
}

// Replaces exactly one match of `re` in the file; fails when there is none or several.
function editFile(root, rel, re, replacement, label) {
  const path = join(root, rel);
  if (!existsSync(path)) fail(`${rel}: file not found`);
  const text = readFileSync(path, 'utf8');
  const matches = text.match(new RegExp(re.source, re.flags.includes('g') ? re.flags : re.flags + 'g'));
  if (!matches || matches.length !== 1) fail(`${rel}: expected exactly one ${label}, found ${matches ? matches.length : 0}`);
  const out = text.replace(re, replacement);
  if (out !== text) writeFileSync(path, out);
  return out;
}

function setCargoToml(root, version) {
  // Only inside [workspace.package]: the first `version = "..."` line after that header and
  // before the next header.
  const rel = 'Cargo.toml';
  const path = join(root, rel);
  const text = readFileSync(path, 'utf8');
  const m = /(\[workspace\.package\][^[]*?\nversion\s*=\s*")([^"]*)(")/.exec(text);
  if (!m) fail(`${rel}: no version in [workspace.package]`);
  const out = text.replace(m[0], `${m[1]}${version}${m[3]}`);
  if (out !== text) writeFileSync(path, out);
  return m[2];
}

function setCargoLock(root, oldVersion, version) {
  const rel = 'Cargo.lock';
  const path = join(root, rel);
  const text = readFileSync(path, 'utf8');
  const parts = text.split('\n[[package]]\n');
  let changed = 0;
  for (let i = 1; i < parts.length; i++) {
    const block = parts[i];
    // Workspace crates (path dependencies) have no `source` line; registry crates do.
    if (/^source = /m.test(block)) continue;
    const m = /^version = "([^"]*)"$/m.exec(block);
    if (!m) continue;
    if (m[1] !== oldVersion) fail(`${rel}: workspace crate with version ${m[1]}, expected ${oldVersion}`);
    parts[i] = block.replace(m[0], `version = "${version}"`);
    changed++;
  }
  if (changed === 0) fail(`${rel}: no workspace crate found`);
  const out = parts.join('\n[[package]]\n');
  if (out !== text) writeFileSync(path, out);
  return changed;
}

function setPackageJson(root, version) {
  editFile(root, 'apps/web/package.json', /^(\s*"version":\s*")[^"]*(",?)$/m, `$1${version}$2`, '"version" line');
  const rel = 'apps/web/package-lock.json';
  const path = join(root, rel);
  const raw = readFileSync(path, 'utf8');
  const lock = JSON.parse(raw);
  lock.version = version;
  if (lock.packages && lock.packages['']) lock.packages[''].version = version;
  // Rewriting through JSON.stringify must not reformat the file, or the diff would be noise.
  if (JSON.stringify(JSON.parse(raw), null, 2) + '\n' !== raw) fail(`${rel}: not formatted like JSON.stringify(.., null, 2); refusing to rewrite it`);
  const out = JSON.stringify(lock, null, 2) + '\n';
  if (out !== raw) writeFileSync(path, out);
}

function setPubspec(root, version, build) {
  const core = version.split('-')[0];
  editFile(root, 'apps/flutter/pubspec.yaml', /^version:\s*\S+\s*$/m, `version: ${core}+${build}`, 'top-level "version:" line');
  return `${core}+${build}`;
}

function setAppleTv(root, version, build) {
  const rel = 'apps/apple-tv/project.yml';
  const path = join(root, rel);
  if (!existsSync(path)) return false;
  let text = readFileSync(path, 'utf8');
  const core = version.split('-')[0];
  const set = (key, value) => {
    const re = new RegExp(`^(\\s*)${key}:.*$`, 'm');
    if (re.test(text)) {
      text = text.replace(re, `$1${key}: "${value}"`);
    } else {
      const anchor = /^(\s*)GENERATE_INFOPLIST_FILE:.*$/m;
      if (!anchor.test(text)) fail(`${rel}: no GENERATE_INFOPLIST_FILE line to insert ${key} after`);
      text = text.replace(anchor, `$&\n$1${key}: "${value}"`);
    }
  };
  // tvOS needs a numeric CFBundleShortVersionString, so the pre-release suffix is left out.
  set('MARKETING_VERSION', core);
  set('CURRENT_PROJECT_VERSION', String(build));
  writeFileSync(path, text);
  return true;
}

function setCompose(root, version, image) {
  editFile(root, 'deploy/docker-compose.yml', /^(\s*image:\s*)\S*cometcast-server\S*\s*$/m, `$1${image}:${version}`, 'cometcast-server image line');
}

function setH264(root, mode) {
  if (mode === 'keep') return;
  editFile(root, 'crates/cc-ffi/Cargo.toml', /^default = \["h264-[a-z]+"\]$/m, `default = ["h264-${mode}"]`, '`default = ["h264-..."]` feature line');
}

// Files that get their version from Flutter must say so; otherwise the release would ship the
// template default (1.0 / 1.0.0) without anyone noticing.
function checkFollowers(root) {
  const problems = [];
  const need = (rel, re, what) => {
    const path = join(root, rel);
    if (!existsSync(path)) return; // a platform folder may be absent
    if (!re.test(readFileSync(path, 'utf8'))) problems.push(`${rel}: ${what}`);
  };
  need('apps/flutter/android/app/build.gradle.kts', /versionName\s*=\s*flutter\.versionName/, 'versionName must be flutter.versionName');
  need('apps/flutter/android/app/build.gradle.kts', /versionCode\s*=\s*flutter\.versionCode/, 'versionCode must be flutter.versionCode');
  need('apps/flutter/ios/Runner/Info.plist', /<key>CFBundleShortVersionString<\/key>\s*<string>\$\(FLUTTER_BUILD_NAME\)<\/string>/, 'CFBundleShortVersionString must be $(FLUTTER_BUILD_NAME)');
  need('apps/flutter/ios/Runner/Info.plist', /<key>CFBundleVersion<\/key>\s*<string>\$\(FLUTTER_BUILD_NUMBER\)<\/string>/, 'CFBundleVersion must be $(FLUTTER_BUILD_NUMBER)');
  need('apps/flutter/macos/Runner/Info.plist', /<key>CFBundleShortVersionString<\/key>\s*<string>\$\(FLUTTER_BUILD_NAME\)<\/string>/, 'CFBundleShortVersionString must be $(FLUTTER_BUILD_NAME)');
  need('apps/flutter/macos/Runner/Info.plist', /<key>CFBundleVersion<\/key>\s*<string>\$\(FLUTTER_BUILD_NUMBER\)<\/string>/, 'CFBundleVersion must be $(FLUTTER_BUILD_NUMBER)');
  need('apps/flutter/windows/runner/Runner.rc', /FLUTTER_VERSION_MAJOR/, 'version resource must use FLUTTER_VERSION_*');
  if (problems.length) fail('version followers out of date:\n  ' + problems.join('\n  '));
}

export function setVersion(opts) {
  const root = resolve(opts.root ?? join(fileURLToPath(import.meta.url), '..', '..'));
  const image = opts.image ?? defaultImage(root);
  const oldVersion = setCargoToml(root, opts.version);
  const crates = setCargoLock(root, oldVersion, opts.version);
  setPackageJson(root, opts.version);
  const flutter = setPubspec(root, opts.version, opts.build);
  const tv = setAppleTv(root, opts.version, opts.build);
  setCompose(root, opts.version, image);
  setH264(root, opts.h264);
  checkFollowers(root);
  return { oldVersion, crates, flutter, tv, image };
}

// ghcr.io/<owner>/cometcast-server from `repository = "https://github.com/OWNER/REPO"`.
function defaultImage(root) {
  const text = readFileSync(join(root, 'Cargo.toml'), 'utf8');
  const m = /^repository\s*=\s*"https:\/\/github\.com\/([^/"]+)\/[^/"]+"/m.exec(text);
  if (!m) fail('Cargo.toml: no GitHub repository URL; pass --image');
  return `ghcr.io/${m[1].toLowerCase()}/cometcast-server`;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const opts = parseArgs(process.argv.slice(2));
    const r = setVersion(opts);
    console.log(`version ${r.oldVersion} -> ${opts.version}`);
    console.log(`  Cargo.toml + Cargo.lock (${r.crates} workspace crates), apps/web package.json + lock`);
    console.log(`  apps/flutter/pubspec.yaml: ${r.flutter}`);
    console.log(`  apps/apple-tv/project.yml: ${r.tv ? 'updated' : 'not present, skipped'}`);
    console.log(`  deploy/docker-compose.yml: ${r.image}:${opts.version}`);
    console.log(`  cc-ffi default H.264 mode: ${opts.h264}`);
  } catch (e) {
    if (e instanceof VersionError) {
      console.error(`set-version: ${e.message}`);
      process.exit(1);
    }
    throw e;
  }
}
