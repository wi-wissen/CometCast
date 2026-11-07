#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Runs the headless content end-to-end test (integration_test/content_e2e_test.dart):
#   - a real cometcast-server,
#   - the real cometcast-cli (sends a PDF, an image and a link to the app; receives from the app),
#   - the Flutter app (debug build) under xvfb-run, with the PDF render worker bundled next to it.
# Needs: `cargo build --workspace --bins` (target/debug: cometcast-server, cometcast-cli,
# cc-docs-worker), xvfb, and the Flutter setup from apps/flutter/README.md. Run it from
# apps/flutter. COMETCAST_DOCS_WORKER must not be set: the test checks that the app finds the
# worker that `flutter build linux` bundled.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
app="$here/.."
root="$(cd "$app/../.." && pwd)"
target="${CARGO_TARGET_DIR:-$root/target}"
port="${SERVER_PORT:-$((20000 + RANDOM % 20000))}"

cleanup() {
  [[ -n "${server_pid:-}" ]] && kill "$server_pid" 2>/dev/null || true
}
trap cleanup EXIT

unset COMETCAST_DOCS_WORKER

"$target/debug/cometcast-server" --listen "127.0.0.1:$port" >/dev/null 2>&1 &
server_pid=$!
sleep 2

cd "$app"
export COMETCAST_CARGO_TARGET_DIR="$target"
xvfb-run -a flutter test "${TEST_FILE:-integration_test/content_e2e_test.dart}" -d linux \
  --dart-define=COMETCAST_TEST_SERVER="127.0.0.1:$port" \
  --dart-define=COMETCAST_CLI="$target/debug/cometcast-cli" \
  --dart-define=COMETCAST_TEST_PDF="$root/crates/cc-docs/tests/data/classroom.pdf"
