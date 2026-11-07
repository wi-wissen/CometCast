// SPDX-License-Identifier: GPL-3.0-or-later
//! C interface of the Windows Miracast receiver (`cc_miracast_ffi.dll`), called from the
//! Flutter app through `dart:ffi` (`apps/flutter/lib/core/miracast.dart`).
//!
//! Shape: everything is POLLED, like the video path of `cc-ffi`. No callbacks cross the
//! boundary. Text goes in and out as JSON (small, versioned, easy to test on both sides);
//! pictures go out as raw BGRA through a buffer the caller owns.
//!
//! * [`controller`]: lifecycle, consent request, counters; platform independent, tested on
//!   Linux with a fake receiver.
//! * [`abi`]: the `extern "C"` functions (the only place with `unsafe`).
//!
//! HONEST STATE: the Windows receiver behind this was never run (docs/miracast.md).

pub mod abi;
pub mod controller;
