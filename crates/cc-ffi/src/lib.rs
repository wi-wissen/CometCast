// SPDX-License-Identifier: GPL-3.0-or-later
//! Thin, stable facade for other languages.
//!
//! Today: flutter_rust_bridge (FRB) bindings for the Flutter app, see [`api`].
//! Later: UniFFI bindings (Swift, for tvOS and the iOS/macOS extensions).
//!
//! Layout: `api` is the hand-written API that FRB reads. `frb_generated` is written by
//! `flutter_rust_bridge_codegen` (do not edit it by hand, see docs/flutter-bridge.md).

// `h264` alone would build the bridge with H.264 types but no OpenH264 at all (neither the
// source build nor the run-time library): every video start would fail. Choose one of the two.
#[cfg(all(
    feature = "h264",
    not(any(feature = "h264-source", feature = "h264-libloading"))
))]
compile_error!(
    "feature `h264` of cc-ffi needs a build mode: enable `h264-source` (development) or \
     `h264-libloading` (releases); see docs/capture.md, \"H.264 licensing\""
);

pub mod api;
mod content;
mod content_sender;
mod encoded;
mod office;
mod video;

// Generated code uses `unsafe` (C calls) and has no docs, so it opts out of those lints.
// `rustfmt::skip`: the generator formats the file itself; `cargo fmt` must not fight it.
#[rustfmt::skip]
#[allow(unsafe_code, missing_docs, clippy::all)]
mod frb_generated;
