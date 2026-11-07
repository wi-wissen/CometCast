// SPDX-License-Identifier: GPL-3.0-or-later
//! Thin, stable facade for other languages.
//!
//! Today: flutter_rust_bridge (FRB) bindings for the Flutter app, see [`api`].
//! Later: UniFFI bindings (Swift, for tvOS and the iOS/macOS extensions).
//!
//! Layout: `api` is the hand-written API that FRB reads. `frb_generated` is written by
//! `flutter_rust_bridge_codegen` (do not edit it by hand, see docs/flutter-bridge.md).

pub mod api;
mod content;
mod content_sender;
mod encoded;
mod video;

// Generated code uses `unsafe` (C calls) and has no docs, so it opts out of those lints.
// `rustfmt::skip`: the generator formats the file itself; `cargo fmt` must not fight it.
#[rustfmt::skip]
#[allow(unsafe_code, missing_docs, clippy::all)]
mod frb_generated;
