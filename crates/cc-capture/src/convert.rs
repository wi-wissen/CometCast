// SPDX-License-Identifier: GPL-3.0-or-later
//! Pixel conversion shared by the picker backends. Every backend delivers pictures in some
//! packed RGB layout with a row stride; the sender wants tightly packed BGRA ([`RawFrame`]),
//! which `cc-media` converts to I420 for the encoder.
//!
//! Layout names follow the PipeWire/SPA convention: the letters give the byte order in memory
//! (`Bgra` = byte 0 is blue). Alpha is always replaced by 255: a screen picture is opaque, and
//! some compositors leave garbage in the "x" byte.

use crate::{CaptureError, RawFrame};

/// Largest width or height accepted from a platform (guards allocations).
pub const MAX_SIDE: u32 = 16_384;
/// Largest picture area accepted from a platform, in pixels (about an 8K picture).
pub const MAX_PIXELS: u64 = 8_192 * 4_608;

/// A packed pixel layout a backend may hand over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// B, G, R, A in memory (Windows, macOS, most Wayland compositors).
    Bgra,
    /// B, G, R, unused.
    Bgrx,
    /// R, G, B, A.
    Rgba,
    /// R, G, B, unused.
    Rgbx,
    /// A, R, G, B.
    Argb,
    /// Unused, R, G, B.
    Xrgb,
    /// A, B, G, R.
    Abgr,
    /// Unused, B, G, R.
    Xbgr,
    /// R, G, B (3 bytes).
    Rgb,
    /// B, G, R (3 bytes).
    Bgr,
}

impl PixelFormat {
    /// Bytes per pixel.
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rgb | Self::Bgr => 3,
            _ => 4,
        }
    }

    /// Positions of (blue, green, red) inside one pixel.
    fn bgr_offsets(self) -> (usize, usize, usize) {
        match self {
            Self::Bgra | Self::Bgrx | Self::Bgr => (0, 1, 2),
            Self::Rgba | Self::Rgbx | Self::Rgb => (2, 1, 0),
            Self::Argb | Self::Xrgb => (3, 2, 1),
            Self::Abgr | Self::Xbgr => (1, 2, 3),
        }
    }
}

/// A rectangle inside a buffer that holds the visible picture (PipeWire "video crop" meta).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crop {
    /// Left edge in pixels.
    pub x: u32,
    /// Top edge in pixels.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Check a picture size against [`MAX_SIDE`] and [`MAX_PIXELS`] and against zero.
pub fn check_size(width: u32, height: u32) -> Result<(), CaptureError> {
    if width == 0 || height == 0 {
        return Err(CaptureError::PixelFormat(format!(
            "empty picture {width}x{height}"
        )));
    }
    if width > MAX_SIDE || height > MAX_SIDE || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(CaptureError::PixelFormat(format!(
            "picture {width}x{height} is larger than supported"
        )));
    }
    Ok(())
}

