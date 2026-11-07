#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Adds the licence files every release artefact must carry to a staging folder:
#   LICENSE, THIRD_PARTY_NOTICES.md, THIRD_PARTY_LICENSES.md and VERSION (the release version, one line).
# Usage: tools/release-stage.sh STAGING_DIR VERSION
# Used by .github/workflows/release.yml. Run from anywhere; paths are relative to the repository.
set -euo pipefail
if [ "$#" -ne 2 ]; then
  echo "usage: $0 STAGING_DIR VERSION" >&2
  exit 2
fi
dir="$1"
version="$2"
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$dir"
cp "$repo/LICENSE" "$repo/THIRD_PARTY_NOTICES.md" "$repo/THIRD_PARTY_LICENSES.md" "$dir/"
printf '%s\n' "$version" > "$dir/VERSION"
