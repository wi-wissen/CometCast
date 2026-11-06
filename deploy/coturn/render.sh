#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Fills deploy/coturn/turnserver.conf.template with values from deploy/.env and
# deploy/secrets/turn_secret and writes deploy/coturn/turnserver.conf (not committed).
# Run from anywhere:  deploy/coturn/render.sh
#
# UNCERTAIN: tested only for syntax with `sh -n`, not with a real coturn.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
deploy=$(dirname "$here")

[ -f "$deploy/.env" ] || { echo "missing $deploy/.env (copy .env.example)" >&2; exit 1; }
[ -f "$deploy/secrets/turn_secret" ] || { echo "missing $deploy/secrets/turn_secret" >&2; exit 1; }

# Read the settings. `set -a` exports everything that is assigned.
set -a
# shellcheck disable=SC1091
. "$deploy/.env"
set +a
TURN_SECRET=$(tr -d '\n\r' < "$deploy/secrets/turn_secret")

: "${TURN_REALM:?}" "${TURN_EXTERNAL_IP:?}" "${TURN_RELAY_IP:?}" "${TURN_MIN_PORT:?}" "${TURN_MAX_PORT:?}"

out="$here/turnserver.conf"
umask 077   # the file contains the shared secret
sed \
  -e "s|@TURN_REALM@|$TURN_REALM|g" \
  -e "s|@TURN_EXTERNAL_IP@|$TURN_EXTERNAL_IP|g" \
  -e "s|@TURN_RELAY_IP@|$TURN_RELAY_IP|g" \
  -e "s|@TURN_MIN_PORT@|$TURN_MIN_PORT|g" \
  -e "s|@TURN_MAX_PORT@|$TURN_MAX_PORT|g" \
  -e "s|@TURN_SECRET@|$TURN_SECRET|g" \
  "$here/turnserver.conf.template" > "$out"
echo "wrote $out"
