// SPDX-License-Identifier: GPL-3.0-or-later
//! The document type: limits first, then parsing and rendering in a resource-limited child
//! process (default) or on a guarded thread.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use crate::error::DocError;
use crate::isolation::Isolation;
use crate::protection::{Mask, Protection};
use crate::worker::{Engine, Job};

/// Hard limits. They are checked before a file is parsed (size, page claims) or before work
/// is done (output size), and the memory and image fields below are enforced inside the
/// render worker process, so that a hostile file cannot make the receiver allocate without
/// bound.
///
/// UNCERTAIN: every default below is a guess for a typical meeting-room setting. The memory default is
/// derived from measurements on one Linux machine (see `docs/documents.md`); none is derived
/// from a measurement of real devices (an Apple TV or a weak mini PC may need lower
/// values). They are public fields so the application can set its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Largest accepted input in bytes (default 100 MiB).
    pub max_file_bytes: usize,
    /// Most pages accepted (default 5000).
    pub max_pages: usize,
    /// Largest output width or height in pixels (default 8192; hayro's own limit is 65535).
    pub max_output_side: u32,
    /// Largest output area in pixels (default 7680 x 4320 = 33 177 600, about 127 MiB RGBA).
    pub max_output_pixels: u64,
    /// Time allowed for parsing at open (default 10 s). The worker process is killed when it
    /// passes; the in-process engine abandons its thread.
    pub open_timeout: Duration,
    /// Time allowed for one render (default 10 s). Same consequence as for `open_timeout`.
    pub render_timeout: Duration,
    /// Memory of the render worker process in bytes (default 1 GiB). On Linux and Android
    /// this is the address-space limit (`RLIMIT_AS`, set by the worker itself, fail closed)
    /// and also the resident-memory cap that the parent watches; elsewhere only the watched
    /// cap. Not used by the in-process engine.
    pub max_worker_memory_bytes: u64,
    /// Largest declared image area (`Width * Height`) in pixels (default 64 Mpx = 8000 x
    /// 8000). A 10 000 x 10 000 scan is refused: the price of the bomb check.
    pub max_image_pixels: u64,
    /// Most bytes one `FlateDecode` stream may inflate to in the pre-scan (default 256 MiB:
    /// an 8-bit CMYK image of `max_image_pixels` is exactly that; the review suggested 128).
    pub max_stream_inflate_bytes: u64,
    /// Most bytes all `FlateDecode` streams together may inflate to in the pre-scan
    /// (default 1 GiB).
    pub max_total_inflate_bytes: u64,
    /// Most objects in the file (default 2 000 000).
    pub max_objects: usize,
    /// Longest filter chain of one stream (default 8).
    pub max_filter_chain: usize,
    /// Inflate every `FlateDecode` stream once at open to find decompression bombs (default
    /// `true`; costs about the time of reading the file). `false` leaves that to the process
    /// limits; the size and structure checks stay on.
    pub inflate_scan: bool,
    /// Deepest nesting of dictionaries and arrays in the file body, checked on the raw bytes
    /// before parsing (default 256; real files stay far below 50).
    pub max_nesting_depth: usize,
    /// Refuse to parse unless the worker could restrict itself with **both** Landlock (no
    /// file system access) and seccomp (no `fork`, `exec`, `socket`) (default `false`). With
    /// `false` a worker on a system without them (old kernel, container that forbids them,
    /// macOS, Windows, Android) still runs with its resource limits only, and
    /// [`Document::protections`] says what was in place. With `true` such a system gives
    /// [`DocError::SandboxUnavailable`] before the PDF is parsed.
    pub require_os_sandbox: bool,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_bytes: 100 * 1024 * 1024,
            max_pages: 5000,
            max_output_side: 8192,
            max_output_pixels: 7680 * 4320,
            open_timeout: Duration::from_secs(10),
            render_timeout: Duration::from_secs(10),
            max_worker_memory_bytes: 1024 * 1024 * 1024,
            max_image_pixels: 64 * 1024 * 1024,
            max_stream_inflate_bytes: 256 * 1024 * 1024,
            max_total_inflate_bytes: 1024 * 1024 * 1024,
            max_objects: 2_000_000,
            max_filter_chain: 8,
            inflate_scan: true,
            max_nesting_depth: 256,
            require_os_sandbox: false,
        }
    }
}

