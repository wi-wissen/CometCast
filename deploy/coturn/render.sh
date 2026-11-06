#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Fills deploy/coturn/turnserver.conf.template with values from deploy/.env and
# deploy/secrets/turn_secret and writes deploy/coturn/turnserver.conf (not committed).
# Run from anywhere:  deploy/coturn/render.sh
#
# Safety rules:
#  - .env is NOT run as shell code. It is read line by line as KEY=VALUE.
#  - Every value is checked against a short list of allowed characters and the script
#    stops with an error otherwise. The values are then inserted as plain text (awk,
#    no pattern matching), so characters such as & | \ have no special meaning anyway.
#
# UNCERTAIN: tested only with a dry render, not with a real coturn.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
deploy=$(dirname "$here")

[ -f "$deploy/.env" ] || { echo "missing $deploy/.env (copy .env.example)" >&2; exit 1; }
[ -f "$deploy/secrets/turn_secret" ] || { echo "missing $deploy/secrets/turn_secret" >&2; exit 1; }

# Read one setting from .env: the last line "KEY=VALUE" wins; quotes around VALUE are removed.
env_value() {
  awk -F= -v key="$1" '
    $0 ~ /^[ \t]*#/ { next }
    {
      k = $1; gsub(/^[ \t]+|[ \t]+$/, "", k)
      if (k == key) {
        v = substr($0, index($0, "=") + 1)
        gsub(/^[ \t]+|[ \t\r]+$/, "", v)
        if (v ~ /^".*"$/ || v ~ /^'\''.*'\''$/) v = substr(v, 2, length(v) - 2)
        val = v; found = 1
      }
    }
    END { if (found) print val }' "$deploy/.env"
}

# Stop unless "$2" (the value of setting "$1") matches the grep pattern "$3" completely.
check() {
  if ! printf '%s\n' "$2" | grep -Eq "^($3)\$"; then
    echo "error: $1 has an empty value or characters that are not allowed here" >&2
    exit 1
  fi
}

TURN_REALM=$(env_value TURN_REALM)
TURN_EXTERNAL_IP=$(env_value TURN_EXTERNAL_IP)
TURN_RELAY_IP=$(env_value TURN_RELAY_IP)
TURN_MIN_PORT=$(env_value TURN_MIN_PORT)
TURN_MAX_PORT=$(env_value TURN_MAX_PORT)
TURN_SECRET=$(tr -d '\n\r' < "$deploy/secrets/turn_secret")

check TURN_REALM "$TURN_REALM" '[A-Za-z0-9.-]+'
check TURN_EXTERNAL_IP "$TURN_EXTERNAL_IP" '[0-9A-Fa-f:.]+'
check TURN_RELAY_IP "$TURN_RELAY_IP" '[0-9A-Fa-f:.]+'
check TURN_MIN_PORT "$TURN_MIN_PORT" '[0-9]{1,5}'
check TURN_MAX_PORT "$TURN_MAX_PORT" '[0-9]{1,5}'
# The secret: at least 16 characters, from a safe set (openssl rand -hex 32 fits).
check "secrets/turn_secret" "$TURN_SECRET" '[A-Za-z0-9._~+/=-]{16,}'

out="$here/turnserver.conf"
umask 077   # the file contains the shared secret
# The values travel in the environment of awk and are inserted as literal text.
TURN_REALM="$TURN_REALM" TURN_EXTERNAL_IP="$TURN_EXTERNAL_IP" TURN_RELAY_IP="$TURN_RELAY_IP" \
TURN_MIN_PORT="$TURN_MIN_PORT" TURN_MAX_PORT="$TURN_MAX_PORT" TURN_SECRET="$TURN_SECRET" \
awk '
  function lit(line, name,    token, val, out, i) {
    token = "@" name "@"; val = ENVIRON[name]; out = ""
    while ((i = index(line, token)) > 0) {
      out = out substr(line, 1, i - 1) val
      line = substr(line, i + length(token))
    }
    return out line
  }
  {
    $0 = lit($0, "TURN_REALM"); $0 = lit($0, "TURN_EXTERNAL_IP"); $0 = lit($0, "TURN_RELAY_IP")
    $0 = lit($0, "TURN_MIN_PORT"); $0 = lit($0, "TURN_MAX_PORT"); $0 = lit($0, "TURN_SECRET")
    print
  }' "$here/turnserver.conf.template" > "$out"
echo "wrote $out"
