#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# FAKE "uxplay" for tests. This is NOT UxPlay and contains no UxPlay code. It only prints
# lines that look like the real UxPlay 1.68 output we recorded (see the *-real-*.txt
# fixtures next to this file) and then waits, so the supervisor can be tested without
# an AirPlay stack, a display or audio.
#
# Mode is chosen with the environment variable FAKE_UXPLAY_MODE:
#   normal        banner, ready line, PIN, one connection that opens and closes, then waits
#   crash-once    first run (marked by the file FAKE_UXPLAY_COUNTER) exits with code 3,
#                 later runs behave like "normal"
#   always-crash  exits with code 1 right after the banner
#   ignore-term   like "normal" but ignores SIGTERM (so the supervisor must kill it)
#   chatty        prints a very long line and some junk, then behaves like "normal"

mode=${FAKE_UXPLAY_MODE:-normal}

echo "UxPlay 1.68: An Open-Source AirPlay mirroring and audio-streaming server. (FAKE)"
echo "using network ports UDP 7011 6001 6000 TCP 7100 7000 7001"

case "$mode" in
always-crash)
    echo "*** ERROR: fake failure"
    exit 1
    ;;
crash-once)
    if [ ! -e "$FAKE_UXPLAY_COUNTER" ]; then
        : >"$FAKE_UXPLAY_COUNTER"
        sleep 0.2
        exit 3
    fi
    ;;
ignore-term)
    trap '' TERM
    ;;
chatty)
    printf 'x%.0s' $(seq 1 20000)
    echo
    printf '\033[31mcolour\033[0m \001\002 junk\n'
    ;;
*)
    trap 'echo "Stopping..."; exit 0' TERM
    ;;
esac

echo "Initialized server socket(s)"
sleep 0.2
echo "Accepted IPv4 client on socket 16"
echo "client sent PAIR-PIN-START request"
echo '*** CLIENT MUST NOW ENTER PIN = "9876" AS AIRPLAY PASSWORD'
echo "Open connections: 1"
sleep 0.3
echo "Open connections: 0"

# Wait until we get a signal. Short sleeps so that a signal is handled quickly.
while true; do
    sleep 0.1
done
