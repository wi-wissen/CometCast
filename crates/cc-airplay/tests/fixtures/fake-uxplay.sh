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
#   spoof         like "normal" but first prints a PIN line with no pairing line before it
#   twopins       like "normal" but prints a second pairing and PIN (5432) at the end
#   helper        like "normal" but starts a background helper (pid in FAKE_UXPLAY_HELPER_PID)
#   helper-crash  starts a background helper, then exits with code 3
#   self-kill     kills itself with SIGKILL after the banner
#   mirror        like "normal", then a device mirrors: a second connection opens and the mirror
#                 line is printed (UxPlay's `raop_rtp_mirror starting mirroring`); it goes on
#                 until the process ends
#   mirror-ends   like "mirror", but the connection closes again after a moment
#                 (`Open connections: 0`)
#   mirror-spoof  prints the mirror line BEFORE any pairing (a device that never paired)
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
spoof)
    trap 'echo "Stopping..."; exit 0' TERM
    echo '*** CLIENT MUST NOW ENTER PIN = "1111" AS AIRPLAY PASSWORD'
    ;;
twopins)
    trap 'echo "Stopping..."; exit 0' TERM
    ;;
helper)
    trap 'echo "Stopping..."; exit 0' TERM
    sleep 300 &
    echo $! >>"$FAKE_UXPLAY_HELPER_PID"
    ;;
helper-crash)
    sleep 300 &
    echo $! >>"$FAKE_UXPLAY_HELPER_PID"
    sleep 0.3
    exit 3
    ;;
self-kill)
    sleep 0.2
    kill -KILL $$
    ;;
chatty)
    printf 'x%.0s' $(seq 1 20000)
    echo
    printf '\033[31mcolour\033[0m \001\002 junk\n'
    ;;
mirror-spoof)
    trap 'echo "Stopping..."; exit 0' TERM
    echo "raop_rtp_mirror starting mirroring"
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

if [ "$mode" = twopins ]; then
    echo "client sent PAIR-PIN-START request"
    echo '*** CLIENT MUST NOW ENTER PIN = "5432" AS AIRPLAY PASSWORD'
fi

case "$mode" in
mirror | mirror-ends)
    # The real flow: the probe connection above closed, a second one carries the session.
    echo "Open connections: 1"
    echo "raop_rtp_mirror starting mirroring"
    if [ "$mode" = mirror-ends ]; then
        sleep 0.3
        echo "Open connections: 0"
    fi
    ;;
esac

# Wait until we get a signal. Short sleeps so that a signal is handled quickly.
while true; do
    sleep 0.1
done
