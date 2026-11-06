// SPDX-License-Identifier: GPL-3.0-or-later
//! The document type: limits first, parsing and rendering on a guarded worker.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use crate::error::DocError;
use crate::worker::{Engine, Job};

/// Hard limits. They are checked before a file is parsed (size) or before work is done
/// (output size) so that a hostile file cannot make the receiver allocate without bound.
///
/// UNCERTAIN: every default below is a guess for a classroom setting. None is derived from
/// a measurement of real devices (an Apple TV or a weak mini PC may need lower values);
/// they are public fields so the application can set its own.
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
    /// Time allowed for parsing at open (default 10 s). Abandoned, not interrupted.
    pub open_timeout: Duration,
    /// Time allowed for one render (default 10 s). Abandoned, not interrupted.
    pub render_timeout: Duration,
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
        }
    }
}

/// Size of a page in PDF points (1/72 inch) after applying the page rotation and the crop box.
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
    /// Replaces [`Limits::render_timeout`] for this call.
    pub timeout: Option<Duration>,
    /// When set to `true` while the render runs, the call returns [`DocError::Cancelled`]
    /// within about 10 ms. The renderer itself is not interrupted (see the module docs).
    pub cancel: Option<Arc<AtomicBool>>,
}

/// An opened PDF document.
///
/// All methods take `&self`; renders on one document run one after the other.
pub struct Document {
    bytes: Arc<Vec<u8>>,
    limits: Limits,
    sizes: Vec<PageSize>,
    engine: Mutex<Option<Engine>>,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("bytes", &self.bytes.len())
            .field("pages", &self.sizes.len())
            .finish()
    }
}

/// Number of renders that were abandoned after a time limit or cancel and still run inside
/// the renderer (always 0 on `wasm32`). New documents are refused with [`DocError::Busy`]
/// when this reaches a small internal bound.
pub fn abandoned_renders() -> usize {
    crate::worker::stuck_workers()
}

impl Document {
    /// Open a PDF with the default [`Limits`].
    pub fn open(bytes: Vec<u8>) -> Result<Document, DocError> {
        Self::open_with_limits(bytes, Limits::default())
    }

    /// Open a PDF. The size and header checks run **before** any parsing.
    ///
    /// Encrypted files: a file that needs a password is refused
    /// ([`DocError::PasswordProtected`]). hayro 0.9 can open files that are encrypted with an
    /// empty user password (RC4, AES-128, AES-256 as its sources show); such a file is accepted.
    pub fn open_with_limits(bytes: Vec<u8>, limits: Limits) -> Result<Document, DocError> {
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
        let bytes = Arc::new(bytes);
        let (engine, sizes) = Engine::open(Arc::clone(&bytes), limits)?;
        Ok(Document {
            bytes,
            limits,
            sizes,
            engine: Mutex::new(Some(engine)),
        })
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

    /// A small picture of page `index` that fits into `max_side` x `max_side` pixels.
    pub fn thumbnail(&self, index: usize, max_side: u32) -> Result<RenderedPage, DocError> {
        let size = self.page_size(index)?;
        if max_side == 0 {
            return Err(DocError::InvalidWidth);
        }
        if !size.is_valid() {
            return Err(DocError::InvalidPageSize);
        }
        let (w, h) = (f64::from(size.width_pt), f64::from(size.height_pt));
        let width = if w >= h {
            f64::from(max_side)
        } else {
            (f64::from(max_side) * w / h).round().max(1.0)
        };
        // `width` is at most max_side (a u32), so the cast cannot overflow.
        self.render_page(index, width as u32)
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
        let l = &self.limits;
        // hayro takes u16 sizes; never exceed that even if the caller raised max_output_side.
        let side_cap = f64::from(l.max_output_side.min(u32::from(u16::MAX)));
        let too_large = || DocError::OutputTooLarge {
            width: u64::from(width_px),
            height: if h.is_finite() && h < 1e18 {
                h as u64
            } else {
                u64::MAX
            },
        };
        if !h.is_finite() || f64::from(width_px) > side_cap || h > side_cap {
            return Err(too_large());
        }
        let height = h as u32;
        if u64::from(width_px) * u64::from(height) > l.max_output_pixels {
            return Err(too_large());
        }
        Ok((width_px, height))
    }

    fn lock(&self) -> MutexGuard<'_, Option<Engine>> {
        self.engine.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn run(&self, job: Job, options: &RenderOptions) -> Result<RenderedPage, DocError> {
        let timeout = options.timeout.unwrap_or(self.limits.render_timeout);
        let mut guard = self.lock();
        // Restart the worker if the previous one timed out, was cancelled or panicked.
        if !guard.as_ref().is_some_and(Engine::alive) {
            *guard = None;
            let (engine, _) = Engine::open(Arc::clone(&self.bytes), self.limits)?;
            *guard = Some(engine);
        }
        let engine = guard.as_mut().ok_or(DocError::Internal("no engine"))?;
        let r = engine.render(job, timeout, options.cancel.as_deref());
        if !engine.alive() {
            *guard = None;
        }
        r
    }
}
