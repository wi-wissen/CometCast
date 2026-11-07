// SPDX-License-Identifier: GPL-3.0-or-later
//! The receiver side: reassembles transfers from the control data channel, validates
//! everything and drives a PDF through `cc-docs`.
//!
//! [`ContentReceiver`] is a plain state machine: feed it each text and binary message of the
//! channel in order ([`ContentReceiver::feed_text`], [`ContentReceiver::feed_binary`]),
//! collect [`ContentEvent`]s. It does no network I/O and never panics on hostile input.
//!
//! The only things that can take long are opening a document and rendering a page (both
//! happen in the sandboxed render process of `cc-docs`, up to its time limits). Feeding never
//! blocks: it leaves such jobs as [`Work`] for [`ContentReceiver::next_action`]; the caller
//! runs [`Work::run`] on a thread that may block and hands the result to
//! [`ContentReceiver::finish_work`] (`cc-receiver` does). While a job runs, messages keep
//! coming in, so:
//!
//! * a flood of page commands is merged into one render of the last page asked for, and
//!   renders keep a minimum interval;
//! * `stop`, a new document and a new image cancel a running render at once (a render that is
//!   slow and stale is cancelled too); a document open cannot be interrupted, its result is
//!   dropped when it returns;
//! * [`ContentReceiver::handle_text`] is the simple form that does it all in the calling thread.
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
//!   Violations decay (one per 2 s by default), so a long session is not blocked by rare slips.
//!   A **refused** transfer start (too large, type not allowed) is one violation; the data
//!   that follows is swallowed silently up to the announced size, then its end message.
//! * Opening a document is budgeted: a failed open is a violation, and opens are rate limited
//!   (burst of 5, one more per 3 s), because each costs a child process.
//! * A transfer that stalls for 60 s is dropped with an event ([`ContentReceiver::tick`]).
//! * Links are validated again here and **never opened**; the event only asks.
//! * A PDF is only handed to `cc-docs`, which checks it again, renders in a child process
//!   and treats it as hostile. A page is only rendered after its size was checked against
//!   the output limits.

use std::collections::VecDeque;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use cc_docs::{
    DocError, Document, DocumentSession, Isolation, Limits, RenderOptions, SessionEvent as DocEvent,
};
use cc_proto::ControlCmd;

use crate::wire::{
    CHUNK_SIZE, ImageMime, MAX_DOCUMENT_BYTES, MAX_IMAGE_BYTES, Message, ParseError, TransferKind,
    announced_transfer, parse_message,
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
    /// A transfer received nothing for too long and was dropped (not a violation).
    TransferTimeout,
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
            Self::TransferTimeout => "transfer_timeout",
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
    /// Bytes of the running transfer received so far. At most one event per whole percent
    /// and four per second, plus one when the transfer is complete; between
    /// [`ContentEvent::TransferStarted`] and the image or document that follows.
    TransferProgress {
        /// Bytes received.
        received: u64,
        /// Announced size in bytes.
        total: u64,
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
    /// Shortest time between the starts of two page renders (default 100 ms). Page commands
    /// that arrive in between are merged: only the last page asked for is rendered.
    pub min_render_interval: Duration,
    /// A running render is cancelled when a newer page is asked for and it has run longer
    /// than this (default 1 s). Faster renders are allowed to finish, so a steady stream of
    /// commands still shows pictures. `stop`, a new document and a new image always cancel at once.
    pub supersede_after: Duration,
    /// A transfer that receives nothing for this long is dropped (default 60 s, zero = never).
    pub transfer_idle_timeout: Duration,
    /// One protocol violation is forgotten per this much time (default 2 s, zero = never).
    pub violation_decay: Duration,
    /// Documents that may be opened in a burst (default 5; the bucket refills one per
    /// [`Self::open_refill`]). Opening costs a child process and up to the open time limit.
    pub open_burst: u32,
    /// Time to refill one document open (default 3 s, zero = no limit on opens).
    pub open_refill: Duration,
    /// Report [`ContentEvent::TransferProgress`] (default `false`: only the apps that show a
    /// progress want the extra events).
    pub progress: bool,
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
            min_render_interval: Duration::from_millis(100),
            supersede_after: Duration::from_secs(1),
            transfer_idle_timeout: Duration::from_secs(60),
            violation_decay: Duration::from_secs(2),
            open_burst: 5,
            open_refill: Duration::from_secs(3),
            progress: false,
        }
    }
}