/// Size of a page in PDF points (1/72 inch) after applying the page rotation and the crop box.
/// A `/UserUnit` entry is not applied (hayro ignores it; a page is shown at its point size
/// whatever the unit says).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSize {
    /// Width in points.
    pub width_pt: f32,
    /// Height in points.
    pub height_pt: f32,
}

impl PageSize {
    /// Both sides finite and greater than zero.
    pub fn is_valid(&self) -> bool {
        self.width_pt.is_finite()
            && self.height_pt.is_finite()
            && self.width_pt > 0.0
            && self.height_pt > 0.0
    }
}

/// A rendered picture: straight RGBA, 8 bit per channel, row by row from the top left.
/// The page is drawn on an opaque white background, so alpha is 255 everywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

impl RenderedPage {
    /// The pixel at `(x, y)` as `[r, g, b, a]`, or `None` outside the picture.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        self.rgba.get(i..i + 4)?.try_into().ok()
    }
}

/// Per-call options for [`Document::render_page_with`].
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// Replaces [`Limits::render_timeout`] for this call. Values above one day are cut to one
    /// day (`Duration::MAX` is fine).
    pub timeout: Option<Duration>,
    /// When set to `true` while the render runs, the call returns [`DocError::Cancelled`]
    /// within about 10 ms. With a worker process the process is killed; the in-process
    /// engine cannot interrupt the renderer (see the module docs of `worker.rs`).
    pub cancel: Option<Arc<AtomicBool>>,
}

/// State of the in-process engine of one document.
#[derive(Default)]
struct ThreadState {
    engine: Option<Engine>,
    /// A render timed out: the document is never rendered again.
    poisoned: bool,
    /// Cancelled renders left a thread behind this many times.
    cancels: u32,
}

/// After this many cancelled (abandoned) renders the in-process engine stops rendering the
/// document.
const MAX_CANCELS: u32 = 3;

enum Backend {
    #[cfg(all(
        not(target_arch = "wasm32"),
        not(target_os = "ios"),
        not(target_os = "tvos")
    ))]
    Process {
        spec: crate::isolation::WorkerSpec,
        /// One child at a time per document.
        lock: Mutex<()>,
    },
    Thread(Mutex<ThreadState>),
}

/// An opened PDF document.
///
/// All methods take `&self`; renders on one document run one after the other.
pub struct Document {
    bytes: Arc<Vec<u8>>,
    limits: Limits,
    sizes: Vec<PageSize>,
    backend: Backend,
    /// Hash of the content, for the poison list of the in-process engine.
    key: u64,
    /// Highest memory (KiB) seen from the last worker process, 0 if unknown.
    last_peak_kib: AtomicU64,
    /// Protections the last worker process reported (bit mask, see `protection.rs`).
    last_protections: AtomicU64,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("bytes", &self.bytes.len())
            .field("pages", &self.sizes.len())
            .field("isolated", &self.is_isolated())
            .finish()
    }
}

/// Number of renders that were abandoned after a time limit or cancel and still run inside
/// the in-process renderer (always 0 on `wasm32` and with worker processes, which are killed
/// instead of abandoned).
pub fn abandoned_renders() -> usize {
    crate::worker::stuck_workers()
}

enum Resolved {
    #[cfg(all(
        not(target_arch = "wasm32"),
        not(target_os = "ios"),
        not(target_os = "tvos")
    ))]
    Process(crate::isolation::WorkerSpec),
    Thread,
}

