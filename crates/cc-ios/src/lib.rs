// SPDX-License-Identifier: GPL-3.0-or-later
//! The sender core for the iOS broadcast extension, behind a small C ABI.
//!
//! The extension (Swift, `apps/flutter/ios/BroadcastExtension`) receives screen pictures from
//! ReplayKit, encodes them with VideoToolbox and hands every H.264 access unit to
//! [`cc_ios_push_frame`](ffi). This crate joins the room, negotiates WebRTC and sends the
//! units; it is `cc-sender` in its `ExternalH264` mode (the same path as `push_encoded_frame`
//! on Android), nothing else. See docs/ios.md for the architecture and the memory budget.
//!
//! * [`engine`]: the safe Rust API (start, push, stop, status events). Unit and loopback tests
//!   use it directly.
//! * [`ffi`]: the `extern "C"` functions declared in `include/cc_ios.h`. The only module
//!   with `unsafe`.
//! * [`share`]: the engine of the share extension (one item, remote controls, polling).
//! * [`log`]: a small size-bounded log file in the App Group container, because an extension
//!   has no console that a person can read after the fact.
//!
//! UNCERTAIN: this crate was written and tested on Linux (host target). It was never linked
//! into an extension and never ran on an iPhone; see docs/ios-device-test-plan.md.

pub mod engine;
pub mod ffi;
pub mod log;
pub mod share;

pub use engine::{Config, ErrorCode, PushCode, State, Status};
