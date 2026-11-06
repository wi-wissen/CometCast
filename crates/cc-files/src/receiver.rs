// SPDX-License-Identifier: GPL-3.0-or-later
//! The receiver side: reassembles transfers from the control data channel, validates
//! everything and drives a PDF through `cc-docs`.
//!
//! [`ContentReceiver`] is a plain state machine: feed it each text and binary message of the
//! channel in order, collect [`ContentEvent`]s. It does no network I/O and never panics on
//! hostile input. The only calls that can take long are opening a document and rendering a
//! page (both happen in the sandboxed render process of `cc-docs`, up to its time limits), so
//! callers run [`ContentReceiver::handle_text`] off the async runtime's threads
//! (`cc-receiver` does).
//!
//! Safety rules (each one has a test):
//! * Memory is bounded: at most one transfer is held, its buffer is reserved once at the
//!   announced size (which is checked against a hard limit first), and a chunk that would
//!   go past the announced size ends the transfer.
//! * Every chunk is exactly 16 KiB except the last one of a transfer, so the number of
//!   messages is bounded by `size / 16 KiB` (a flood of tiny messages is refused). The
//!   count is also checked explicitly.
//! * Chunks without a transfer, an end message of the wrong kind, a size mismatch and a new
//!   transfer that interrupts one are protocol violations. Too many violations (default 64)
//!   block the receiver: further input is ignored and the owner is told to close the channel.
//! * Links are validated again here and **never opened**; the event only asks.
//! * A PDF is only handed to `cc-docs`, which checks it again, renders in a child process
//!   and treats it as hostile. A page is only rendered after its size was checked against
//!   the output limits.

use std::ops::Deref;
use std::sync::Arc;

use cc_docs::{DocError, Document, DocumentSession, Isolation, Limits, SessionEvent as DocEvent};
use cc_proto::ControlCmd;

use crate::wire::{
    CHUNK_SIZE, ImageMime, MAX_DOCUMENT_BYTES, MAX_IMAGE_BYTES, Message, ParseError, parse_message,
};

/// Smallest width a page is rendered at, in pixels.
const MIN_RENDER_WIDTH: u32 = 16;
/// Width used until the application sets one.
pub const DEFAULT_TARGET_WIDTH: u32 = 1280;
/// Protocol violations after which a receiver blocks.
/// UNCERTAIN: placeholder, unmeasured.
pub const DEFAULT_MAX_VIOLATIONS: u32 = 64;

/// Bytes shared between events. `Debug` prints only the length, so logging an event does not
/// dump a picture.
#[derive(Clone, PartialEq, Eq)]
pub struct Blob(Arc<Vec<u8>>);

impl Blob {
    /// Wrap bytes.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(Arc::new(bytes))
    }
}

impl Deref for Blob {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for Blob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Blob({} bytes)", self.0.len())
    }
}

/// Why something was refused or failed. The codes in [`ContentError::code`] for the first six
/// are the ones of the web client (`TransferErrorCode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentError {
    /// A text message that is malformed, unknown or has invalid fields (also: a transfer
    /// announcement over the limit or with a type that is not allowed).
    BadMessage,
    /// A chunk without a transfer.
    UnexpectedChunk,
    /// More data than announced (or the buffer could not be reserved).
    TooLarge,
    /// The end message came with fewer bytes than announced.
    SizeMismatch,
    /// An end message without a matching transfer.
    NoTransfer,
    /// A chunk that breaks the 16 KiB rule.
    BadChunk,
    /// A new transfer started before the previous one ended; the old one was dropped.
    Interrupted,
    /// The PDF was refused by `cc-docs` (damaged, password, too large, too many pages, ...).
    DocumentRejected,
    /// A page could not be rendered.
    RenderFailed,
    /// Too many protocol violations; the receiver ignores further input.
    TooManyErrors,
}

impl ContentError {
    /// Stable machine-readable code.
    pub fn code(self) -> &'static str {
        match self {
            Self::BadMessage => "bad_message",
            Self::UnexpectedChunk => "unexpected_chunk",
            Self::TooLarge => "too_large",
            Self::SizeMismatch => "size_mismatch",
            Self::NoTransfer => "no_transfer",
            Self::BadChunk => "bad_chunk",
            Self::Interrupted => "interrupted",
            Self::DocumentRejected => "document_rejected",
            Self::RenderFailed => "render_failed",
            Self::TooManyErrors => "too_many_errors",
        }
    }
}