fn resolve(isolation: Isolation) -> Result<Resolved, DocError> {
    #[cfg(all(
        not(target_arch = "wasm32"),
        not(target_os = "ios"),
        not(target_os = "tvos")
    ))]
    {
        match isolation {
            Isolation::InProcess => Ok(Resolved::Thread),
            Isolation::Process(spec) => Ok(Resolved::Process(spec)),
            Isolation::Auto => crate::isolation::WorkerSpec::discover_checked()
                .map(Resolved::Process)
                .map_err(DocError::WorkerUnavailable),
        }
    }
    #[cfg(not(all(
        not(target_arch = "wasm32"),
        not(target_os = "ios"),
        not(target_os = "tvos")
    )))]
    {
        match isolation {
            Isolation::Process(_) => Err(DocError::WorkerUnavailable(
                "child processes are not available on this platform".into(),
            )),
            Isolation::Auto | Isolation::InProcess => Ok(Resolved::Thread),
        }
    }
}

impl Document {
    /// Open a PDF with the default [`Limits`] and [`Isolation::Auto`].
    pub fn open(bytes: Vec<u8>) -> Result<Document, DocError> {
        Self::open_with_options(bytes, Limits::default(), Isolation::Auto)
    }

    /// Open a PDF with [`Isolation::Auto`]. The size, header and page-claim checks run
    /// **before** any parsing.
    ///
    /// Encrypted files: a file that needs a password is refused
    /// ([`DocError::PasswordProtected`]). hayro 0.9 can open files that are encrypted with an
    /// empty user password (RC4, AES-128, AES-256 as its sources show); such a file is accepted.
    pub fn open_with_limits(bytes: Vec<u8>, limits: Limits) -> Result<Document, DocError> {
        Self::open_with_options(bytes, limits, Isolation::Auto)
    }

