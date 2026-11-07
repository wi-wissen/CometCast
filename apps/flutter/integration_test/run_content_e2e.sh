#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Runs the headless content end-to-end test (integration_test/content_e2e_test.dart):
#   - a real cometcast-server,
#   - the real cometcast-cli (sends a PDF, an image and a link to the app; receives from the app),
#   - the Flutter app (debug build) under xvfb-run, with the PDF render worker bundled next to it.
# Needs: `cargo build --workspace --bins` (target/debug: cometcast-server, cometcast-cli,
# cc-docs-worker, cc-office-worker), xvfb, and the Flutter setup from apps/flutter/README.md. Run it from
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

# Wait until the server's port answers (at most 20 s); stop at once if the server died.
ready=0
for _ in $(seq 1 100); do
  if ! kill -0 "$server_pid" 2>/dev/null; then
    echo "cometcast-server exited before it listened on port $port" >&2
    exit 1
  fi
  if (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; then
    ready=1
    break
  fi
  sleep 0.2
done
if [[ "$ready" != 1 ]]; then
  echo "cometcast-server did not answer on port $port within 20 s" >&2
  exit 1
fi

# A tiny .docx for the Office conversion test (the same parts as the fixture of the cc-office
# tests: content types, package relationship, one paragraph).
docx_dir="$(mktemp -d)"
trap 'cleanup; rm -rf "$docx_dir"' EXIT
python3 -I - "$docx_dir/classnotes.docx" <<'PY'
import sys, zipfile
ct = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>'
rels = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>'
doc = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello Comet Cast</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1417" w:right="1417" w:bottom="1417" w:left="1417" w:header="708" w:footer="708" w:gutter="0"/></w:sectPr></w:body></w:document>'
with zipfile.ZipFile(sys.argv[1], "w", zipfile.ZIP_DEFLATED) as z:
    z.writestr("[Content_Types].xml", ct)
    z.writestr("_rels/.rels", rels)
    z.writestr("word/document.xml", doc)
PY

cd "$app"
export COMETCAST_CARGO_TARGET_DIR="$target"
xvfb-run -a flutter test "${TEST_FILE:-integration_test/content_e2e_test.dart}" -d linux \
  --dart-define=COMETCAST_TEST_SERVER="127.0.0.1:$port" \
  --dart-define=COMETCAST_CLI="$target/debug/cometcast-cli" \
  --dart-define=COMETCAST_TEST_PDF="$root/crates/cc-docs/tests/data/slides.pdf" \
  --dart-define=COMETCAST_TEST_DOCX="$docx_dir/classnotes.docx"