/// What the receiver reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentEvent {
    /// A transfer began (name already cleaned). For a progress display.
    TransferStarted {
        /// `true` for a PDF, `false` for an image.
        document: bool,
        /// Display name.
        name: String,
        /// Announced size in bytes.
        size: u64,
    },
    /// A complete image arrived. It replaces what was shown before.
    ImageReceived {
        /// Display name.
        name: String,
        /// The declared type (from the allow list; the bytes are not decoded here).
        mime: ImageMime,
        /// The file's bytes.
        bytes: Blob,
    },
    /// The sender asks the receiver to show a link. **Never opened automatically**: show the
    /// host and let the person decide. The URL is validated (http/https only).
    LinkRequested {
        /// The normalised URL.
        url: String,
    },
    /// A PDF was accepted by `cc-docs` and is now the shown content.
    DocumentOpened {
        /// Display name.
        name: String,
        /// Number of pages (at least 1).
        pages: u32,
    },
    /// A page was rendered.
    PageRendered {
        /// Page number, 1-based.
        page: u32,
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// RGBA pixels, `width * height * 4` bytes, opaque white background.
        rgba: Blob,
    },
    /// The sender stopped the content; whatever was shown should disappear.
    ContentStopped,
    /// Something was refused or failed. The detail is for logs, not for translation.
    Error {
        /// The kind of problem.
        error: ContentError,
        /// A short English explanation.
        detail: String,
    },
}

/// Hard limits of a [`ContentReceiver`].
#[derive(Debug, Clone)]
pub struct ContentOptions {
    /// Largest image accepted, in bytes.
    pub max_image_bytes: u64,
    /// Largest PDF accepted, in bytes (also limited by `doc_limits.max_file_bytes`).
    pub max_document_bytes: u64,
    /// Limits for `cc-docs` (page count, output size, time, memory of the render process).
    pub doc_limits: Limits,
    /// Where the PDF renderer runs. The default [`Isolation::Auto`] needs a worker program;
    /// see `cc_docs::WorkerSpec::discover`.
    pub isolation: Isolation,
    /// Width in pixels that pages are rendered at (clamped to the output limits).
    pub target_width: u32,
    /// Protocol violations after which the receiver blocks.
    pub max_violations: u32,
}

impl Default for ContentOptions {
    fn default() -> Self {
        Self {
            max_image_bytes: MAX_IMAGE_BYTES,
            max_document_bytes: MAX_DOCUMENT_BYTES,
            doc_limits: Limits::default(),
            isolation: Isolation::Auto,
            target_width: DEFAULT_TARGET_WIDTH,
            max_violations: DEFAULT_MAX_VIOLATIONS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Image(ImageMime),
    Document,
}

struct Active {
    kind: Kind,
    name: String,
    size: usize,
    buf: Vec<u8>,
    chunks: u64,
    max_chunks: u64,
}

/// A document that is shown.
struct Shown {
    doc: Document,
    pages: DocumentSession,
}

/// The receiver state machine. See the module docs.
pub struct ContentReceiver {
    opts: ContentOptions,
    active: Option<Active>,
    shown: Option<Shown>,
    violations: u32,
    blocked: bool,
    /// After an `unexpected_chunk` event, further ones are counted but not reported until
    /// the next text message (a flood must not flood the event queue).
    quiet_chunks: bool,
}

impl ContentReceiver {
    /// A receiver with the given options.
    pub fn new(opts: ContentOptions) -> Self {
        Self {
            opts,
            active: None,
            shown: None,
            violations: 0,
            blocked: false,
            quiet_chunks: false,
        }
    }

    /// `true` after too many violations: all input is ignored and the owner should close the channel.
    pub fn is_blocked(&self) -> bool {
        self.blocked
    }

    /// Protocol violations seen so far.
    pub fn violations(&self) -> u32 {
        self.violations
    }

    /// `true` while a transfer is being received.
    pub fn transfer_active(&self) -> bool {
        self.active.is_some()
    }

    /// The shown document's current page (1-based) and page count, if a document is shown.
    pub fn document_state(&self) -> Option<(u32, u32)> {
        self.shown
            .as_ref()
            .map(|s| (s.pages.current_page(), s.pages.page_count()))
    }

    fn violation(&mut self, out: &mut Vec<ContentEvent>, error: ContentError, detail: &str) {
        self.violations = self.violations.saturating_add(1);
        out.push(ContentEvent::Error {
            error,
            detail: detail.to_string(),
        });
        self.check_blocked(out);
    }

    fn check_blocked(&mut self, out: &mut Vec<ContentEvent>) {
        if !self.blocked && self.violations > self.opts.max_violations {
            self.blocked = true;
            self.active = None;
            out.push(ContentEvent::Error {
                error: ContentError::TooManyErrors,
                detail: "too many protocol violations; the channel is ignored from now on".into(),
            });
        }
    }

    /// Handle one text message.
    pub fn handle_text(&mut self, text: &str) -> Vec<ContentEvent> {
        let mut out = Vec::new();
        if self.blocked {
            return out;
        }
        self.quiet_chunks = false;
        let msg = parse_message(
            text,
            self.opts.max_image_bytes,
            self.opts.max_document_bytes.min(self.opts.doc_limits.max_file_bytes as u64),
        );
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                // A refused start message must not leave an older transfer open to take
                // the chunks meant for it.
                self.active = None;
                let detail = match e {
                    ParseError::TooLong => "message too long",
                    ParseError::Malformed => "not a control message",
                    ParseError::UnknownKind => "unknown message kind",
                    ParseError::InvalidFields => "invalid fields or size out of range",
                    ParseError::BadMime => "type not allowed",
                    ParseError::BadLink => "link not allowed",
                };
                self.violation(&mut out, ContentError::BadMessage, detail);
                return out;
            }
        };
        match msg {
            Message::Image { name, mime, size } => {
                self.start(&mut out, Kind::Image(mime), name, size);
            }
            Message::Document { name, size } => self.start(&mut out, Kind::Document, name, size),
            Message::ImageEnd => self.end(&mut out, false),
            Message::DocumentEnd => self.end(&mut out, true),
            Message::Link { url } => out.push(ContentEvent::LinkRequested { url }),
            Message::Cmd(cmd) => self.command(&mut out, &cmd),
        }
        out
    }

