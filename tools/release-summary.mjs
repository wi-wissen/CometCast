#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Writes the hand-written part of the release notes (build status table, signing state, checks
// to do) for .github/workflows/release.yml. The GitHub-generated change list is added by
// `gh release create --generate-notes`.
//
//   NEEDS_JSON='{"linux":{"result":"success"},...}' node tools/release-summary.mjs \
//     --version 1.2.3 --assets DIR --out notes.md [--dry-run] [--image NAME]
//
// Exit code 1 when DIR contains no platform artefact at all (only licence/SBOM/deploy files or
// nothing). The notes file is still written first. Failed jobs do not change the exit code
// (they are listed in the notes); the workflow marks the run red in a later step.
import { readdirSync, writeFileSync } from 'node:fs';

const args = process.argv.slice(2);
const opt = (name) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};
const version = opt('--version');
const assetsDir = opt('--assets');
const out = opt('--out');
const image = opt('--image');
const dry = args.includes('--dry-run');
if (!version || !assetsDir || !out) {
  console.error('usage: release-summary.mjs --version V --assets DIR --out FILE [--dry-run] [--image NAME]');
  process.exit(2);
}

const needs = JSON.parse(process.env.NEEDS_JSON || '{}');
const files = readdirSync(assetsDir).filter((f) => f !== 'SHA256SUMS').sort();

// Job id -> what it builds. Every job stays UNCERTAIN until its first real run (docs/releasing.md).
const jobs = [
  ['linux', 'Linux desktop (tar.gz, .deb), x86_64'],
  ['windows', 'Windows desktop (zip), x86_64'],
  ['macos', 'macOS app (zip, dmg), universal'],
  ['android', 'Android (APK, AAB)'],
  ['ios', 'iOS (xcarchive, optional IPA; NO Rust core, not shippable)'],
  ['tvos', 'Apple TV (xcarchive, optional signed IPA)'],
  ['web', 'Web app (zip)'],
  ['server', 'Server binaries (linux x86_64/aarch64, windows, macOS)'],
  ['docker', 'Docker image build (amd64, arm64)'],
  ['docker-manifest', 'Docker multi-arch tag on ghcr.io'],
  ['meta', 'SBOMs, deploy bundle, licence files'],
];
const rows = jobs.map(([id, what]) => {
  const result = needs[id]?.result ?? 'not run';
  return `| ${id} | ${what} | ${result} | UNCERTAIN until first real run |`;
});

// Files that are not release-signed or not shippable. UNVERIFIED: the iOS build was never run on
// a device (docs/ios-device-test-plan.md). NO-CORE: the old name of the iOS files (no Rust core).
const unsigned = files.filter((f) => /debugkey|UNSIGNED|adhoc|not-notarized|NO-CORE|UNVERIFIED/i.test(f));
// Platform artefacts = everything except the files of the `meta` job.
const META = /^(cometcast-sbom-|cometcast-deploy-|LICENSE$|THIRD_PARTY_NOTICES\.md$)/;
const platformFiles = files.filter((f) => !META.test(f));
const bad = jobs.filter(([id]) => ['failure', 'cancelled'].includes(needs[id]?.result)).map(([id]) => id);

const lines = [];
lines.push(`## Comet Cast ${version}${dry ? ' (dry run, nothing published)' : ''}`);
lines.push('');
lines.push('**UNCERTAIN: the release workflow itself has not had a verified real run yet. Treat every artefact as unverified until you have started it on the target platform.**');
lines.push('');
lines.push('### Build status');
lines.push('');
lines.push('| Job | Content | Result | Verification |');
lines.push('| --- | --- | --- | --- |');
lines.push(...rows);
lines.push('');
if (bad.length) {
  lines.push(`**Failed or cancelled: ${bad.join(', ')}. Their artefacts are missing from this release.**`);
  lines.push('');
}
lines.push('### Signing');
lines.push('');
if (unsigned.length) {
  lines.push('These files are NOT signed with a release key (signing secrets were not configured) or are NOT shippable (`UNVERIFIED`: the iOS app and its broadcast extension were never run on a device, so even a signed .ipa is only a build check). Do not distribute them as official builds:');
  lines.push('');
  for (const f of unsigned) lines.push(`- \`${f}\``);
} else {
  lines.push('No file name marks an unsigned artefact. This does not prove the Windows, Linux, web, server and Apple archive files are signed: only Android (keystore), macOS (Developer ID, notarisation) and the optional iOS and tvOS IPAs are signed by this workflow, and only when their secrets exist. See docs/releasing.md.');
}
lines.push('');
lines.push('### Notes');
lines.push('');
lines.push('- Verify downloads with `sha256sum -c SHA256SUMS`.');
lines.push('- H.264: the native apps are built with the OpenH264 `h264-libloading` mode and download Cisco\'s `libopenh264` on first use after the user agrees (checked against a pinned SHA-256). Only the Linux x86_64 download and load was tested; Windows, macOS and Android were not. `cometcast-cli` does not download it (set `COMETCAST_CODEC_DIR` or `COMETCAST_OPENH264_LIB`). See docs/capture.md.');
lines.push('- Windows, macOS and iOS Flutter builds bundle a Rust core and workers only as far as this workflow does it (docs/releasing.md); the project README lists these platforms as not verified.');
if (image) lines.push(`- Container image: \`${image}:${version}\` (linux/amd64, linux/arm64). \`latest\` was NOT moved: run the "Promote latest" workflow after publishing this release (full releases only). The image has no PDF viewer.`);
lines.push('');

writeFileSync(out, lines.join('\n'));
console.log(lines.join('\n'));
if (platformFiles.length === 0) {
  console.error('release-summary: no platform artefact to publish (every platform job failed or was skipped)');
  process.exit(1);
}
