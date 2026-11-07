// SPDX-License-Identifier: GPL-3.0-or-later
//! Office files to PDF on the **sender** side. The receiver only ever gets a PDF, so its
//! security model (PDF rendering in a limited child process, `cc-docs`) does not change.
//!
//! ```text
//! file (.docx .pptx .xlsx; OpenDocument and legacy binary files are refused)
//!   -> input checks (regular file, size)          [runner]
//!   -> ZIP pre-flight (entries, names, bombs)     [zipcheck, our own header reader]
//!   -> private work dir (0700) with a copy        [runner]
//!   -> worker child: rlimits, no TCP, own group   [worker]
//!        office2pdf (pure Rust, Apache-2.0), in-process in the worker
//!   -> deadline, kill of the whole process group  [runner]
//!   -> result checks (regular file, cap, %PDF-)   [runner]
//! ```
//!
//! The only engine is the pure-Rust one; see `docs/documents.md` ("Office documents").
//! Nothing here measures how faithful the PDF is on real files.
//!
//! Use [`convert_file`] or [`convert_bytes`] with [`Options`]. The worker program is
//! `cc-office-worker` (next to the application) or any program that calls [`worker_main`],
//! for example the hidden `office-worker` command of `cometcast-cli`.

mod error;
#[cfg(feature = "engine-rust")]
mod fonts;
mod format;
mod runner;
mod worker;
mod zipcheck;

pub use error::{Limits, OfficeError};
pub use format::OfficeFormat;
pub use runner::{
    Converted, Options, WorkerSpec, convert_bytes, convert_file, expected_protections,
};
pub use worker::{worker_main, worker_main_args};
pub use zipcheck::{ZipReport, check as check_container};
