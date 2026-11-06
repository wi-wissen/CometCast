#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Fills deploy/coturn/turnserver.conf.template with values from deploy/.env and
# deploy/secrets/turn_secret and writes deploy/coturn/turnserver.conf (not committed).
# Run from anywhere:  deploy/coturn/render.sh
#
# Safety rules:
#  - .env is NOT run as shell code. It is read line by line as KEY=VALUE.
#  - Every value is checked and the script stops with an error otherwise: realm from a short
#    list of characters, addresses as real IPv4/IPv6 addresses (not unspecified, loopback or
#    multicast), ports as numbers 1-65535 with min <= max, the secret as ONE line of safe
#    characters (a file with several lines is refused, never merged).
#  - The values are inserted as plain text (awk, no pattern matching), so characters such
#    as & | \ have no special meaning anyway.
#  - The output file is written with mode 600 (also when an older file with other
#    permissions exists), via a temporary file that replaces it in one step.
#
# UNCERTAIN: tested with dry renders (deploy/coturn/test-render.sh), not with a real coturn.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
deploy=$(dirname "$here")

die() {
  echo "error: $*" >&2
  exit 1
}

[ -f "$deploy/.env" ] || die "missing $deploy/.env (copy .env.example)"
[ -f "$deploy/secrets/turn_secret" ] || die "missing $deploy/secrets/turn_secret"

nl='
'

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

# Stop unless "$2" (the value of setting "$1") is one line that matches the grep pattern "$3"
# completely.
check() {
  case $2 in
    *"$nl"*) die "$1 has more than one line" ;;
  esac
  if ! printf '%s\n' "$2" | grep -Eq "^($3)\$"; then
    die "$1 has an empty value or characters that are not allowed here"
  fi
}

# Stop unless "$2" is a plausible address for setting "$1": a dotted-quad IPv4 address or an
# IPv6 address in hex notation, and not unspecified, loopback or multicast (such a value
# can only be a mistake in a relay setting).
check_ip() {
  case $2 in
    *"$nl"*) die "$1 has more than one line" ;;
  esac
  if ! printf '%s\n' "$2" | awk '
    function v4(s,    n, a, i) {
      n = split(s, a, ".")
      if (n != 4) return 0
      for (i = 1; i <= 4; i++) {
        if (a[i] !~ /^(0|[1-9][0-9]*)$/ || length(a[i]) > 3 || a[i] + 0 > 255) return 0
      }
      if (a[1] + 0 == 0 || a[1] + 0 == 127 || a[1] + 0 >= 224) return 0
      return 1
    }
    function v6(s,    n, a, i, cnt, tmp, dbl, first) {
      if (s !~ /^[0-9A-Fa-f:]+$/) return 0
      if (s ~ /:::/) return 0
      tmp = s
      dbl = gsub(/::/, "@", tmp)
      if (dbl > 1) return 0
      if (s ~ /^:[^:]/ || s ~ /[^:]:$/) return 0
      n = split(s, a, ":")
      cnt = 0
      for (i = 1; i <= n; i++) {
        if (a[i] != "") {
          if (length(a[i]) > 4) return 0
          cnt++
        }
      }
      if (dbl == 1) { if (cnt > 7) return 0 } else if (cnt != 8) return 0
      # not :: (unspecified), not ::1 (loopback), not ff00::/8 (multicast)
      if (s == "::" || s == "::1") return 0
      first = tolower(a[1])
      if (length(first) == 4 && substr(first, 1, 2) == "ff") return 0
      return 1
    }
    { ok = ($0 ~ /^[0-9.]+$/) ? v4($0) : v6($0); exit(ok ? 0 : 1) }'
  then
    die "$1 is not a usable IPv4/IPv6 address"
  fi
}

TURN_REALM=$(env_value TURN_REALM)
TURN_EXTERNAL_IP=$(env_value TURN_EXTERNAL_IP)
TURN_RELAY_IP=$(env_value TURN_RELAY_IP)
TURN_MIN_PORT=$(env_value TURN_MIN_PORT)
TURN_MAX_PORT=$(env_value TURN_MAX_PORT)

# The secret file must hold exactly one line (one trailing line end is fine) of safe
# characters. Several lines are an error: they are never joined into one secret.
secret_file="$deploy/secrets/turn_secret"
lines=$(awk 'END { print NR }' "$secret_file")
[ "$lines" = 1 ] || die "secrets/turn_secret must contain exactly one line (found $lines)"
stray=$(LC_ALL=C tr -d 'A-Za-z0-9._~+/=\r\n-' < "$secret_file" | wc -c)
[ "$stray" -eq 0 ] || die "secrets/turn_secret has characters that are not allowed here"
TURN_SECRET=$(head -n 1 "$secret_file")
cr=$(printf '\r')
TURN_SECRET=${TURN_SECRET%"$cr"}   # a Windows line end

check TURN_REALM "$TURN_REALM" '[A-Za-z0-9.-]+'
check_ip TURN_EXTERNAL_IP "$TURN_EXTERNAL_IP"
check_ip TURN_RELAY_IP "$TURN_RELAY_IP"
# Ports: 1-65535, no leading zero (so the shell never reads them as octal numbers).
check TURN_MIN_PORT "$TURN_MIN_PORT" '[1-9][0-9]{0,4}'
check TURN_MAX_PORT "$TURN_MAX_PORT" '[1-9][0-9]{0,4}'
[ "$TURN_MIN_PORT" -le 65535 ] || die "TURN_MIN_PORT must be at most 65535"
[ "$TURN_MAX_PORT" -le 65535 ] || die "TURN_MAX_PORT must be at most 65535"
[ "$TURN_MIN_PORT" -le "$TURN_MAX_PORT" ] || die "TURN_MIN_PORT must not be above TURN_MAX_PORT"
# The secret: at least 16 characters, from a safe set (openssl rand -hex 32 fits).
check "secrets/turn_secret" "$TURN_SECRET" '[A-Za-z0-9._~+/=-]{16,}'

out="$here/turnserver.conf"
umask 077   # the file contains the shared secret
tmp=$(mktemp "$here/.turnserver.conf.XXXXXX") || die "cannot create a temporary file in $here"
trap 'rm -f "$tmp"' EXIT HUP INT TERM
chmod 600 "$tmp"
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
  }' "$here/turnserver.conf.template" > "$tmp"
# Replace the old file in one step; the new one keeps mode 600 whatever the old one had.
mv -f "$tmp" "$out"
chmod 600 "$out"
trap - EXIT HUP INT TERM
echo "wrote $out"
