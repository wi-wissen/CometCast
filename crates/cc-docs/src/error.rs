// SPDX-License-Identifier: GPL-3.0-or-later
//! Errors of `cc-docs`.

/// Why a document could not be opened or a page could not be rendered.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocError {
    /// The input has no bytes.
    #[error("the file is empty")]
    Empty,
    /// The input is larger than [`Limits::max_file_bytes`](crate::Limits).
    #[error("the file is too large ({size} bytes, limit {max} bytes)")]
    TooLarge {
        /// Size of the input.
        size: usize,
        /// The limit that applies.
        max: usize,
    },
    /// No `%PDF-` header near the start of the file.
    #[error("not a PDF file (no %PDF- header in the first 1024 bytes)")]
    NotPdf,
    /// The file needs a password (this crate never asks for one).
    #[error("the PDF is password protected")]
    PasswordProtected,
    /// The encryption is invalid or of a kind the renderer does not support.
    #[error("the PDF uses encryption that cannot be handled ({0})")]
    UnsupportedEncryption(&'static str),
    /// The PDF could not be parsed.
    #[error("the PDF is damaged or not supported")]
    Invalid,
    /// The document has no page.
    #[error("the PDF has no pages")]
    NoPages,
    /// More pages than [`Limits::max_pages`](crate::Limits).
    #[error("too many pages ({count}, limit {max})")]
    TooManyPages {
        /// Pages found.
        count: usize,
        /// The limit that applies.
        max: usize,
    },
    /// The page index is not in the document.
    #[error("page index {index} is out of range (document has {count} pages)")]
    PageOutOfRange {
        /// Requested 0-based index.
        index: usize,
        /// Pages in the document.
        count: usize,
    },
    /// The page size in the file is zero, negative, not finite or otherwise unusable.
    #[error("the page has an unusable size")]
    InvalidPageSize,
    /// The requested picture would be larger than the output limits.
    #[error("the requested picture of {width} x {height} pixels exceeds the output limits")]
    OutputTooLarge {
        /// Requested or derived width in pixels.
        width: u64,
        /// Requested or derived height in pixels.
        height: u64,
    },
    /// Width of 0 pixels requested.
    #[error("the requested width must be at least 1 pixel")]
    InvalidWidth,
    /// Opening or rendering took longer than the time limit.
    #[error("the time limit was exceeded")]
    Timeout,
    /// The caller cancelled through the cancel flag.
    #[error("cancelled")]
    Cancelled,
    /// The renderer panicked on this input (caught).
    #[error("the renderer failed on this document")]
    Panicked,
    /// Too many earlier renders that timed out are still running in the background
    /// (in-process engine only).
    #[error("too many abandoned renders are still running")]
    Busy,
    /// An earlier render of this document (or of a file with the same content) timed out in
    /// the in-process engine; it is not rendered again so that one bad file costs one thread.
    #[error("an earlier render of this document timed out; it is not tried again")]
    Poisoned,
    /// The pre-scan refused the document before rendering (see [`Limits`](crate::Limits)).
    #[error("the PDF was refused by the safety scan: {0}")]
    Rejected(&'static str),
    /// The render process used more memory than [`Limits::max_worker_memory_bytes`](crate::Limits).
    #[error("the renderer needed more memory than allowed")]
    MemoryLimit,
    /// The render process ended without an answer (crash, out of memory, CPU limit, signal).
    #[error("the renderer process failed: {0}")]
    WorkerFailed(&'static str),
    /// The render process could not be started or found.
    #[error("the render worker is not available: {0}")]
    WorkerUnavailable(String),
    /// A worker thread could not be started or died unexpectedly.
    #[error("internal error: {0}")]
    Internal(&'static str),
    /// Encoding a picture failed.
    #[error("image encoding failed: {0}")]
    Encode(String),
    /// A WebP file failed validation.
    #[error("invalid WebP: {0}")]
    InvalidWebp(String),
}
