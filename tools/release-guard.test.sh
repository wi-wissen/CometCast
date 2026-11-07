#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Self-test of tools/release-guard.sh with stub git/gh/docker commands. No network.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
guard="$here/release-guard.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/bin"
export PATH="$tmp/bin:$PATH"
export GH_TOKEN=dummy
fails=0

stub() { printf '#!/usr/bin/env bash\n%s\n' "$2" > "$tmp/bin/$1"; chmod +x "$tmp/bin/$1"; }
expect() { # name expected-exit command...
  local name="$1" want="$2"; shift 2
  "$@" > "$tmp/out" 2>&1; local got=$?
  if [ "$got" -ne "$want" ]; then echo "FAIL $name: exit $got, wanted $want"; cat "$tmp/out"; fails=$((fails+1)); else echo "ok   $name"; fi
}

# tag-free
stub git 'exit 2'
stub gh 'echo "[{\"tagName\":\"v0.9.0\",\"isDraft\":false}]"'
expect "tag free" 0 "$guard" tag-free o/r v1.0.0
stub git 'exit 0'
expect "tag exists" 1 "$guard" tag-free o/r v1.0.0
stub git 'exit 128'
expect "git error" 2 "$guard" tag-free o/r v1.0.0
stub git 'exit 2'
stub gh 'echo "[{\"tagName\":\"v1.0.0\",\"isDraft\":true}]"'
expect "draft exists" 1 "$guard" tag-free o/r v1.0.0
stub gh 'echo "[{\"tagName\":\"v1.0.0\",\"isDraft\":false}]"'
expect "published exists" 1 "$guard" tag-free o/r v1.0.0
stub gh 'exit 1'
expect "gh error" 2 "$guard" tag-free o/r v1.0.0

# image-free
stub docker 'exit 0'
expect "image exists" 1 "$guard" image-free ghcr.io/o/i 1.0.0
stub docker 'echo "ERROR: ghcr.io/o/i:1.0.0: not found"; exit 1'
expect "image free" 0 "$guard" image-free ghcr.io/o/i 1.0.0
stub docker 'echo "ERROR: denied"; exit 1'
expect "image check error" 2 "$guard" image-free ghcr.io/o/i 1.0.0

# skip-valid
expect "skip empty" 0 "$guard" skip-valid ""
expect "skip ok" 0 "$guard" skip-valid "linux,docker,meta"
expect "skip typo" 1 "$guard" skip-valid "linux,dokcer"
expect "skip space" 1 "$guard" skip-valid "linux, docker"
expect "skip trailing comma" 1 "$guard" skip-valid "linux,"

# licenses-fresh
lic="$tmp/lic"
mkdir -p "$lic/apps/web" "$lic/apps/flutter"
printf 'a\n' > "$lic/Cargo.lock"; printf 'b\n' > "$lic/apps/web/package-lock.json"; printf 'c\n' > "$lic/apps/flutter/pubspec.lock"
expect "licenses missing" 1 "$guard" licenses-fresh "$lic"
# shellcheck disable=SC2016 # the backticks are literal text of a Markdown fence
mk_lic() { printf '# x\n\n```text\nCargo.lock-sha256: %s\npackage-lock.json-sha256: %s\npubspec.lock-sha256: %s\n```\n' \
  "$(sha256sum "$lic/Cargo.lock" | cut -d' ' -f1)" "$(sha256sum "$lic/apps/web/package-lock.json" | cut -d' ' -f1)" "$(sha256sum "$lic/apps/flutter/pubspec.lock" | cut -d' ' -f1)" > "$lic/THIRD_PARTY_LICENSES.md"; }
mk_lic
expect "licenses fresh" 0 "$guard" licenses-fresh "$lic"
printf 'changed\n' >> "$lic/Cargo.lock"
expect "licenses older than Cargo.lock" 1 "$guard" licenses-fresh "$lic"
mk_lic; printf 'changed\n' >> "$lic/apps/web/package-lock.json"
expect "licenses older than package-lock" 1 "$guard" licenses-fresh "$lic"
mk_lic; printf 'changed\n' >> "$lic/apps/flutter/pubspec.lock"
expect "licenses older than pubspec.lock" 1 "$guard" licenses-fresh "$lic"
printf '# no hashes\n' > "$lic/THIRD_PARTY_LICENSES.md"
expect "licenses without hash lines" 1 "$guard" licenses-fresh "$lic"

# the copies for the apps (native-apps.md 21.7), checked when the root notices file exists
mk_lic
printf 'notices\n' > "$lic/THIRD_PARTY_NOTICES.md"
expect "licence copies missing" 1 "$guard" licenses-fresh "$lic"
mkdir -p "$lic/apps/flutter/assets/licenses" "$lic/apps/apple-tv/Resources/Licenses" "$lic/apps/web/public"
mk_copies() {
  for d in apps/flutter/assets/licenses apps/apple-tv/Resources/Licenses; do
    cp "$lic/THIRD_PARTY_LICENSES.md" "$lic/THIRD_PARTY_NOTICES.md" "$lic/$d/"
  done
  { cat "$lic/THIRD_PARTY_NOTICES.md"; printf '\n'; cat "$lic/THIRD_PARTY_LICENSES.md"; } > "$lic/apps/web/public/third-party-licenses.txt"
}
mk_copies
expect "licence copies fresh" 0 "$guard" licenses-fresh "$lic"
printf 'x\n' >> "$lic/apps/flutter/assets/licenses/THIRD_PARTY_NOTICES.md"
expect "flutter copy differs" 1 "$guard" licenses-fresh "$lic"
mk_copies; printf 'x\n' >> "$lic/apps/apple-tv/Resources/Licenses/THIRD_PARTY_LICENSES.md"
expect "apple tv copy differs" 1 "$guard" licenses-fresh "$lic"
mk_copies; printf 'x\n' >> "$lic/apps/web/public/third-party-licenses.txt"
expect "web copy differs" 1 "$guard" licenses-fresh "$lic"

if [ "$fails" -eq 0 ]; then echo "all passed"; else echo "$fails failed"; exit 1; fi
