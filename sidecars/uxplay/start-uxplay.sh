#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Starts the separate UxPlay program by hand, with the same safe options that the Rust
# supervisor (crates/cc-airplay) uses. For manual tests on a mini PC. The app itself does
# NOT use this script; it starts `uxplay` through the supervisor.
#
# Usage:  start-uxplay.sh [NAME]
# Needs:  uxplay (installed separately, GPL-3.0), GStreamer plugins, Avahi (mDNS).
#
# stdbuf -oL makes UxPlay write each line at once, even when the output goes to a pipe.
# Without it, UxPlay collects its output and the PIN line shows up late (seen with
# UxPlay 1.68). The supervisor uses a pseudo-terminal for the same reason.

set -eu

NAME="${1:-Comet Cast}"
UXPLAY="${UXPLAY:-uxplay}"

# -n NAME  receiver name        -nh   do not add "@hostname"
# -pin     random PIN for every connection (UxPlay 1.68; newer UxPlay may call it -pw)
# -fs      full screen          -p    fixed legacy ports (open them in the firewall)
# Not used on purpose: -reg (would remember devices and skip the PIN later),
#                      -nohold (would drop a running mirroring when a second device connects).
exec stdbuf -oL "$UXPLAY" -n "$NAME" -nh -pin -fs -p