    /// Open a PDF with limits and an explicit choice where the renderer runs.
    pub fn open_with_options(
        bytes: Vec<u8>,
        limits: Limits,
        isolation: Isolation,
    ) -> Result<Document, DocError> {
        if bytes.is_empty() {
            return Err(DocError::Empty);
        }
        if bytes.len() > limits.max_file_bytes {
            return Err(DocError::TooLarge {
                size: bytes.len(),
                max: limits.max_file_bytes,
            });
        }
        let head = &bytes[..bytes.len().min(1024)];
        if !head.windows(5).any(|w| w == b"%PDF-") {
            return Err(DocError::NotPdf);
        }
        crate::prescan::raw_page_precheck(&bytes, limits.max_pages)?;
        crate::prescan::raw_nesting_precheck(&bytes, limits.max_nesting_depth)?;
        let resolved = resolve(isolation)?;
        let bytes = Arc::new(bytes);
        let last_peak_kib = AtomicU64::new(0);
        let last_protections = AtomicU64::new(0);
        let (backend, sizes, key) = match resolved {
            #[cfg(all(
                not(target_arch = "wasm32"),
                not(target_os = "ios"),
                not(target_os = "tvos")
            ))]
            Resolved::Process(spec) => {
                let done = crate::process::run(
                    &spec,
                    &bytes,
                    &limits,
                    crate::process::Call::Info,
                    limits.open_timeout,
                    None,
                );
                last_peak_kib.store(done.peak_kib, Ordering::Relaxed);
                last_protections.store(done.protections.0, Ordering::Relaxed);
                let sizes = match done.result? {
                    crate::process::Output::Info(s) => s,
                    crate::process::Output::Page(_) => {
                        return Err(DocError::Internal("unexpected reply"));
                    }
                };
                (
                    Backend::Process {
                        spec,
                        lock: Mutex::new(()),
                    },
                    sizes,
                    0,
                )
            }
            Resolved::Thread => {
                let key = crate::worker::content_key(&bytes);
                let (engine, sizes) = Engine::open(Arc::clone(&bytes), limits, key)?;
                let state = ThreadState {
                    engine: Some(engine),
                    ..ThreadState::default()
                };
                (Backend::Thread(Mutex::new(state)), sizes, key)
            }
        };
        Ok(Document {
            bytes,
            limits,
            sizes,
            backend,
            key,
            last_peak_kib,
            last_protections,
        })
    }

    /// `true` when pages are rendered in a resource-limited child process, `false` for the
    /// in-process engine (no memory bound, no separation at all).
    ///
    /// This does **not** mean "sandboxed" in the full sense: the child has the same user id
    /// and, unless [`protections`](Self::protections) lists `Landlock` and `Seccomp`, can
    /// still read the user's files and use the network. See `docs/documents.md`.
    pub fn is_isolated(&self) -> bool {
        !matches!(self.backend, Backend::Thread(_))
    }

    /// The protections that were really in place around the worker of the last call: those
    /// the parent provides (own process group, clean environment, memory watchdog) and the
    /// ones the worker reported after applying them to itself (limits, Landlock, seccomp).
    /// Empty for the in-process engine and before the first call finished. Landlock and
    /// seccomp are best effort (see [`Limits::require_os_sandbox`]); this is how an
    /// application finds out whether they were available.
    pub fn protections(&self) -> Vec<Protection> {
        Mask(self.last_protections.load(Ordering::Relaxed)).list()
    }

    /// Highest memory (KiB) seen from the worker process of the last call, as far as the
    /// parent could read it (the worker reports its own peak on Linux; the parent polls it).
    /// `None` for the in-process engine or when nothing could be read.
    pub fn last_worker_peak_kib(&self) -> Option<u64> {
        match self.last_peak_kib.load(Ordering::Relaxed) {
            0 => None,
            k => Some(k),
        }
    }

    /// Number of pages (at least 1).
    pub fn page_count(&self) -> usize {
        self.sizes.len()
    }

    /// Size of page `index` (0-based) in points.
    pub fn page_size(&self, index: usize) -> Result<PageSize, DocError> {
        self.sizes
            .get(index)
            .copied()
            .ok_or(DocError::PageOutOfRange {
                index,
                count: self.sizes.len(),
            })
    }

    /// Speaker notes of a page. Always `None` for PDFs: a PDF has no notes concept that this
    /// crate reads. Notes come from PPTX files later (planned with `cc-office`, not built).
    pub fn notes(&self, _index: usize) -> Option<String> {
        None
    }

    /// Render page `index` (0-based) `width_px` pixels wide; the height follows the page
    /// aspect ratio. Uses [`Limits::render_timeout`].
    pub fn render_page(&self, index: usize, width_px: u32) -> Result<RenderedPage, DocError> {
        self.render_page_with(index, width_px, &RenderOptions::default())
    }

    /// Like [`render_page`](Self::render_page) with a timeout override and a cancel flag.
    pub fn render_page_with(
        &self,
        index: usize,
        width_px: u32,
        options: &RenderOptions,
    ) -> Result<RenderedPage, DocError> {
        let size = self.page_size(index)?;
        let (width, height) = self.output_size(size, width_px)?;
        self.run(
            Job {
                index,
                width,
                height,
            },
            options,
        )
    }

    /// A small picture of page `index` that fits into `max_side` x `max_side` pixels. The
    /// page shape is kept as far as whole pixels allow; for an extreme shape (a 10 x 10 000 pt
    /// page into 64 pixels) the short side is at least 1 pixel and the picture is distorted
    /// rather than larger than the box.
    pub fn thumbnail(&self, index: usize, max_side: u32) -> Result<RenderedPage, DocError> {
        let size = self.page_size(index)?;
        if max_side == 0 {
            return Err(DocError::InvalidWidth);
        }
        if !size.is_valid() {
            return Err(DocError::InvalidPageSize);
        }
        let (w, h) = (f64::from(size.width_pt), f64::from(size.height_pt));
        let scale = f64::from(max_side) / w.max(h);
        let fit = |side: f64| ((side * scale).round().max(1.0) as u32).min(max_side);
        let (width, height) = (fit(w), fit(h));
        self.check_output(width, height)?;
        self.run(
            Job {
                index,
                width,
                height,
            },
            &RenderOptions::default(),
        )
    }

    /// Output size for `width_px`, or the reason it is refused. Pure; used before any work.
    fn output_size(&self, size: PageSize, width_px: u32) -> Result<(u32, u32), DocError> {
        if width_px == 0 {
            return Err(DocError::InvalidWidth);
        }
        if !size.is_valid() {
            return Err(DocError::InvalidPageSize);
        }
        let ratio = f64::from(size.height_pt) / f64::from(size.width_pt);
        let h = (f64::from(width_px) * ratio).round().max(1.0);
        let side_cap = f64::from(self.limits.max_output_side.min(u32::from(u16::MAX)));
        if !h.is_finite() || h > side_cap {
            return Err(DocError::OutputTooLarge {
                width: u64::from(width_px),
                height: if h.is_finite() && h < 1e18 {
                    h as u64
                } else {
                    u64::MAX
                },
            });
        }
        let height = h as u32;
        self.check_output(width_px, height)?;
        Ok((width_px, height))
    }

    /// Both sides within the side cap and the area within the pixel cap.
    fn check_output(&self, width: u32, height: u32) -> Result<(), DocError> {
        let l = &self.limits;
        // hayro takes u16 sizes; never exceed that even if the caller raised max_output_side.
        let side_cap = l.max_output_side.min(u32::from(u16::MAX));
        if width > side_cap
            || height > side_cap
            || u64::from(width) * u64::from(height) > l.max_output_pixels
        {
            return Err(DocError::OutputTooLarge {
                width: u64::from(width),
                height: u64::from(height),
            });
        }
        Ok(())
    }

    fn run(&self, job: Job, options: &RenderOptions) -> Result<RenderedPage, DocError> {
        let timeout = options.timeout.unwrap_or(self.limits.render_timeout);
        match &self.backend {
            #[cfg(all(
                not(target_arch = "wasm32"),
                not(target_os = "ios"),
                not(target_os = "tvos")
            ))]
            Backend::Process { spec, lock } => {
                let _one_at_a_time = lock.lock().unwrap_or_else(|p| p.into_inner());
                let done = crate::process::run(
                    spec,
                    &self.bytes,
                    &self.limits,
                    crate::process::Call::Render {
                        index: job.index,
                        width: job.width,
                        height: job.height,
                    },
                    timeout,
                    options.cancel.as_deref(),
                );
                self.last_peak_kib.store(done.peak_kib, Ordering::Relaxed);
                self.last_protections
                    .store(done.protections.0, Ordering::Relaxed);
                match done.result? {
                    crate::process::Output::Page(p) => Ok(p),
                    crate::process::Output::Info(_) => Err(DocError::Internal("unexpected reply")),
                }
            }
            Backend::Thread(state) => self.run_thread(state, job, timeout, options),
        }
    }

    fn run_thread(
        &self,
        state: &Mutex<ThreadState>,
        job: Job,
        timeout: Duration,
        options: &RenderOptions,
    ) -> Result<RenderedPage, DocError> {
        let mut st: MutexGuard<'_, ThreadState> = state.lock().unwrap_or_else(|p| p.into_inner());
        if st.poisoned || crate::worker::is_poisoned(self.key) {
            return Err(DocError::Poisoned);
        }
        // Restart the worker if the previous one was cancelled or panicked.
        if !st.engine.as_ref().is_some_and(Engine::alive) {
            st.engine = None;
            match Engine::open(Arc::clone(&self.bytes), self.limits, self.key) {
                Ok((engine, _)) => st.engine = Some(engine),
                Err(e) => {
                    if e == DocError::Timeout {
                        st.poisoned = true;
                    }
                    return Err(e);
                }
            }
        }
        let engine = st.engine.as_mut().ok_or(DocError::Internal("no engine"))?;
        let r = engine.render(job, timeout, options.cancel.as_deref());
        let alive = engine.alive();
        match &r {
            // One bad file costs one thread: never render it again.
            Err(DocError::Timeout) => {
                st.poisoned = true;
                crate::worker::poison(self.key);
            }
            Err(DocError::Cancelled) => {
                st.cancels += 1;
                if st.cancels >= MAX_CANCELS {
                    st.poisoned = true;
                    crate::worker::poison(self.key);
                }
            }
            _ => {}
        }
        if !alive {
            st.engine = None;
        }
        r
    }
}
