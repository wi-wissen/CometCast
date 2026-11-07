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

# The linear memory of the module is capped (512 MiB = 8192 pages of 64 KiB): an allocation over it traps
# inside the module instead of growing the tab (the worker then ends the document). UNCERTAIN: the value
# is a guess, not measured. The flag goes to the final crate only (`cargo rustc ... --`), so RUSTFLAGS,
# CARGO_ENCODED_RUSTFLAGS and build.rustflags of the person who runs this stay exactly as they are.
max_bytes=536870912
(cd "$repo" && cargo rustc -p cc-docs-wasm --lib --crate-type cdylib --release --target wasm32-unknown-unknown \
  -- -C "link-arg=--max-memory=$max_bytes")
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" --out-name cc_docs_wasm \
  "$target/wasm32-unknown-unknown/release/cc_docs_wasm.wasm"
# Prove that the cap is in the file that ships (parses its memory section); fail otherwise.
node "$here/check-wasm-memory.mjs" "$out/cc_docs_wasm_bg.wasm" $((max_bytes / 65536))
ls -l "$out"