/// The data of a refused transfer that is swallowed silently is bounded by what the sender
/// announced, but at most this much; a bigger announcement is not believed and its chunks
/// count as violations as before.
/// UNCERTAIN: placeholder, unmeasured.
pub const MAX_SKIPPED_BYTES: u64 = 1024 * 1024 * 1024;

/// Page commands remembered while a document opens (the oldest are dropped past this).
const MAX_QUEUED_COMMANDS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Image(ImageMime),
    Document,
}

/// Progress events leave at most this often (four per second).
const PROGRESS_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

struct Active {
    kind: Kind,
    name: String,
    size: usize,
    buf: Vec<u8>,
    chunks: u64,
    max_chunks: u64,
    last_activity: Instant,
    /// When and at which whole percent the last progress event went out.
    last_progress: Option<(Instant, u64)>,
}

/// The data of a refused transfer, which is dropped without further violations.
struct Skipping {
    document: bool,
    chunks_left: u64,
    last_activity: Instant,
}

/// A document that is shown.
struct Shown {
    doc: Arc<Document>,
    pages: DocumentSession,
}

/// Bookkeeping for the one blocking job that may run at a time.
struct Inflight {
    id: u64,
    is_open: bool,
    started: Instant,
    cancel: Arc<AtomicBool>,
}

/// What [`ContentReceiver::next_action`] decides. (A value is matched at once; boxing the job would only add an allocation.)
#[allow(clippy::large_enum_variant)]
pub enum Next {
    /// Run this on a thread that may block ([`Work::run`]) and give the result to
    /// [`ContentReceiver::finish_work`].
    Work(Work),
    /// Ask again at this time (the minimum render interval has not passed).
    Wait(Instant),
    /// Nothing to do until the next message.
    Idle,
}

// One value lives for the duration of one job and is moved once; boxing would only add an allocation.
#[allow(clippy::large_enum_variant)]
enum Job {
    Open {
        name: String,
        bytes: Vec<u8>,
        limits: Limits,
        isolation: Isolation,
    },
    Render {
        doc: Arc<Document>,
        index: usize,
        page: u32,
        width: u32,
        limits: Limits,
        cancel: Arc<AtomicBool>,
    },
}

/// A job that blocks: opening a PDF or rendering a page (both wait for the render process of
/// `cc-docs`). It owns everything it needs, so the receiver itself stays available for
/// messages (and for `stop`) while it runs.
pub struct Work {
    id: u64,
    epoch: u64,
    job: Job,
}

// Same as `Job`: one short-lived value per job.
#[allow(clippy::large_enum_variant)]
enum Done {
    Opened {
        name: String,
        result: Result<Document, DocError>,
    },
    Rendered {
        page: u32,
        result: Result<cc_docs::RenderedPage, DocError>,
    },
}

/// The result of a [`Work`].
pub struct WorkDone {
    id: u64,
    epoch: u64,
    done: Done,
}

impl Work {
    /// Identifies the job, for [`ContentReceiver::work_panicked`].
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Run the job. Blocks for as long as the render process needs (bounded by the limits of
    /// `cc-docs`, or shorter when cancelled).
    pub fn run(self) -> WorkDone {
        let done = match self.job {
            Job::Open {
                name,
                bytes,
                limits,
                isolation,
            } => Done::Opened {
                name,
                result: Document::open_with_options(bytes, limits, isolation),
            },
            Job::Render {
                doc,
                index,
                page,
                width,
                limits,
                cancel,
            } => Done::Rendered {
                page,
                result: render(&doc, index, width, &limits, cancel),
            },
        };
        WorkDone {
            id: self.id,
            epoch: self.epoch,
            done,
        }
    }
}

/// The receiver state machine. See the module docs.
pub struct ContentReceiver {
    opts: ContentOptions,
    active: Option<Active>,
    skipping: Option<Skipping>,
    shown: Option<Shown>,
    violations: u32,
    blocked: bool,
    /// After an `unexpected_chunk` event, further ones are counted but not reported until
    /// the next text message (a flood must not flood the event queue).
    quiet_chunks: bool,
    last_decay: Instant,
    /// Document that is complete and waits for its open.
    open_pending: Option<(String, Vec<u8>)>,
    /// Page commands that came in while the new document was opening; applied to it when it is
    /// shown (the sender cannot know how long an open takes). At most [`MAX_QUEUED_COMMANDS`].
    queued_cmds: VecDeque<ControlCmd>,
    /// The shown page (or width) differs from the last picture asked for.
    render_wanted: bool,
    last_render_start: Option<Instant>,
    inflight: Option<Inflight>,
    next_job: u64,
    /// Bumped whenever the content is replaced or stopped; results of older jobs are dropped.
    epoch: u64,
    open_tokens: f64,
    open_last: Instant,
}

impl ContentReceiver {
    /// A receiver with the given options.
    pub fn new(opts: ContentOptions) -> Self {
        let now = Instant::now();
        let open_tokens = f64::from(opts.open_burst);
        Self {
            opts,
            active: None,
            skipping: None,
            shown: None,
            violations: 0,
            blocked: false,
            quiet_chunks: false,
            last_decay: now,
            open_pending: None,
            queued_cmds: VecDeque::new(),
            render_wanted: false,
            last_render_start: None,
            inflight: None,
            next_job: 0,
            epoch: 0,
            open_tokens,
            open_last: now,
        }
    }