/// Convert a buffer with `stride` bytes per row into a tight BGRA [`RawFrame`], optionally
/// cutting out `crop`. Checks that the buffer is big enough for what it claims.
pub fn to_bgra(
    format: PixelFormat,
    src: &[u8],
    width: u32,
    height: u32,
    stride: usize,
    crop: Option<Crop>,
) -> Result<RawFrame, CaptureError> {
    check_size(width, height)?;
    let bpp = format.bytes_per_pixel();
    let row_bytes = width as usize * bpp;
    if stride < row_bytes {
        return Err(CaptureError::PixelFormat(format!(
            "stride {stride} is smaller than a row of {width} pixels"
        )));
    }
    let needed = stride
        .checked_mul(height as usize - 1)
        .and_then(|n| n.checked_add(row_bytes))
        .ok_or_else(|| CaptureError::PixelFormat("buffer size overflow".into()))?;
    if src.len() < needed {
        return Err(CaptureError::PixelFormat(format!(
            "buffer has {} bytes, {needed} needed",
            src.len()
        )));
    }
    let (cx, cy, cw, ch) = match crop {
        Some(c) => {
            let inside = c.x.checked_add(c.width).is_some_and(|r| r <= width)
                && c.y.checked_add(c.height).is_some_and(|b| b <= height);
            if !inside || c.width == 0 || c.height == 0 {
                // A crop that does not fit is ignored rather than failing the whole stream.
                (0, 0, width, height)
            } else {
                (c.x, c.y, c.width, c.height)
            }
        }
        None => (0, 0, width, height),
    };
    let mut out = vec![0u8; cw as usize * ch as usize * 4];
    let (ob, og, or) = format.bgr_offsets();
    for row in 0..ch as usize {
        let s = (cy as usize + row) * stride + cx as usize * bpp;
        let src_row = &src[s..s + cw as usize * bpp];
        let dst_row = &mut out[row * cw as usize * 4..(row + 1) * cw as usize * 4];
        if format == PixelFormat::Bgrx || format == PixelFormat::Bgra {
            dst_row.copy_from_slice(src_row);
            for px in dst_row.as_chunks_mut::<4>().0.iter_mut() {
                px[3] = 255;
            }
        } else {
            for (d, s) in dst_row
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(src_row.chunks_exact(bpp))
            {
                d[0] = s[ob];
                d[1] = s[og];
                d[2] = s[or];
                d[3] = 255;
            }
        }
    }
    Ok(RawFrame {
        width: cw,
        height: ch,
        bgra: out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgrx_keeps_colour_and_forces_alpha() {
        let src = [10, 20, 30, 0, 40, 50, 60, 7];
        let f = to_bgra(PixelFormat::Bgrx, &src, 2, 1, 8, None).unwrap();
        assert_eq!(f.bgra, vec![10, 20, 30, 255, 40, 50, 60, 255]);
    }

    #[test]
    fn all_layouts_give_the_same_colour() {
        // Colour: R=1, G=2, B=3.
        let cases: [(PixelFormat, Vec<u8>); 8] = [
            (PixelFormat::Bgra, vec![3, 2, 1, 9]),
            (PixelFormat::Bgrx, vec![3, 2, 1, 9]),
            (PixelFormat::Rgba, vec![1, 2, 3, 9]),
            (PixelFormat::Rgbx, vec![1, 2, 3, 9]),
            (PixelFormat::Argb, vec![9, 1, 2, 3]),
            (PixelFormat::Xrgb, vec![9, 1, 2, 3]),
            (PixelFormat::Abgr, vec![9, 3, 2, 1]),
            (PixelFormat::Xbgr, vec![9, 3, 2, 1]),
        ];
        for (fmt, px) in cases {
            let f = to_bgra(fmt, &px, 1, 1, 4, None).unwrap();
            assert_eq!(f.bgra, vec![3, 2, 1, 255], "{fmt:?}");
        }
        let f = to_bgra(PixelFormat::Rgb, &[1, 2, 3], 1, 1, 3, None).unwrap();
        assert_eq!(f.bgra, vec![3, 2, 1, 255]);
        let f = to_bgra(PixelFormat::Bgr, &[3, 2, 1], 1, 1, 3, None).unwrap();
        assert_eq!(f.bgra, vec![3, 2, 1, 255]);
    }

    #[test]
    fn stride_padding_is_dropped() {
        // 2x2, stride 12 (4 bytes padding per row).
        let mut src = vec![0u8; 24];
        src[0..4].copy_from_slice(&[1, 1, 1, 0]);
        src[4..8].copy_from_slice(&[2, 2, 2, 0]);
        src[12..16].copy_from_slice(&[3, 3, 3, 0]);
        src[16..20].copy_from_slice(&[4, 4, 4, 0]);
        let f = to_bgra(PixelFormat::Bgrx, &src, 2, 2, 12, None).unwrap();
        assert_eq!(f.bgra.len(), 16);
        assert_eq!(&f.bgra[8..12], &[3, 3, 3, 255]);
        assert_eq!(&f.bgra[12..16], &[4, 4, 4, 255]);
    }

    #[test]
    fn crop_cuts_the_visible_part() {
        let mut src = vec![0u8; 4 * 4 * 4];
        for (i, px) in src.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            px[0] = i as u8;
        }
        let crop = Crop {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        };
        let f = to_bgra(PixelFormat::Bgrx, &src, 4, 4, 16, Some(crop)).unwrap();
        assert_eq!((f.width, f.height), (2, 2));
        assert_eq!(f.bgra[0], 5);
        assert_eq!(f.bgra[4], 6);
        assert_eq!(f.bgra[8], 9);
        assert_eq!(f.bgra[12], 10);
    }

    #[test]
    fn a_crop_outside_the_buffer_is_ignored() {
        let src = vec![0u8; 2 * 2 * 4];
        let crop = Crop {
            x: 1,
            y: 0,
            width: 5,
            height: 1,
        };
        let f = to_bgra(PixelFormat::Bgrx, &src, 2, 2, 8, Some(crop)).unwrap();
        assert_eq!((f.width, f.height), (2, 2));
    }

    #[test]
    fn short_buffers_and_bad_sizes_are_errors() {
        assert!(to_bgra(PixelFormat::Bgrx, &[0; 7], 2, 1, 8, None).is_err());
        assert!(to_bgra(PixelFormat::Bgrx, &[0; 8], 2, 1, 4, None).is_err());
        assert!(to_bgra(PixelFormat::Bgrx, &[], 0, 1, 0, None).is_err());
        assert!(check_size(MAX_SIDE + 1, 1).is_err());
        assert!(check_size(8_192, 8_192).is_err());
        assert!(check_size(7_680, 4_320).is_ok());
    }

    #[test]
    fn the_last_row_needs_no_padding() {
        // 1x2, stride 8: the second row only needs 4 bytes.
        let src = [1, 2, 3, 0, 0, 0, 0, 0, 4, 5, 6, 0];
        let f = to_bgra(PixelFormat::Bgrx, &src, 1, 2, 8, None).unwrap();
        assert_eq!(&f.bgra[4..8], &[4, 5, 6, 255]);
    }
}
