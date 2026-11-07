// SPDX-License-Identifier: GPL-3.0-or-later
//! Errors and limits.

use std::time::Duration;

/// Why a conversion did not produce a PDF.
#[derive(Debug, thiserror::Error)]
pub enum OfficeError {
    /// The file extension is not one of the accepted formats.
    #[error(
        "unsupported file type: only .docx .pptx .xlsx (and .odt .odp .ods with LibreOffice) can be converted"
    )]
    UnsupportedFormat,
    /// An old binary format or an encrypted OOXML file (both are OLE containers, not ZIP).
    #[error(
        "this is an old binary Office file or a password-protected one; save it as .docx/.pptx/.xlsx without a password and try again"
    )]
    LegacyOrEncrypted,
    /// The file could not be read.
    #[error("cannot read the file: {0}")]
    Io(String),
    /// The input is larger than the limit.
    #[error("the file is too large (at most {limit} bytes)")]
    InputTooLarge {
        /// The limit that was exceeded.
        limit: u64,
    },
    /// The input is not a regular file (device, FIFO, directory).
    #[error("not a regular file")]
    NotRegularFile,
    /// The ZIP container is damaged, hostile or does not match its extension.
    #[error("the document container was refused: {0}")]
    BadContainer(String),
    /// The document looks like a decompression bomb.
    #[error("the document was refused because it would expand too much: {0}")]
    TooMuchData(String),
    /// No engine can convert this file (feature off, LibreOffice not found).
    #[error("no conversion engine is available: {0}")]
    NoEngine(String),
    /// The worker program was not found.
    #[error("the conversion worker program was not found: {0}")]
    WorkerUnavailable(String),
    /// The engine ran and failed.
    #[error("the conversion failed: {0}")]
    ConversionFailed(String),
    /// The engine did not finish in time; it was killed.
    #[error("the conversion took longer than {0:?} and was stopped")]
    Timeout(Duration),
    /// The result is not a usable PDF or is larger than the cap.
    #[error("the conversion result was refused: {0}")]
    BadOutput(String),
}

/// Limits for one conversion. Every one is checked; none is advisory.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Largest input file.
    pub max_input_bytes: u64,
    /// Most entries in the ZIP container.
    pub max_zip_entries: usize,
    /// Largest single entry after decompression (declared and actually inflated).
    pub max_entry_bytes: u64,
    /// Largest sum of all entries after decompression.
    pub max_total_uncompressed_bytes: u64,
    /// Largest ratio uncompressed/compressed for an entry that is larger than 1 MiB.
    pub max_ratio: u64,
    /// Wall-clock time for the engine; the whole process group is killed after it.
    pub timeout: Duration,
    /// Largest PDF the conversion may produce (also the per-file write limit of the worker).
    pub max_output_bytes: u64,
    /// Address-space limit of the worker with the pure-Rust engine (Linux, other Unix).
    pub rust_engine_memory_bytes: u64,
    /// Address-space limit for the worker that supervises LibreOffice (which is a big
    /// program). UNCERTAIN: 4 GiB worked for the small test files on one machine; large
    /// real files may need more or less.
    pub soffice_memory_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 50 * 1024 * 1024,
            max_zip_entries: 4000,
            max_entry_bytes: 128 * 1024 * 1024,
            max_total_uncompressed_bytes: 512 * 1024 * 1024,
            max_ratio: 200,
            timeout: Duration::from_secs(120),
            max_output_bytes: 64 * 1024 * 1024,
            rust_engine_memory_bytes: 4 * 1024 * 1024 * 1024,
            soffice_memory_bytes: 4 * 1024 * 1024 * 1024,
        }
    }
}
