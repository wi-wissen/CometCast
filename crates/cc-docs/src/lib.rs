// SPDX-License-Identifier: GPL-3.0-or-later
//! Documents: render PDF pages on the receiver, check and encode page images, keep the
//! page state that remote-control commands drive.
//!
//! Plan decision: a PDF is sent **once as a file** (not streamed as video) and every
//! device renders the pages itself with the same renderer, [`hayro`](https://crates.io/crates/hayro)
//! (pure Rust, Apache-2.0 OR MIT). This crate is that renderer plus the limits around it.
//!
//! * [`Document::open`] checks hard limits **before** parsing, then parses on a guarded
//!   worker thread (see [`Limits`]).
//! * [`Document::render_page`] and [`Document::thumbnail`] give RGBA pictures
//!   ([`RenderedPage`]); [`encode_png`] / [`encode_webp_lossless`] / [`validate_webp`] are
//!   the pure-Rust image helpers.
//! * [`DocumentSession`] is the receiver-side page state driven by `cc_proto::ControlCmd`.
//!
//! Page numbers: [`Document`] uses 0-based page **indices**; [`DocumentSession`] and
//! `ControlCmd::GoToPage` use 1-based page **numbers** (the first page is 1).
//!
//! UNCERTAIN: hayro describes itself as experimental. It was tried here only on the small
//! generated PDFs in `tests/data`, not on the 20 real classroom PDFs the plan asks for
//! (measurement point 7). See `docs/documents.md` for what was and was not checked.
//!
//! Not in this crate (later steps): Office conversion, the WASM build for the web app,
//! speaker notes (they come from PPTX; PDFs have none, see [`Document::notes`]).

mod document;
mod encode;
mod error;
mod session;
mod worker;

pub use document::{Document, Limits, PageSize, RenderOptions, RenderedPage, abandoned_renders};
pub use encode::{WebpInfo, encode_png, encode_webp_lossless, validate_webp};
pub use error::DocError;
pub use session::{DocumentSession, SessionEvent};
