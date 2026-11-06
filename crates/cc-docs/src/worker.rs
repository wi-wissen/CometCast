// SPDX-License-Identifier: GPL-3.0-or-later
//! The in-process engine that talks to hayro, and the functions the render worker process
//! shares with it.
//!
//! This is the **weak** engine (see `docs/documents.md`): it is used on `wasm32`, on iOS and
//! tvOS (no child processes) and when the caller asks for `Isolation::InProcess`. The
//! default on every other platform is the child process of `process.rs`.
//!
//! Threads: one worker thread per open document owns the parsed `Pdf` and the render
//! cache; callers send jobs and wait with a time limit. hayro has no hook to interrupt a
//! running page, so when the limit passes (or the caller cancels) the thread is **abandoned**:
//! the caller gets an error at once and the thread runs on until hayro returns. A document
//! whose render timed out is **poisoned** (never rendered again, also not after reopening the
//! same bytes), so one bad file costs one thread. Memory is not bounded in this engine.
//!
//! `wasm32`: no threads and no usable `catch_unwind` (panic aborts there), so the engine
//! parses and renders inline, without time limit or cancel. The hard limits that do not need
//! threads (file size, page count, output size) and the pre-scan still apply.

use std::sync::Arc;

use hayro::hayro_interpret::util::TransformExt;
use hayro::hayro_interpret::{InterpreterSettings, hayro_syntax};
use hayro::hayro_syntax::{LoadPdfError, Pdf};
use hayro::kurbo::Affine;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::vello_cpu::{Pixmap, RasterizerSettings, RenderContext, Resources, TargetInit};
use hayro::{RenderCache, RenderSettings, render_into};

use crate::document::{Limits, PageSize, RenderedPage};
use crate::error::DocError;
use crate::prescan::ScanLevel;

/// A validated render request: the output size is already checked against the limits.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Job {
    pub index: usize,
    pub width: u32,
    pub height: u32,
}

/// Parse the bytes, run the pre-scan and collect the page sizes. Runs on the worker thread
/// (or inline on wasm, or in the worker process).
pub(crate) fn parse(
    bytes: &Arc<Vec<u8>>,
    limits: &Limits,
    level: ScanLevel,
) -> Result<(Pdf, Vec<PageSize>), DocError> {
    let pdf = Pdf::new(Arc::clone(bytes)).map_err(|e| match e {
        LoadPdfError::Decryption(d) => match d {
            hayro_syntax::DecryptionError::PasswordProtected => DocError::PasswordProtected,
            hayro_syntax::DecryptionError::MissingIDEntry => {
                DocError::UnsupportedEncryption("missing ID entry")
            }
            hayro_syntax::DecryptionError::InvalidEncryption => {
                DocError::UnsupportedEncryption("invalid encryption dictionary")
            }
            hayro_syntax::DecryptionError::UnsupportedAlgorithm => {
                DocError::UnsupportedEncryption("unsupported algorithm")
            }
        },
        LoadPdfError::Invalid => DocError::Invalid,
    })?;
    crate::prescan::scan(&pdf, limits, level)?;
    let pages = pdf.pages();
    if pages.is_empty() {
        return Err(DocError::NoPages);
    }
    if pages.len() > limits.max_pages {
        return Err(DocError::TooManyPages {
            count: pages.len(),
            max: limits.max_pages,
        });
    }
    let sizes = pages
        .iter()
        .map(|p| {
            let (w, h) = p.render_dimensions();
            PageSize {
                width_pt: w,
                height_pt: h,
            }
        })
        .collect();
    Ok((pdf, sizes))
}

