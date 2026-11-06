// SPDX-License-Identifier: GPL-3.0-or-later
//! Documents: render PDF pages on the receiver, check and encode page images, keep the
//! page state that remote-control commands drive.
//!
//! Plan decision: a PDF is sent **once as a file** (not streamed as video) and every
//! device renders the pages itself with the same renderer, [`hayro`](https://crates.io/crates/hayro)
//! (pure Rust, Apache-2.0 OR MIT). This crate is that renderer plus the limits around it.
//!
//! * [`Document::open`] checks hard limits **before** parsing, then parses and renders in a
//!   **child process** with memory and CPU limits (see [`Limits`], [`Isolation`], and
//!   `docs/documents.md`). A PDF comes from a remote sender and is treated as hostile; hayro
//!   cannot be interrupted and aborts the whole process on out-of-memory, so the process is
//!   the boundary. iOS, tvOS and `wasm32` have no child processes and use a weaker in-process
//!   engine.
//! * The worker program is the `cc-docs-worker` binary of this crate or any program that
//!   calls [`worker_main`] (`cometcast-cli docs-worker`).
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

#[cfg(not(target_arch = "wasm32"))]
mod clamp;
mod document;
mod encode;
mod error;
#[cfg(not(target_arch = "wasm32"))]
mod hardening;
mod isolation;
mod prescan;
#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "ios"),
    not(target_os = "tvos")
))]
mod process;
#[cfg(not(target_arch = "wasm32"))]
mod process_stats;
mod protection;
mod session;
#[cfg(not(target_arch = "wasm32"))]
mod wire;
mod worker;
#[cfg(not(target_arch = "wasm32"))]
mod worker_main;

pub use document::{Document, Limits, PageSize, RenderOptions, RenderedPage, abandoned_renders};
pub use isolation::{Isolation, WORKER_ENV, WORKER_FILE_NAME, WorkerSpec};
#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "ios"),
    not(target_os = "tvos")
))]
pub use process::{
    active_workers, max_concurrent_workers, set_max_concurrent_workers, worker_limits_available,
};
pub use protection::Protection;
#[cfg(not(target_arch = "wasm32"))]
pub use worker_main::worker_main;

/// Restrict the **calling process** the way the render worker restricts itself (resource
/// limits, Landlock, seccomp) and report what was applied. Only for the tests of this crate,
/// which run it in a child copy of the test program; do not call it from an application:
/// the limits and filters cannot be undone.
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub fn enter_worker_sandbox_for_tests(limits: &Limits) -> Result<Vec<Protection>, DocError> {
    hardening::apply(hardening::Plan {
        memory_bytes: limits.max_worker_memory_bytes,
        cpu_secs: 60,
        require_os_sandbox: limits.require_os_sandbox,
    })
    .map(|m| m.list())
}

/// There is no render worker process on `wasm32`; returns 2.
#[cfg(target_arch = "wasm32")]
pub fn worker_main() -> i32 {
    2
}
pub use encode::{WebpInfo, encode_png, encode_webp_lossless, validate_webp};
pub use error::DocError;
pub use session::{DocumentSession, SessionEvent};
