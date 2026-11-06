#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Runs the headless screen end-to-end test (integration_test/screen_e2e_test.dart):
#   - a real cometcast-server,
#   - an Xvfb display (:99 by default) whose screen is shared by the tests,
#   - the Flutter app (debug build) on a second Xvfb display chosen by xvfb-run.
# Needs: cargo build -p cometcast-server -p cometcast-cli (target/debug), xvfb, xterm, and the
# Flutter setup from apps/flutter/README.md. Run it from apps/flutter.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
app="$here/.."
root="$(cd "$app/../.." && pwd)"
target="${CARGO_TARGET_DIR:-$root/target}"
port="${SERVER_PORT:-$((20000 + RANDOM % 20000))}"

displayfile="$(mktemp)"
cleanup() {
  rm -f "$displayfile"
  [[ -n "${server_pid:-}" ]] && kill "$server_pid" 2>/dev/null || true
  [[ -n "${xvfb_pid:-}" ]] && kill "$xvfb_pid" 2>/dev/null || true
}
trap cleanup EXIT

# The screen that gets shared: 640x480, 24 bit. Xvfb picks a free display number itself
# (-displayfd) and writes it to the file when it is ready, so nothing else is disturbed.
Xvfb -displayfd 5 -screen 0 640x480x24 -nolisten tcp 5>"$displayfile" >/dev/null 2>&1 &
xvfb_pid=$!
for _ in $(seq 1 50); do
  [[ -s "$displayfile" ]] && break
  sleep 0.1
done
shared_display=":$(head -n1 "$displayfile")"
echo "shared screen: $shared_display"

"$target/debug/cometcast-server" --listen "127.0.0.1:$port" >/dev/null 2>&1 &
server_pid=$!
sleep 2

cd "$app"
export COMETCAST_CARGO_TARGET_DIR="$target"
xvfb-run -a flutter test "${TEST_FILE:-integration_test/screen_e2e_test.dart}" -d linux \
  --dart-define=COMETCAST_TEST_SERVER="127.0.0.1:$port" \
  --dart-define=COMETCAST_CLI="$target/debug/cometcast-cli" \
  --dart-define=COMETCAST_CAPTURE_DISPLAY="$shared_display"
