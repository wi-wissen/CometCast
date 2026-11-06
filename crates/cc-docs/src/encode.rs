// SPDX-License-Identifier: GPL-3.0-or-later
//! Picture helpers: PNG and lossless WebP encoding of rendered pages, and a WebP check.
//!
//! Chosen crates (both pure Rust, `MIT OR Apache-2.0`, no C code): `png` 0.18 for PNG and
//! `image-webp` 0.2 for WebP. `image-webp` encodes **lossless only**; lossy WebP would need
//! libwebp (C), which this crate does not use. Lossless WebP of rendered text pages is
//! usually smaller than PNG, but that was not measured here.

use crate::document::RenderedPage;
use crate::error::DocError;

fn check_len(p: &RenderedPage) -> Result<(), DocError> {
    let expected = u64::from(p.width) * u64::from(p.height) * 4;
    if p.width == 0 || p.height == 0 || expected != p.rgba.len() as u64 {
        return Err(DocError::Encode(
            "picture size does not match its data".into(),
        ));
    }
    Ok(())
}

/// Encode a page as PNG (8 bit RGBA).
pub fn encode_png(page: &RenderedPage) -> Result<Vec<u8>, DocError> {
    check_len(page)?;
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, page.width, page.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc
            .write_header()
            .map_err(|e| DocError::Encode(e.to_string()))?;
        w.write_image_data(&page.rgba)
            .map_err(|e| DocError::Encode(e.to_string()))?;
        w.finish().map_err(|e| DocError::Encode(e.to_string()))?;
    }
    Ok(out)
}

/// Encode a page as lossless WebP.
pub fn encode_webp_lossless(page: &RenderedPage) -> Result<Vec<u8>, DocError> {
    check_len(page)?;
    let mut out = Vec::new();
    image_webp::WebPEncoder::new(&mut out)
        .encode(
            &page.rgba,
            page.width,
            page.height,
            image_webp::ColorType::Rgba8,
        )
        .map_err(|e| DocError::Encode(e.to_string()))?;
    Ok(out)
}

/// What [`validate_webp`] found out without decoding the pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebpInfo {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The file has an alpha channel.
    pub has_alpha: bool,
    /// The file is an animation (not accepted by [`validate_webp`]).
    pub animated: bool,
}

/// Check a WebP file: the container must parse, it must not be animated, and
/// `width * height` must not exceed `max_pixels`. Pixels are **not** decoded, so this is
/// cheap enough for the server (the plan lists WebP validation as the only part of this
/// crate the server uses).
///
/// UNCERTAIN: this checks the header only. A file with a valid header and a corrupt pixel
/// stream passes here and fails later in the client's decoder.
pub fn validate_webp(bytes: &[u8], max_pixels: u64) -> Result<WebpInfo, DocError> {
    let dec = image_webp::WebPDecoder::new(std::io::Cursor::new(bytes))
        .map_err(|e| DocError::InvalidWebp(e.to_string()))?;
    let (width, height) = dec.dimensions();
    let animated = dec.is_animated();
    if animated {
        return Err(DocError::InvalidWebp(
            "animated WebP is not accepted".into(),
        ));
    }
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > max_pixels {
        return Err(DocError::InvalidWebp(format!(
            "picture of {width} x {height} pixels is outside the limit"
        )));
    }
    Ok(WebpInfo {
        width,
        height,
        has_alpha: dec.has_alpha(),
        animated,
    })
}
