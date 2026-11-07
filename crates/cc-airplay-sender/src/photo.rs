// SPDX-License-Identifier: GPL-3.0-or-later
//! Photos and media links for AirPlay TVs (native-apps.md 21.5).
//!
//! * [`jpeg_for_photo`]: the photo endpoint takes JPEG only, so a PNG, WebP or GIF picture is
//!   decoded and encoded again as JPEG (quality [`JPEG_QUALITY`], the long side shrunk to at
//!   most [`MAX_PHOTO_SIDE`]); a JPEG is passed through unchanged when it is small enough.
//! * [`check_media_link`]: `/play` needs a link to a media file or stream, not a web page.

use std::io::Cursor;

use image::ImageReader;
use image::codecs::jpeg::JpegEncoder;

use crate::client::{MAX_PHOTO_BYTES, MAX_URL_LEN};
use crate::error::{Result, SenderError};

/// JPEG quality of re-encoded photos.
pub const JPEG_QUALITY: u8 = 90;
/// Longest side of a re-encoded photo, in pixels (a 4K TV).
pub const MAX_PHOTO_SIDE: u32 = 3840;
/// Largest picture we decode, in pixels (guards against decompression bombs).
pub const MAX_DECODE_PIXELS: u64 = 64_000_000;

/// A JPEG for the TV from the bytes of an image file (PNG, JPEG, WebP, GIF).
pub fn jpeg_for_photo(bytes: &[u8]) -> Result<Vec<u8>> {
    let is_jpeg = bytes.len() > 3 && bytes[..3] == [0xFF, 0xD8, 0xFF];
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| SenderError::Input(format!("not an image: {e}")))?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| SenderError::Input(format!("not an image: {e}")))?;
    if u64::from(w) * u64::from(h) > MAX_DECODE_PIXELS {
        return Err(SenderError::TooLarge("image"));
    }
    if is_jpeg && w.max(h) <= MAX_PHOTO_SIDE && bytes.len() <= MAX_PHOTO_BYTES {
        return Ok(bytes.to_vec());
    }
    let img = image::load_from_memory(bytes)
        .map_err(|e| SenderError::Input(format!("cannot read the image: {e}")))?;
    let img = if w.max(h) > MAX_PHOTO_SIDE {
        img.resize(
            MAX_PHOTO_SIDE,
            MAX_PHOTO_SIDE,
            image::imageops::FilterType::Triangle,
        )
    } else {
        img
    };
    // JPEG has no alpha: flatten onto white.
    let rgb = if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        let mut out = image::RgbImage::new(rgba.width(), rgba.height());
        for (x, y, p) in rgba.enumerate_pixels() {
            let a = u16::from(p[3]);
            let mix = |c: u8| ((u16::from(c) * a + 255 * (255 - a)) / 255) as u8;
            out.put_pixel(x, y, image::Rgb([mix(p[0]), mix(p[1]), mix(p[2])]));
        }
        out
    } else {
        img.to_rgb8()
    };
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY)
        .encode_image(&rgb)
        .map_err(|e| SenderError::Input(format!("cannot write the JPEG: {e}")))?;
    if out.len() > MAX_PHOTO_BYTES {
        return Err(SenderError::TooLarge("photo"));
    }
    Ok(out)
}

/// File name endings that name a playable media file or stream (lower case, with the dot).
const MEDIA_EXTENSIONS: [&str; 11] = [
    ".mp4", ".m4v", ".mov", ".m3u8", ".mp3", ".m4a", ".aac", ".wav", ".mpg", ".ts", ".webm",
];

/// Is this link a playable media URL? `http(s)`, a path that ends with a media file extension
/// (the query is ignored), no credentials in the link. A page of a video site is not (the TV
/// cannot play a web page). UNCERTAIN: a link without an extension that serves a video is
/// refused too; the answer to a `HEAD` request is not asked for.
pub fn check_media_link(url: &str) -> Result<()> {
    let bad = |m: &str| Err(SenderError::Input(m.into()));
    if url.len() > MAX_URL_LEN || url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return bad("the link is too long or has spaces");
    }
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(|| SenderError::Input("only http:// and https:// links work".into()))?;
    let (authority, path_query) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.is_empty() || authority.contains('@') {
        return bad("the link has no host or has credentials");
    }
    let path = path_query
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if MEDIA_EXTENSIONS.iter().any(|e| path.ends_with(e)) {
        Ok(())
    } else {
        Err(SenderError::Unsupported(
            "the link is not a media file or stream".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, alpha: bool) -> Vec<u8> {
        let mut out = Vec::new();
        let img: image::DynamicImage = if alpha {
            image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                w,
                h,
                image::Rgba([10, 120, 200, 128]),
            ))
        } else {
            image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(w, h, image::Rgb([9, 9, 9])))
        };
        img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn a_png_becomes_a_jpeg() {
        for alpha in [false, true] {
            let j = jpeg_for_photo(&png(64, 48, alpha)).unwrap();
            assert_eq!(&j[..3], &[0xFF, 0xD8, 0xFF]);
            let back = image::load_from_memory(&j).unwrap();
            assert_eq!((back.width(), back.height()), (64, 48));
        }
    }

    #[test]
    fn a_small_jpeg_passes_unchanged() {
        let j = jpeg_for_photo(&png(32, 32, false)).unwrap();
        assert_eq!(jpeg_for_photo(&j).unwrap(), j);
    }

    #[test]
    fn a_big_picture_is_shrunk() {
        let j = jpeg_for_photo(&png(MAX_PHOTO_SIDE + 200, 40, false)).unwrap();
        let back = image::load_from_memory(&j).unwrap();
        assert_eq!(back.width(), MAX_PHOTO_SIDE);
    }

    #[test]
    fn what_is_not_an_image_is_refused() {
        assert!(jpeg_for_photo(b"%PDF-1.7 not a picture").is_err());
        assert!(jpeg_for_photo(&[]).is_err());
    }

    #[test]
    fn media_links() {
        for ok in [
            "https://example.org/a/movie.mp4",
            "http://192.168.1.2:8080/live/index.M3U8?token=1",
            "https://cdn.example.org/x.mov#t=5",
        ] {
            assert!(check_media_link(ok).is_ok(), "{ok}");
        }
        for bad in [
            "https://www.youtube.com/watch?v=abc",
            "https://example.org/page.html",
            "https://example.org/",
            "ftp://example.org/a.mp4",
            "https://user:pw@example.org/a.mp4",
            "https://example.org/a b.mp4",
            "",
        ] {
            assert!(check_media_link(bad).is_err(), "{bad}");
        }
    }
}