    /// Handle one binary message (a chunk).
    pub fn handle_binary(&mut self, chunk: &[u8]) -> Vec<ContentEvent> {
        let mut out = Vec::new();
        if self.blocked {
            return out;
        }
        let Some(a) = self.active.as_mut() else {
            self.violations = self.violations.saturating_add(1);
            if !self.quiet_chunks {
                self.quiet_chunks = true;
                out.push(ContentEvent::Error {
                    error: ContentError::UnexpectedChunk,
                    detail: "data without a transfer".into(),
                });
            }
            self.check_blocked(&mut out);
            return out;
        };
        let end = a.buf.len().saturating_add(chunk.len());
        let verdict = if a.chunks + 1 > a.max_chunks || end > a.size {
            Some((ContentError::TooLarge, "more data than announced"))
        } else if chunk.is_empty() || (chunk.len() != CHUNK_SIZE && end != a.size) {
            Some((ContentError::BadChunk, "chunk is not 16 KiB"))
        } else {
            None
        };
        match verdict {
            Some((error, detail)) => {
                self.active = None;
                self.violation(&mut out, error, detail);
            }
            None => {
                // Capacity was reserved at the start, so this does not reallocate.
                a.buf.extend_from_slice(chunk);
                a.chunks += 1;
            }
        }
        out
    }

    fn start(&mut self, out: &mut Vec<ContentEvent>, kind: Kind, name: String, size: u64) {
        if self.active.take().is_some() {
            self.violation(
                out,
                ContentError::Interrupted,
                "a new transfer started before the previous one ended",
            );
            if self.blocked {
                return;
            }
        }
        let Ok(size_usize) = usize::try_from(size) else {
            self.violation(out, ContentError::TooLarge, "size does not fit");
            return;
        };
        let mut buf = Vec::new();
        if buf.try_reserve_exact(size_usize).is_err() {
            self.violation(out, ContentError::TooLarge, "cannot reserve memory");
            return;
        }
        out.push(ContentEvent::TransferStarted {
            document: kind == Kind::Document,
            name: name.clone(),
            size,
        });
        self.active = Some(Active {
            kind,
            name,
            size: size_usize,
            buf,
            chunks: 0,
            max_chunks: size.div_ceil(CHUNK_SIZE as u64),
        });
    }

