#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Duplicate guards of .github/workflows/release.yml and promote-latest.yml.
#   tools/release-guard.sh tag-free REPO TAG      fail when the tag or any release (draft too) for TAG exists
#   tools/release-guard.sh image-free IMAGE VER   fail when ghcr.io/... IMAGE:VER already exists
#   tools/release-guard.sh skip-valid LIST        fail when LIST has a job name that is not known
# Exit codes: 0 = free/valid, 1 = conflict or invalid, 2 = could not check (treated as a failure by
# the workflow: a guard that cannot look must not wave the run through).
# `tag-free` needs GH_TOKEN (a token that may list releases: drafts are only visible with
# contents: write). `image-free` needs `docker login ghcr.io` done before.
# Self-test: tools/release-guard.test.sh
set -euo pipefail

KNOWN_JOBS="linux windows macos android ios tvos web server docker meta"

die() { echo "::error::$*" >&2; }

cmd="${1:-}"
case "$cmd" in
  tag-free)
    repo="${2:?repo}"; tag="${3:?tag}"
    : "${GH_TOKEN:?GH_TOKEN is required}"
    # Authenticated, so private repositories work. The token goes in a header, not in the URL
    # (a URL would show up in process lists and in git's own error messages).
    auth="$(printf 'x-access-token:%s' "$GH_TOKEN" | base64 | tr -d '\n')"
    echo "::add-mask::$auth"
    rc=0
    git -c "http.extraheader=AUTHORIZATION: basic $auth" ls-remote --exit-code --tags \
      "https://github.com/$repo.git" "refs/tags/$tag" > /dev/null 2>&1 || rc=$?
    case "$rc" in
      0) die "tag $tag already exists"; exit 1 ;;
      2) ;; # no such ref: this is the good case
      *) die "could not query the tags of $repo (git exit $rc)"; exit 2 ;;
    esac
    if ! json="$(gh release list --repo "$repo" --limit 1000 --json tagName,isDraft)"; then
      die "could not list the releases of $repo"; exit 2
    fi
    hit="$(printf '%s' "$json" | jq -r --arg tag "$tag" '[.[] | select(.tagName == $tag)] | map(if .isDraft then "draft" else "published" end) | join(",")')"
    if [ -n "$hit" ]; then
      die "a release for $tag already exists ($hit); delete it first"; exit 1
    fi
    ;;
  image-free)
    image="${2:?image}"; ver="${3:?version}"
    rc=0
    out="$(docker buildx imagetools inspect "$image:$ver" 2>&1)" || rc=$?
    if [ "$rc" -eq 0 ]; then
      die "$image:$ver is already pushed; versions are never overwritten. Use a new version."; exit 1
    fi
    if printf '%s' "$out" | grep -qiE 'not found|manifest unknown|name unknown'; then
      exit 0
    fi
    die "could not check whether $image:$ver exists: $out"; exit 2
    ;;
  skip-valid)
    list="${2-}"
    [ -z "$list" ] && exit 0
    case "$list" in
      ,* | *, | *,,*) die "skip has an empty entry ('$list'); comma separated, no spaces"; exit 1 ;;
    esac
    IFS=',' read -r -a items <<< "$list"
    for item in "${items[@]}"; do
      case " $KNOWN_JOBS " in
        *" $item "*) ;;
        *) die "unknown job '$item' in skip (known: ${KNOWN_JOBS// /,}; comma separated, no spaces)"; exit 1 ;;
      esac
    done
    ;;
  *)
    echo "usage: $0 tag-free REPO TAG | image-free IMAGE VERSION | skip-valid LIST" >&2
    exit 64
    ;;
esac
