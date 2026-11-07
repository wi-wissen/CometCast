// SPDX-License-Identifier: GPL-3.0-or-later
//! PDF page rendering for the web client: `cc-docs` (which uses hayro) built for WebAssembly
//! with `wasm-bindgen`.
//!
//! The web app loads this module only inside a dedicated Web Worker (`apps/web/src/core/pdfWorker.ts`),
//! because on `wasm32` the renderer has no threads, no time limit and no panic protection: the
//! page ends the worker (`terminate()`) when a time limit passes or the module traps, and starts a
//! new one. Nothing here runs anything from the PDF: only the page content is drawn (no scripts, no
//! actions, no links are followed; hayro has no JavaScript and this API exposes no link data).
//!
//! The plain-Rust part ([`Viewer`], [`web_limits`], [`error_code`]) compiles and is tested on every
//! platform; the `wasm-bindgen` exports exist on `wasm32` only.

use cc_docs::{DocError, Document, Isolation, Limits};

/// Largest PDF the web client opens, in bytes. Same as the protocol limit
/// (`MAX_DOCUMENT_BYTES`, 64 MiB), so a file the transfer accepts is not refused here.
pub const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
/// Most pages the web client opens.
pub const MAX_PAGES: usize = 1000;
/// Longest side of a rendered page in pixels.
pub const MAX_OUTPUT_SIDE: u32 = 4096;
/// Largest rendered page in pixels (4096 x 2304 = 9.4 Mpx, 36 MiB of RGBA).
pub const MAX_OUTPUT_PIXELS: u64 = 4096 * 2304;

/// The limits used in the browser: stricter than the native defaults, because a browser tab
/// has less memory to spare. `WebAssembly.Memory` is also capped when the module is linked
/// (`--max-memory`, see `apps/web/scripts/build-wasm.sh`).
///
/// UNCERTAIN: every value is a guess for typical PDFs; none is measured on real devices.
/// The time limits are not used on `wasm32` (no clock there); the page enforces them.
pub fn web_limits() -> Limits {
    Limits {
        max_file_bytes: MAX_FILE_BYTES,
        max_pages: MAX_PAGES,
        max_output_side: MAX_OUTPUT_SIDE,
        max_output_pixels: MAX_OUTPUT_PIXELS,
        // A decoded image of 16 Mpx is 64 MiB at 4 components; the default 64 Mpx could
        // need 256 MiB for one image, too much next to the memory cap of the module.
        max_image_pixels: 16 * 1024 * 1024,
        max_stream_inflate_bytes: 128 * 1024 * 1024,
        max_total_inflate_bytes: 512 * 1024 * 1024,
        ..Limits::default()
    }
}

/// A short stable code for an error, used by the page as a translation key suffix.
pub fn error_code(e: &DocError) -> &'static str {
    match e {
        DocError::Empty | DocError::NotPdf | DocError::Invalid | DocError::NoPages => "invalid",
        DocError::PasswordProtected | DocError::UnsupportedEncryption(_) => "encrypted",
        DocError::TooLarge { .. } => "too_large",
        DocError::TooManyPages { .. } => "too_many_pages",
        DocError::PageOutOfRange { .. } => "page_out_of_range",
        DocError::OutputTooLarge { .. } | DocError::InvalidWidth | DocError::InvalidPageSize => {
            "bad_size"
        }
        DocError::Rejected(_) => "rejected",
        DocError::MemoryLimit => "memory",
        DocError::Timeout | DocError::Cancelled => "timeout",
        _ => "failed",
    }
}

/// An opened PDF with the web limits. Parsing happens at [`Viewer::open`]; pages are rendered
/// one at a time on request ([`Viewer::render`]), never in advance.
pub struct Viewer {
    doc: Document,
}

/// One rendered page: straight RGBA, 8 bit per channel, on an opaque white background.
pub struct Page {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

impl Viewer {
    /// Opens a PDF. The size, header, page and nesting checks run before parsing (see
    /// `cc-docs`). The error is a code from [`error_code`].
    pub fn open(bytes: Vec<u8>) -> Result<Self, &'static str> {
        // In-process: the only engine on wasm32 (no processes there), and it keeps the native
        // unit tests free of a worker program.
        let doc = Document::open_with_options(bytes, web_limits(), Isolation::InProcess)
            .map_err(|e| error_code(&e))?;
        Ok(Self { doc })
    }

    /// Number of pages (at least 1).
    pub fn page_count(&self) -> usize {
        self.doc.page_count()
    }