    fn end(&mut self, out: &mut Vec<ContentEvent>, document: bool) {
        let Some(a) = self.active.take() else {
            self.violation(out, ContentError::NoTransfer, "end without a transfer");
            return;
        };
        if (a.kind == Kind::Document) != document {
            self.violation(out, ContentError::NoTransfer, "end message of the wrong kind");
            return;
        }
        if a.buf.len() != a.size {
            self.violation(out, ContentError::SizeMismatch, "fewer bytes than announced");
            return;
        }
        match a.kind {
            Kind::Image(mime) => {
                // A picture replaces a document that was shown.
                self.shown = None;
                out.push(ContentEvent::ImageReceived {
                    name: a.name,
                    mime,
                    bytes: Blob::new(a.buf),
                });
            }
            Kind::Document => self.open_document(out, a.name, a.buf),
        }
    }

    fn open_document(&mut self, out: &mut Vec<ContentEvent>, name: String, bytes: Vec<u8>) {
        // The old document goes away first, whatever happens to the new one.
        self.shown = None;
        let doc = match Document::open_with_options(
            bytes,
            self.opts.doc_limits,
            self.opts.isolation.clone(),
        ) {
            Ok(d) => d,
            Err(e) => {
                out.push(doc_error(ContentError::DocumentRejected, &e));
                return;
            }
        };
        let pages = u32::try_from(doc.page_count())
            .ok()
            .and_then(DocumentSession::new);
        let Some(pages) = pages else {
            out.push(doc_error(ContentError::DocumentRejected, &DocError::NoPages));
            return;
        };
        out.push(ContentEvent::DocumentOpened {
            name,
            pages: pages.page_count(),
        });
        self.shown = Some(Shown { doc, pages });
        self.render_current(out);
    }

    fn command(&mut self, out: &mut Vec<ContentEvent>, cmd: &ControlCmd) {
        if *cmd == ControlCmd::Stop {
            self.active = None;
            self.shown = None;
            out.push(ContentEvent::ContentStopped);
            return;
        }
        let Some(shown) = self.shown.as_mut() else {
            return; // page commands without a document are ignored (the web client does too)
        };
        if let DocEvent::PageChanged(_) = shown.pages.apply(cmd) {
            self.render_current(out);
        }
    }

    /// Set the width pages are rendered at; the shown page is rendered again if it changed.
    pub fn set_target_width(&mut self, width: u32) -> Vec<ContentEvent> {
        let mut out = Vec::new();
        let width = width.max(MIN_RENDER_WIDTH);
        if width != self.opts.target_width {
            self.opts.target_width = width;
            if self.shown.is_some() && !self.blocked {
                self.render_current(&mut out);
            }
        }
        out
    }

    fn render_current(&mut self, out: &mut Vec<ContentEvent>) {
        let Some(shown) = self.shown.as_ref() else {
            return;
        };
        let index = shown.pages.current_index();
        let page = shown.pages.current_page();
        match render(&shown.doc, index, self.opts.target_width, &self.opts.doc_limits) {
            Ok(p) => out.push(ContentEvent::PageRendered {
                page,
                width: p.width,
                height: p.height,
                rgba: Blob::new(p.rgba),
            }),
            Err(e) => out.push(doc_error(ContentError::RenderFailed, &e)),
        }
    }
}

fn doc_error(error: ContentError, e: &DocError) -> ContentEvent {
    ContentEvent::Error {
        error,
        detail: e.to_string(),
    }
}

/// Render a page at (about) `width` pixels, after checking that the result fits the output
/// limits. A page that is too tall or too large is scaled down to fit instead of refused.
fn render(
    doc: &Document,
    index: usize,
    width: u32,
    limits: &Limits,
) -> Result<cc_docs::RenderedPage, DocError> {
    let size = doc.page_size(index)?;
    if !size.is_valid() {
        return Err(DocError::InvalidPageSize);
    }
    let max_side = f64::from(limits.max_output_side.max(1));
    let max_pixels = limits.max_output_pixels.max(1) as f64;
    let mut w = f64::from(width.clamp(MIN_RENDER_WIDTH, limits.max_output_side.max(MIN_RENDER_WIDTH)));
    let aspect = f64::from(size.height_pt) / f64::from(size.width_pt);
    let mut h = (w * aspect).ceil();
    if h > max_side || w * h > max_pixels {
        let scale = (max_side / h).min((max_pixels / (w * h)).sqrt()).min(1.0);
        w = (w * scale).floor();
        h = (w * aspect).ceil();
    }
    if !w.is_finite() || !h.is_finite() || w < 1.0 || h < 1.0 || h > max_side || w * h > max_pixels
    {
        return Err(DocError::OutputTooLarge {
            width: w as u64,
            height: h as u64,
        });
    }
    doc.render_page(index, w as u32)
}

#[cfg(test)]
mod tests;