    /// `true` after too many violations: all input is ignored and the owner should close the channel.
    pub fn is_blocked(&self) -> bool {
        self.blocked
    }

    /// Protocol violations counted now (they decay over time, see
    /// [`ContentOptions::violation_decay`]).
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
            self.skipping = None;
            self.drop_document();
            out.push(ContentEvent::Error {
                error: ContentError::TooManyErrors,
                detail: "too many protocol violations; the channel is ignored from now on".into(),
            });
        }
    }

    /// Forget one violation per [`ContentOptions::violation_decay`] that has passed.
    fn decay(&mut self, now: Instant) {
        let step = self.opts.violation_decay;
        if self.violations == 0 || step.is_zero() {
            self.last_decay = now;
            return;
        }
        let steps = now.saturating_duration_since(self.last_decay).as_nanos() / step.as_nanos();
        if steps > 0 {
            let steps32 = u32::try_from(steps).unwrap_or(u32::MAX);
            self.violations = self.violations.saturating_sub(steps32);
            self.last_decay += step.saturating_mul(steps32);
        }
    }

    /// Drop the shown document and everything on its way to the screen: a document waiting
    /// to open, a running job (cancelled; its result is dropped when it comes back).
    fn drop_document(&mut self) {
        self.shown = None;
        self.open_pending = None;
        self.queued_cmds.clear();
        self.render_wanted = false;
        self.epoch = self.epoch.wrapping_add(1);
        if let Some(i) = &self.inflight {
            i.cancel.store(true, Ordering::SeqCst);
        }
    }

    /// The page to show changed: render again, and cancel a render that is slow and now stale.
    fn page_wanted(&mut self) {
        self.render_wanted = true;
        if let Some(i) = &self.inflight
            && !i.is_open
            && i.started.elapsed() > self.opts.supersede_after
        {
            i.cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Start swallowing the data of a refused transfer.
    fn begin_skip(&mut self, document: bool, size: u64, now: Instant) {
        if self.blocked || size > MAX_SKIPPED_BYTES {
            return;
        }
        self.skipping = Some(Skipping {
            document,
            chunks_left: size.div_ceil(CHUNK_SIZE as u64),
            last_activity: now,
        });
    }

    /// Handle one text message and run the work it causes (blocks). For callers without an
    /// event loop; the `cc-receiver` task uses [`Self::feed_text`] and [`Self::next_action`].
    pub fn handle_text(&mut self, text: &str) -> Vec<ContentEvent> {
        let mut out = self.feed_text(text);
        self.run_pending(&mut out);
        out
    }

    /// Handle one binary message (a chunk).
    pub fn handle_binary(&mut self, chunk: &[u8]) -> Vec<ContentEvent> {
        self.feed_binary(chunk)
    }

    /// Set the width pages are rendered at and render the shown page again (blocks). See
    /// [`Self::feed_target_width`] for the non-blocking form.
    pub fn set_target_width(&mut self, width: u32) -> Vec<ContentEvent> {
        self.feed_target_width(width);
        let mut out = Vec::new();
        self.run_pending(&mut out);
        out
    }

    /// Run all work now, ignoring the render interval.
    fn run_pending(&mut self, out: &mut Vec<ContentEvent>) {
        while let Next::Work(w) = self.next_action(Instant::now(), false) {
            let done = w.run();
            out.extend(self.finish_work(done));
        }
    }

    /// Feed one text message. Never blocks: opening a document and rendering are left as
    /// work for [`Self::next_action`]. Page commands only change the wanted page, so a flood
    /// of them costs one render.
    pub fn feed_text(&mut self, text: &str) -> Vec<ContentEvent> {
        let mut out = Vec::new();
        if self.blocked {
            return out;
        }
        let now = Instant::now();
        self.decay(now);
        self.quiet_chunks = false;
        // Any text message ends the skipping of a refused transfer; only its own end
        // message is swallowed without a word.
        let skipping = self.skipping.take();
        let msg = parse_message(
            text,
            self.opts.max_image_bytes,
            self.opts
                .max_document_bytes
                .min(self.opts.doc_limits.max_file_bytes as u64),
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
                let announced = announced_transfer(text);
                let document = announced.is_some_and(|(k, _)| k == TransferKind::Document);
                if document {
                    // The old document must not stay on screen as if nothing happened.
                    self.drop_document();
                }
                let error = if document {
                    ContentError::DocumentRejected
                } else {
                    ContentError::BadMessage
                };
                self.violation(&mut out, error, detail);
                // One violation for the refused transfer; its data is swallowed.
                if let Some((_, size)) = announced {
                    self.begin_skip(document, size, now);
                }
                return out;
            }
        };
        match msg {
            Message::Image { name, mime, size } => {
                self.start(&mut out, Kind::Image(mime), name, size, now);
            }
            Message::Document { name, size } => {
                self.start(&mut out, Kind::Document, name, size, now);
            }
            Message::ImageEnd | Message::DocumentEnd => {
                let document = msg == Message::DocumentEnd;
                if skipping.is_some_and(|s| s.document == document) {
                    return out; // the end of the refused transfer
                }
                self.end(&mut out, document, now);
            }
            Message::Link { url } => out.push(ContentEvent::LinkRequested { url }),
            // Display to presenter only: ignored here, and not a violation.
            Message::Page { .. } => self.skipping = skipping,
            Message::Cmd(cmd) => self.command(&mut out, &cmd),
        }
        out
    }

    /// Feed one binary message (a chunk).
    pub fn feed_binary(&mut self, chunk: &[u8]) -> Vec<ContentEvent> {
        let mut out = Vec::new();
        if self.blocked {
            return out;
        }
        let now = Instant::now();
        self.decay(now);
        if let Some(s) = self.skipping.as_mut() {
            if s.chunks_left == 0 {
                self.skipping = None;
                self.violation(&mut out, ContentError::TooLarge, "more data than announced");
            } else {
                s.chunks_left -= 1;
                s.last_activity = now;
            }
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
                a.last_activity = now;
                let (got, total) = (a.buf.len() as u64, a.size as u64);
                let pct = if total == 0 { 100 } else { got * 100 / total };
                let due = a.last_progress.is_none_or(|(t, p)| {
                    pct > p && now.saturating_duration_since(t) >= PROGRESS_INTERVAL
                });
                if self.opts.progress
                    && (due || (got == total && a.last_progress.is_none_or(|(_, p)| p < 100)))
                {
                    a.last_progress = Some((now, pct));
                    out.push(ContentEvent::TransferProgress {
                        received: got,
                        total,
                    });
                }
            }
        }
        out
    }

    /// Set the width pages are rendered at; the shown page is rendered again (as work).
    pub fn feed_target_width(&mut self, width: u32) {
        let width = width.max(MIN_RENDER_WIDTH);
        if width != self.opts.target_width {
            self.opts.target_width = width;
            if self.shown.is_some() && !self.blocked {
                self.page_wanted();
            }
        }
    }

    fn start(
        &mut self,
        out: &mut Vec<ContentEvent>,
        kind: Kind,
        name: String,
        size: u64,
        now: Instant,
    ) {
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
        let document = kind == Kind::Document;
        let Ok(size_usize) = usize::try_from(size) else {
            self.violation(out, ContentError::TooLarge, "size does not fit");
            self.begin_skip(document, size, now);
            return;
        };
        let mut buf = Vec::new();
        if buf.try_reserve_exact(size_usize).is_err() {
            self.violation(out, ContentError::TooLarge, "cannot reserve memory");
            self.begin_skip(document, size, now);
            return;
        }
        out.push(ContentEvent::TransferStarted {
            document,
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
            last_activity: now,
            last_progress: None,
        });
    }

    fn end(&mut self, out: &mut Vec<ContentEvent>, document: bool, now: Instant) {
        let Some(a) = self.active.take() else {
            self.violation(out, ContentError::NoTransfer, "end without a transfer");
            return;
        };
        if (a.kind == Kind::Document) != document {
            self.violation(
                out,
                ContentError::NoTransfer,
                "end message of the wrong kind",
            );
            return;
        }
        if a.buf.len() != a.size {
            self.violation(
                out,
                ContentError::SizeMismatch,
                "fewer bytes than announced",
            );
            return;
        }
        match a.kind {
            Kind::Image(mime) => {
                // A picture replaces a document that was shown.
                self.drop_document();
                out.push(ContentEvent::ImageReceived {
                    name: a.name,
                    mime,
                    bytes: Blob::new(a.buf),
                });
            }
            Kind::Document => self.queue_document(out, a.name, a.buf, now),
        }
    }

    /// Take a token from the open budget.
    fn open_allowed(&mut self, now: Instant) -> bool {
        let refill = self.opts.open_refill;
        if refill.is_zero() {
            return true;
        }
        let burst = f64::from(self.opts.open_burst);
        let gained =
            now.saturating_duration_since(self.open_last).as_secs_f64() / refill.as_secs_f64();
        self.open_last = now;
        self.open_tokens = (self.open_tokens + gained).min(burst);
        if self.open_tokens >= 1.0 {
            self.open_tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn queue_document(
        &mut self,
        out: &mut Vec<ContentEvent>,
        name: String,
        bytes: Vec<u8>,
        now: Instant,
    ) {
        // The old document goes away first, whatever happens to the new one.
        self.drop_document();
        if !self.open_allowed(now) {
            self.violation(
                out,
                ContentError::DocumentRejected,
                "too many documents in a short time",
            );
            return;
        }
        self.open_pending = Some((name, bytes));
    }

    fn command(&mut self, out: &mut Vec<ContentEvent>, cmd: &ControlCmd) {
        if *cmd == ControlCmd::Stop {
            self.active = None;
            self.drop_document();
            out.push(ContentEvent::ContentStopped);
            return;
        }
        let Some(shown) = self.shown.as_mut() else {
            // While the document is opening the commands wait for it; without any document
            // they are ignored (the web client does too).
            if self.open_pending.is_some() || self.inflight.as_ref().is_some_and(|i| i.is_open) {
                if self.queued_cmds.len() >= MAX_QUEUED_COMMANDS {
                    self.queued_cmds.pop_front();
                }
                self.queued_cmds.push_back(cmd.clone());
            }
            return;
        };
        if let DocEvent::PageChanged(_) = shown.pages.apply(cmd) {
            self.page_wanted();
        }
    }

    /// Decide what to do next, if the one job that may run at a time is not running. With
    /// `pace` set, renders keep [`ContentOptions::min_render_interval`] between their starts.
    pub fn next_action(&mut self, now: Instant, pace: bool) -> Next {
        if self.blocked || self.inflight.is_some() {
            return Next::Idle;
        }
        if let Some((name, bytes)) = self.open_pending.take() {
            let job = Job::Open {
                name,
                bytes,
                limits: self.opts.doc_limits,
                isolation: self.opts.isolation.clone(),
            };
            return Next::Work(self.begin(job, true, now, Arc::new(AtomicBool::new(false))));
        }
        if self.render_wanted
            && let Some(shown) = self.shown.as_ref()
        {
            if pace
                && let Some(t) = self.last_render_start
                && now < t + self.opts.min_render_interval
            {
                return Next::Wait(t + self.opts.min_render_interval);
            }
            let cancel = Arc::new(AtomicBool::new(false));
            let job = Job::Render {
                doc: Arc::clone(&shown.doc),
                index: shown.pages.current_index(),
                page: shown.pages.current_page(),
                width: self.opts.target_width,
                limits: self.opts.doc_limits,
                cancel: Arc::clone(&cancel),
            };
            self.render_wanted = false;
            self.last_render_start = Some(now);
            return Next::Work(self.begin(job, false, now, cancel));
        }
        Next::Idle
    }

    fn begin(&mut self, job: Job, is_open: bool, now: Instant, cancel: Arc<AtomicBool>) -> Work {
        self.next_job += 1;
        self.inflight = Some(Inflight {
            id: self.next_job,
            is_open,
            started: now,
            cancel,
        });
        Work {
            id: self.next_job,
            epoch: self.epoch,
            job,
        }
    }

    /// Apply the result of a [`Work`]. A result for content that was stopped or replaced in
    /// the meantime is dropped.
    pub fn finish_work(&mut self, done: WorkDone) -> Vec<ContentEvent> {
        let mut out = Vec::new();
        if self.inflight.as_ref().is_some_and(|i| i.id == done.id) {
            self.inflight = None;
        }
        if self.blocked || done.epoch != self.epoch {
            return out;
        }
        match done.done {
            Done::Opened { name, result } => match result {
                Ok(doc) => {
                    let pages = u32::try_from(doc.page_count())
                        .ok()
                        .and_then(DocumentSession::new);
                    let Some(pages) = pages else {
                        let e = DocError::NoPages;
                        self.violation(&mut out, ContentError::DocumentRejected, &e.to_string());
                        return out;
                    };
                    out.push(ContentEvent::DocumentOpened {
                        name,
                        pages: pages.page_count(),
                    });
                    let mut pages = pages;
                    for c in self.queued_cmds.drain(..) {
                        pages.apply(&c);
                    }
                    self.shown = Some(Shown {
                        doc: Arc::new(doc),
                        pages,
                    });
                    self.render_wanted = true;
                }
                // A document that cannot be opened counts against the violation budget:
                // every open costs a child process.
                Err(e) => self.violation(&mut out, ContentError::DocumentRejected, &e.to_string()),
            },
            Done::Rendered { page, result } => match result {
                Ok(p) => out.push(ContentEvent::PageRendered {
                    page,
                    width: p.width,
                    height: p.height,
                    rgba: Blob::new(p.rgba),
                }),
                Err(DocError::Cancelled) => {}
                Err(e) => out.push(doc_error(ContentError::RenderFailed, &e)),
            },
        }
        out
    }

    /// The thread that ran a [`Work`] panicked (it should not). The receiver keeps its state
    /// (violations, blocked); the job is reported as failed.
    pub fn work_panicked(&mut self, id: u64) -> Vec<ContentEvent> {
        let Some(i) = self.inflight.take_if(|i| i.id == id) else {
            return Vec::new();
        };
        let error = if i.is_open {
            ContentError::DocumentRejected
        } else {
            ContentError::RenderFailed
        };
        vec![ContentEvent::Error {
            error,
            detail: "internal error while handling content".into(),
        }]
    }

    /// When [`Self::tick`] next has something to do (the idle timeout of a transfer).
    pub fn idle_deadline(&self) -> Option<Instant> {
        let t = self.opts.transfer_idle_timeout;
        if t.is_zero() {
            return None;
        }
        let a = self.active.as_ref().map(|a| a.last_activity);
        let s = self.skipping.as_ref().map(|s| s.last_activity);
        a.into_iter().chain(s).min().map(|at| at + t)
    }

    /// Drop a transfer that stalled (nothing received for
    /// [`ContentOptions::transfer_idle_timeout`]) and say so. Not a violation: the network
    /// can stall.
    pub fn tick(&mut self, now: Instant) -> Vec<ContentEvent> {
        let mut out = Vec::new();
        let t = self.opts.transfer_idle_timeout;
        if t.is_zero() {
            return out;
        }
        if self
            .skipping
            .as_ref()
            .is_some_and(|s| now.saturating_duration_since(s.last_activity) >= t)
        {
            self.skipping = None;
        }
        if self
            .active
            .as_ref()
            .is_some_and(|a| now.saturating_duration_since(a.last_activity) >= t)
        {
            self.active = None;
            out.push(ContentEvent::Error {
                error: ContentError::TransferTimeout,
                detail: "the transfer stalled and was dropped".into(),
            });
        }
        out
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
    cancel: Arc<AtomicBool>,
) -> Result<cc_docs::RenderedPage, DocError> {
    let size = doc.page_size(index)?;
    if !size.is_valid() {
        return Err(DocError::InvalidPageSize);
    }
    let max_side = f64::from(limits.max_output_side.max(1));
    let max_pixels = limits.max_output_pixels.max(1) as f64;
    let mut w = f64::from(width.clamp(
        MIN_RENDER_WIDTH,
        limits.max_output_side.max(MIN_RENDER_WIDTH),
    ));
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
    doc.render_page_with(
        index,
        w as u32,
        &RenderOptions {
            timeout: None,
            cancel: Some(cancel),
        },
    )
}

#[cfg(test)]
mod tests;
