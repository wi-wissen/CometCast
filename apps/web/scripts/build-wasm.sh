#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the PDF renderer (crates/cc-docs-wasm: cc-docs + hayro) for the browser and writes the
# generated files to apps/web/public/pdf/ (git-ignored). Needs: the rust target
# wasm32-unknown-unknown and a wasm-bindgen CLI of exactly the version in Cargo.lock
# (`cargo install wasm-bindgen-cli --version <that version> --locked`).
# Without these files the web app still works; a received PDF then only gives the notice
# "cannot show documents".
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
out="$here/../public/pdf"
target="${CARGO_TARGET_DIR:-$repo/target}"

# The linear memory of the module is capped (512 MiB): an allocation over it fails inside the
# module instead of growing the tab. UNCERTAIN: the value is a guess, not measured.
export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=--max-memory=536870912"
(cd "$repo" && cargo build -p cc-docs-wasm --release --target wasm32-unknown-unknown)
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" --out-name cc_docs_wasm \
  "$target/wasm32-unknown-unknown/release/cc_docs_wasm.wasm"
ls -l "$out"