    /// Renders page `index` (0-based) `width_px` pixels wide. The width is clamped to the
    /// output limits first, so a request for a huge width gives a smaller picture, not an error.
    pub fn render(&self, index: usize, width_px: u32) -> Result<Page, &'static str> {
        let size = self.doc.page_size(index).map_err(|e| error_code(&e))?;
        let mut w = f64::from(width_px.clamp(1, MAX_OUTPUT_SIDE));
        let aspect = f64::from(size.height_pt) / f64::from(size.width_pt);
        // Keep height and area inside the limits by shrinking the width.
        w = w.min(f64::from(MAX_OUTPUT_SIDE) / aspect.max(1.0));
        w = w.min((MAX_OUTPUT_PIXELS as f64 / aspect).sqrt());
        let w = (w.floor() as u32).max(1);
        let page = self.doc.render_page(index, w).map_err(|e| error_code(&e))?;
        Ok(Page {
            width: page.width,
            height: page.height,
            rgba: page.rgba,
        })
    }
}

#[cfg(target_arch = "wasm32")]
mod exports {
    use wasm_bindgen::prelude::*;

    /// An opened PDF (JavaScript class `PdfDocument`).
    #[wasm_bindgen]
    pub struct PdfDocument {
        inner: super::Viewer,
    }

    /// A rendered page (JavaScript class `RenderedPage`).
    #[wasm_bindgen]
    pub struct RenderedPage {
        inner: super::Page,
    }

    #[wasm_bindgen]
    impl PdfDocument {
        /// Opens a PDF; throws an `Error` whose `message` is a short code (`invalid`,
        /// `encrypted`, `too_large`, `too_many_pages`, `rejected`, `memory`, `failed`).
        pub fn open(bytes: Vec<u8>) -> Result<PdfDocument, JsError> {
            super::Viewer::open(bytes)
                .map(|inner| PdfDocument { inner })
                .map_err(JsError::new)
        }

        /// Number of pages.
        pub fn page_count(&self) -> u32 {
            u32::try_from(self.inner.page_count()).unwrap_or(u32::MAX)
        }

        /// Renders page `index` (0-based) about `width_px` pixels wide.
        pub fn render(&self, index: u32, width_px: u32) -> Result<RenderedPage, JsError> {
            self.inner
                .render(index as usize, width_px)
                .map(|inner| RenderedPage { inner })
                .map_err(JsError::new)
        }
    }

    #[wasm_bindgen]
    impl RenderedPage {
        /// Width in pixels.
        #[wasm_bindgen(getter)]
        pub fn width(&self) -> u32 {
            self.inner.width
        }

        /// Height in pixels.
        #[wasm_bindgen(getter)]
        pub fn height(&self) -> u32 {
            self.inner.height
        }

        /// The pixels as RGBA bytes (a copy).
        pub fn rgba(&self) -> Vec<u8> {
            self.inner.rgba.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-page PDF with a black rectangle and a correct cross-reference table.
    fn tiny_pdf(width: u32, height: u32) -> Vec<u8> {
        let content = "0 0 0 rg 10 10 100 100 re f";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Contents 4 0 R >>"
            ),
            format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            ),
        ];
        let mut out = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (i, body) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
        }
        let xref = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
        );
        for o in offsets {
            out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Root 1 0 R /Size {} >>\nstartxref\n{xref}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        out
    }

    #[test]
    fn opens_and_renders_a_page_that_is_not_blank() {
        let v = Viewer::open(tiny_pdf(200, 200)).expect("open");
        assert_eq!(v.page_count(), 1);
        let p = v.render(0, 100).expect("render");
        assert_eq!((p.width, p.height), (100, 100));
        assert_eq!(p.rgba.len(), 100 * 100 * 4);
        let dark = p.rgba.chunks_exact(4).filter(|px| px[0] < 64).count();
        assert!(
            dark > 500,
            "the black rectangle must be drawn, dark pixels: {dark}"
        );
    }

    #[test]
    fn page_out_of_range_and_garbage_give_codes() {
        let v = Viewer::open(tiny_pdf(200, 200)).expect("open");
        assert_eq!(v.render(5, 100).err(), Some("page_out_of_range"));
        assert_eq!(Viewer::open(b"hello".to_vec()).err(), Some("invalid"));
        assert_eq!(Viewer::open(Vec::new()).err(), Some("invalid"));
    }

    #[test]
    fn a_huge_width_is_clamped_not_refused() {
        let v = Viewer::open(tiny_pdf(200, 400)).expect("open");
        let p = v.render(0, u32::MAX).expect("render");
        assert!(p.width <= MAX_OUTPUT_SIDE && p.height <= MAX_OUTPUT_SIDE);
        assert!(u64::from(p.width) * u64::from(p.height) <= MAX_OUTPUT_PIXELS);
        // A very tall page still fits the limits.
        let tall = Viewer::open(tiny_pdf(10, 14000)).expect("open tall");
        let p = tall.render(0, 4000).expect("render tall");
        assert!(p.height <= MAX_OUTPUT_SIDE);
    }

    #[test]
    fn files_over_the_web_limit_are_refused_before_parsing() {
        let mut big = b"%PDF-1.4\n".to_vec();
        big.resize(MAX_FILE_BYTES + 1, b' ');
        assert_eq!(Viewer::open(big).err(), Some("too_large"));
    }
}
