#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Runs the cc-capture PipeWire loopback test against a throw-away PipeWire + WirePlumber, so it
# works on a machine (or CI runner) without a desktop session:
#
#   tools/pipewire-loopback-test.sh
#
# What it checks: the PipeWire half of the Wayland backend (format negotiation, buffer types,
# BGRx -> BGRA, a changing picture, the end when the node disappears) against a real daemon,
# with a fake screen producer in place of the compositor. It does NOT test the desktop portal.
# Needs: pipewire, wireplumber, dbus-run-session, libpipewire-0.3-dev, libclang-dev, cargo.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
for t in pipewire wireplumber dbus-run-session; do
  command -v "$t" >/dev/null || { echo "missing: $t" >&2; exit 2; }
done

dir="$(mktemp -d)"
chmod 700 "$dir"
export XDG_RUNTIME_DIR="$dir"
pids=()
cleanup() {
  for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
  rm -rf "$dir"
}
trap cleanup EXIT

pipewire >"$dir/pipewire.log" 2>&1 &
pids+=($!)
for _ in $(seq 1 50); do
  [ -S "$dir/pipewire-0" ] && break
  sleep 0.1
done
[ -S "$dir/pipewire-0" ] || { echo "PipeWire did not start" >&2; cat "$dir/pipewire.log" >&2; exit 1; }
dbus-run-session -- wireplumber >"$dir/wireplumber.log" 2>&1 &
pids+=($!)
sleep 3

# --nocapture so a skip ("no PipeWire socket") cannot hide: the socket exists here.
cargo test -p cc-capture --features wayland frames_flow -- --nocapture --test-threads=1
