// SPDX-License-Identifier: GPL-3.0-or-later
//! Office files to PDF on the **sender** side. The receiver only ever gets a PDF, so its
//! security model (PDF rendering in a limited child process, `cc-docs`) does not change.
//!
//! ```text
//! file (.docx .pptx .xlsx, .odt .odp .ods)
//!   -> input checks (regular file, size)          [runner]
//!   -> ZIP pre-flight (entries, names, bombs)     [zipcheck, our own header reader]
//!   -> private work dir (0700) with a copy        [runner]
//!   -> worker child: rlimits, no TCP, own group   [worker]
//!        engine "soffice": LibreOffice headless, clean env, own profile, no macros
//!        engine "rust":    office2pdf (pure Rust, Apache-2.0)
//!   -> deadline, kill of the whole process group  [runner]
//!   -> result checks (regular file, cap, %PDF-)   [runner]
//! ```
//!
//! Which engine, and why: see `docs/documents.md` ("Office documents"). Nothing here
//! measures how faithful the PDF is on real classroom files.
//!
//! Use [`convert_file`] or [`convert_bytes`] with [`Options`]. The worker program is
//! `cc-office-worker` (next to the application) or any program that calls [`worker_main`],
//! for example the hidden `office-worker` command of `cometcast-cli`.

mod engine;
mod error;
mod format;
mod runner;
mod worker;
mod zipcheck;

pub use engine::{Concrete, Engine, SOFFICE_ENV, find_soffice, rust_engine_compiled};
pub use error::{Limits, OfficeError};
pub use format::OfficeFormat;
pub use runner::{
    Converted, Options, WorkerSpec, convert_bytes, convert_file, expected_protections,
};
pub use worker::{worker_main, worker_main_args};
pub use zipcheck::{ZipReport, check as check_container};