/// Render one page at exactly `job.width` x `job.height` pixels on white.
pub(crate) fn render_one<'a>(
    pdf: &'a Pdf,
    cache: &RenderCache<'a>,
    settings: &InterpreterSettings,
    job: Job,
) -> Result<RenderedPage, DocError> {
    let page = pdf.pages().get(job.index).ok_or(DocError::PageOutOfRange {
        index: job.index,
        count: pdf.pages().len(),
    })?;
    let (pw, ph) = page.render_dimensions();
    if !(pw.is_finite() && ph.is_finite() && pw > 0.0 && ph > 0.0) {
        return Err(DocError::InvalidPageSize);
    }
    // The caller keeps both sides <= u16::MAX (hayro's context takes u16).
    let (w16, h16) = (
        u16::try_from(job.width).map_err(|_| DocError::Internal("width over u16"))?,
        u16::try_from(job.height).map_err(|_| DocError::Internal("height over u16"))?,
    );
    let mut ctx = RenderContext::new(w16, h16);
    let transform = Affine::scale_non_uniform(
        f64::from(job.width) / f64::from(pw),
        f64::from(job.height) / f64::from(ph),
    ) * page.initial_transform(true).to_kurbo();
    render_into(
        page,
        cache,
        settings,
        &RenderSettings::default(),
        &mut ctx,
        transform,
    );
    ctx.flush();
    let mut pixmap = Pixmap::new(w16, h16);
    ctx.render_with(
        &mut pixmap,
        &mut Resources::default(),
        RasterizerSettings {
            target_init: TargetInit::Clear(WHITE),
            ..Default::default()
        },
    );
    Ok(RenderedPage {
        width: job.width,
        height: job.height,
        rgba: pixmap.data_as_u8_slice().to_vec(),
    })
}

