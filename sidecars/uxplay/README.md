<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# UxPlay sidecar

Comet Cast receives AirPlay with [UxPlay](https://github.com/FDH2/UxPlay) (GPL-3.0). UxPlay is
a **separate program**. It is not linked into Comet Cast and none of its code is copied here.

This folder holds only:

- `start-uxplay.sh`: a hand-start script with the safe options, for manual tests.

**Nothing from UxPlay is stored in this repository yet.** Install it yourself, for example
`apt-get install uxplay` on Ubuntu (package 1.68.2-1build2, checked on 2026-11-06).

When Comet Cast starts to ship a UxPlay build, the release must also contain the UxPlay licence
text and an offer of (or link to) the matching UxPlay source code. See `docs/airplay.md` and
`THIRD_PARTY_NOTICES.md`.

The supervisor in `crates/cc-airplay` does the real work (start, watch, restart, stop).
