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

if [ "$fails" -eq 0 ]; then echo "all passed"; else echo "$fails failed"; exit 1; fi