#[cfg(target_arch = "wasm32")]
pub(crate) use wasm::{Engine, content_key, is_poisoned, poison, stuck_workers};

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
    use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use super::{Job, ScanLevel, parse, render_one};
    use crate::document::{Limits, PageSize, RenderedPage};
    use crate::error::DocError;

    /// Renders that were abandoned (time limit, cancel) and still run inside hayro.
    static STUCK: AtomicUsize = AtomicUsize::new(0);
    /// Do not start new workers while this many abandoned ones are still running.
    /// Poisoning (see the module docs) means a single file can hold only one of them.
    // UNCERTAIN: 8 is a guess; it only bounds the CPU and memory hostile files can keep busy.
    const MAX_STUCK: usize = 8;
    /// Stack of a worker thread. Parsing and rendering recurse (nested objects, forms,
    /// patterns); a stack overflow cannot be caught and would end the whole process.
    // UNCERTAIN: 64 MiB is generous but not derived from hayro's maximum recursion depth.
    const WORKER_STACK: usize = 64 * 1024 * 1024;
    /// Content hashes of files whose open or render timed out (bounded, oldest dropped).
    static POISONED: Mutex<Vec<u64>> = Mutex::new(Vec::new());
    const MAX_POISONED: usize = 64;

    const RUNNING: u8 = 0;
    const ABANDONED: u8 = 1;
    const DONE: u8 = 2;

    /// Shared between the engine and its thread; makes the STUCK count exact whatever the
    /// order of "abandon" and "thread ended".
    struct Slot(AtomicU8);

    impl Slot {
        /// The caller gives up on the thread.
        fn abandon(&self) {
            if self
                .0
                .compare_exchange(RUNNING, ABANDONED, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                STUCK.fetch_add(1, Ordering::SeqCst);
            }
        }
        fn is_abandoned(&self) -> bool {
            self.0.load(Ordering::SeqCst) == ABANDONED
        }
    }

    /// Dropped when the thread ends, also after a panic.
    struct ThreadEnd(Arc<Slot>);

    impl Drop for ThreadEnd {
        fn drop(&mut self) {
            if self.0.0.swap(DONE, Ordering::SeqCst) == ABANDONED {
                STUCK.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    enum Reply {
        Info(Result<Vec<PageSize>, DocError>),
        Page(Result<RenderedPage, DocError>),
    }

    /// Number of abandoned renders still running (for tests and diagnostics).
    pub(crate) fn stuck_workers() -> usize {
        STUCK.load(Ordering::SeqCst)
    }

    /// A hash of the file content (random per process).
    pub(crate) fn content_key(bytes: &[u8]) -> u64 {
        use std::hash::BuildHasher;
        use std::sync::OnceLock;
        static STATE: OnceLock<std::collections::hash_map::RandomState> = OnceLock::new();
        STATE.get_or_init(Default::default).hash_one(bytes)
    }

    pub(crate) fn is_poisoned(key: u64) -> bool {
        POISONED
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(&key)
    }

    pub(crate) fn poison(key: u64) {
        let mut v = POISONED.lock().unwrap_or_else(|p| p.into_inner());
        if !v.contains(&key) {
            if v.len() >= MAX_POISONED {
                v.remove(0);
            }
            v.push(key);
        }
    }

    pub(crate) struct Engine {
        tx: Option<Sender<Job>>,
        rx: Receiver<Reply>,
        slot: Arc<Slot>,
    }

    impl Drop for Engine {
        fn drop(&mut self) {
            // Dropping the sender ends the thread loop once the current job returns.
            self.tx = None;
        }
    }

    impl Engine {
        /// Start the worker, parse on it and wait for the page sizes. A timeout here poisons
        /// the content (`key`).
        pub(crate) fn open(
            bytes: Arc<Vec<u8>>,
            limits: Limits,
            key: u64,
        ) -> Result<(Engine, Vec<PageSize>), DocError> {
            if is_poisoned(key) {
                return Err(DocError::Poisoned);
            }
            if stuck_workers() >= MAX_STUCK {
                return Err(DocError::Busy);
            }
            let (job_tx, job_rx) = channel::<Job>();
            let (reply_tx, reply_rx) = channel::<Reply>();
            let slot = Arc::new(Slot(AtomicU8::new(RUNNING)));
            let end = ThreadEnd(Arc::clone(&slot));
            std::thread::Builder::new()
                .name("cc-docs-worker".into())
                .stack_size(WORKER_STACK)
                .spawn(move || {
                    let _end = end;
                    serve(&bytes, &limits, &job_rx, &reply_tx);
                })
                .map_err(|_| DocError::Internal("cannot start worker thread"))?;
            let mut engine = Engine {
                tx: Some(job_tx),
                rx: reply_rx,
                slot,
            };
            let r = engine.wait(limits.open_timeout, None);
            if matches!(r, Err(DocError::Timeout)) {
                poison(key);
            }
            match r? {
                Reply::Info(r) => r.map(|sizes| (engine, sizes)),
                Reply::Page(_) => Err(DocError::Internal("unexpected reply")),
            }
        }

        /// Render one page with a time limit; `cancel` is polled every few milliseconds.
        pub(crate) fn render(
            &mut self,
            job: Job,
            timeout: Duration,
            cancel: Option<&AtomicBool>,
        ) -> Result<RenderedPage, DocError> {
            let tx = self
                .tx
                .as_ref()
                .ok_or(DocError::Internal("worker is gone"))?;
            tx.send(job)
                .map_err(|_| DocError::Internal("worker is gone"))?;
            match self.wait(timeout, cancel)? {
                Reply::Page(r) => r,
                Reply::Info(_) => Err(DocError::Internal("unexpected reply")),
            }
        }

        /// Is the worker usable for another job?
        pub(crate) fn alive(&self) -> bool {
            self.tx.is_some() && !self.slot.is_abandoned()
        }

        fn wait(
            &mut self,
            timeout: Duration,
            cancel: Option<&AtomicBool>,
        ) -> Result<Reply, DocError> {
            let deadline = crate::clamp::deadline(timeout);
            loop {
                if cancel.is_some_and(|c| c.load(Ordering::SeqCst)) {
                    self.abandon();
                    return Err(DocError::Cancelled);
                }
                let now = Instant::now();
                if now >= deadline {
                    self.abandon();
                    return Err(DocError::Timeout);
                }
                let slice = (deadline - now).min(Duration::from_millis(10));
                match self.rx.recv_timeout(slice) {
                    Ok(reply) => {
                        // A panic in the worker ends the thread; the engine is not reused.
                        if matches!(
                            &reply,
                            Reply::Info(Err(DocError::Panicked))
                                | Reply::Page(Err(DocError::Panicked))
                        ) {
                            self.tx = None;
                        }
                        return Ok(reply);
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => {
                        self.tx = None;
                        return Err(DocError::Internal("worker ended unexpectedly"));
                    }
                }
            }
        }

        fn abandon(&mut self) {
            self.slot.abandon();
            self.tx = None;
        }
    }

    fn panic_to_error<T>(r: std::thread::Result<Result<T, DocError>>) -> Result<T, DocError> {
        r.unwrap_or(Err(DocError::Panicked))
    }

    /// The worker loop: parse once, then answer jobs until the sender is dropped.
    fn serve(bytes: &Arc<Vec<u8>>, limits: &Limits, jobs: &Receiver<Job>, replies: &Sender<Reply>) {
        let parsed = panic_to_error(catch_unwind(AssertUnwindSafe(|| {
            parse(bytes, limits, ScanLevel::Full)
        })));
        let pdf = match parsed {
            Ok((pdf, sizes)) => {
                let _ = replies.send(Reply::Info(Ok(sizes)));
                pdf
            }
            Err(e) => {
                let _ = replies.send(Reply::Info(Err(e)));
                return;
            }
        };
        let settings = hayro::hayro_interpret::InterpreterSettings::default();
        let cache = hayro::RenderCache::new();
        while let Ok(job) = jobs.recv() {
            let r = panic_to_error(catch_unwind(AssertUnwindSafe(|| {
                render_one(&pdf, &cache, &settings, job)
            })));
            let panicked = matches!(r, Err(DocError::Panicked));
            let _ = replies.send(Reply::Page(r));
            if panicked {
                // The parsed state may be inconsistent after a panic: end this worker.
                return;
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{Engine, content_key, is_poisoned, poison, stuck_workers};

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    use super::{Job, ScanLevel, parse, render_one};
    use crate::document::{Limits, PageSize, RenderedPage};
    use crate::error::DocError;

    pub(crate) fn stuck_workers() -> usize {
        0
    }

    pub(crate) fn content_key(_bytes: &[u8]) -> u64 {
        0
    }

    pub(crate) fn is_poisoned(_key: u64) -> bool {
        false
    }

    pub(crate) fn poison(_key: u64) {}

    /// Inline engine: parses again for every render (hayro's page type borrows the parsed
    /// document, so it cannot be stored next to it without extra machinery).
    // UNCERTAIN: re-parsing per render costs time on large files; not measured on wasm.
    pub(crate) struct Engine {
        bytes: Arc<Vec<u8>>,
        limits: Limits,
    }

    impl Engine {
        pub(crate) fn open(
            bytes: Arc<Vec<u8>>,
            limits: Limits,
            _key: u64,
        ) -> Result<(Engine, Vec<PageSize>), DocError> {
            let (_, sizes) = parse(&bytes, &limits, ScanLevel::Full)?;
            Ok((Engine { bytes, limits }, sizes))
        }

        /// `timeout` and `cancel` cannot be honoured without threads and are ignored.
        pub(crate) fn render(
            &mut self,
            job: Job,
            _timeout: Duration,
            _cancel: Option<&AtomicBool>,
        ) -> Result<RenderedPage, DocError> {
            let (pdf, _) = parse(&self.bytes, &self.limits, ScanLevel::Structure)?;
            let settings = hayro::hayro_interpret::InterpreterSettings::default();
            let cache = hayro::RenderCache::new();
            render_one(&pdf, &cache, &settings, job)
        }

        pub(crate) fn alive(&self) -> bool {
            true
        }
    }
}
