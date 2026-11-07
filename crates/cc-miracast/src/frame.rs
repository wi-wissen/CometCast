// SPDX-License-Identifier: GPL-3.0-or-later
//! Decoded pictures of the incoming stream and the one-slot mailbox that hands them to the UI.
//!
//! Pull model, same idea as the video path of `cc-ffi` (`crates/cc-ffi/src/video.rs`): the
//! player thread publishes the newest picture, the UI takes it when it is ready. A picture the
//! UI has not taken yet is replaced (and counted as dropped), so memory stays bounded by
//! one pending picture plus the one being built, however slow the UI is.
//!
//! Pixel format: BGRA, 8 bits per channel, rows without padding. This is what Windows hands
//! out (`SoftwareBitmap` `Bgra8`) and what Flutter's `decodeImageFromPixels` accepts.
//! Everything in this module is platform independent and unit-tested on Linux.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Largest picture width handed to the UI (pictures are scaled down to fit).
pub const DEFAULT_MAX_WIDTH: u32 = 1920;
/// Largest picture height handed to the UI.
pub const DEFAULT_MAX_HEIGHT: u32 = 1080;
/// Bytes per pixel (BGRA).
pub const BYTES_PER_PIXEL: usize = 4;
/// Upper bound for a requested maximum size, so a wrong value cannot ask for gigabytes.
pub const HARD_MAX_DIMENSION: u32 = 4096;

/// One picture, BGRA, rows without padding. `seq` grows by one per published picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
    /// Sequence number, starts at 1. A UI that remembers the last one it took can tell
    /// whether a picture is new.
    pub seq: u64,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub bgra: Vec<u8>,
}

/// Counters of the picture path, for the statistics line and the device test.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameCounters {
    /// Pictures published by the player.
    pub published: u64,
    /// Pictures taken by the UI.
    pub taken: u64,
    /// Pictures replaced before the UI took them.
    pub dropped: u64,
    /// Pictures the player could not copy or convert (errors).
    pub failed: u64,
    /// Size of the newest picture (after scaling), 0 if none yet.
    pub width: u32,
    /// Size of the newest picture (after scaling), 0 if none yet.
    pub height: u32,
}

#[derive(Default)]
struct Slot {
    latest: Option<VideoFrame>,
    seq: u64,
    counters: FrameCounters,
}

/// One slot, newest picture wins.
#[derive(Default)]
pub struct FrameMailbox {
    slot: Mutex<Slot>,
}

impl FrameMailbox {
    fn lock(&self) -> MutexGuard<'_, Slot> {
        self.slot.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Store a new picture; a picture nobody took yet is replaced and counted as dropped.
    /// Returns the sequence number given to it. `bgra` must be `width * height * 4` bytes
    /// (otherwise the picture is refused and counted as failed; returns 0).
    pub fn publish(&self, width: u32, height: u32, bgra: Vec<u8>) -> u64 {
        let mut s = self.lock();
        if width == 0
            || height == 0
            || bgra.len() != (width as usize) * (height as usize) * BYTES_PER_PIXEL
        {
            s.counters.failed += 1;
            return 0;
        }
        if s.latest.is_some() {
            s.counters.dropped += 1;
        }
        s.seq += 1;
        let seq = s.seq;
        s.latest = Some(VideoFrame {
            seq,
            width,
            height,
            bgra,
        });
        s.counters.published += 1;
        s.counters.width = width;
        s.counters.height = height;
        seq
    }

    /// Count a picture that could not be produced.
    pub fn note_failure(&self) {
        self.lock().counters.failed += 1;
    }

    /// Take the newest picture (removing it from the slot), if there is one.
    pub fn take(&self) -> Option<VideoFrame> {
        let mut s = self.lock();
        let f = s.latest.take();
        if f.is_some() {
            s.counters.taken += 1;
        }
        f
    }

    /// Size of the pending picture without taking it (so the UI can size its buffer).
    #[must_use]
    pub fn peek_size(&self) -> Option<(u32, u32)> {
        self.lock().latest.as_ref().map(|f| (f.width, f.height))
    }

    /// Forget the pending picture (a stream ended); counters are kept.
    pub fn clear(&self) {
        self.lock().latest = None;
    }

    /// Copy of the counters.
    #[must_use]
    pub fn counters(&self) -> FrameCounters {
        self.lock().counters
    }

    /// Reset everything (a new session starts): no picture, counters at zero, sequence at zero.
    pub fn reset(&self) {
        *self.lock() = Slot::default();
    }
}

/// The size a `width` x `height` picture gets when it must fit into `max_w` x `max_h`,
/// keeping the aspect ratio. Never enlarges. Both sides are at least 1 pixel; returns
/// `(0, 0)` for an empty source.
#[must_use]
pub fn fit_size(width: u32, height: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (0, 0);
    }
    let max_w = max_w.clamp(1, HARD_MAX_DIMENSION);
    let max_h = max_h.clamp(1, HARD_MAX_DIMENSION);
    if width <= max_w && height <= max_h {
        return (width, height);
    }
    // Compare width/max_w with height/max_h without floating point.
    let (w, h, mw, mh) = (
        u64::from(width),
        u64::from(height),
        u64::from(max_w),
        u64::from(max_h),
    );
    if w * mh >= h * mw {
        // Width is the limiting side.
        let nh = (h * mw / w).max(1);
        (max_w, u32::try_from(nh).unwrap_or(max_h).min(max_h))
    } else {
        let nw = (w * mh / h).max(1);
        (u32::try_from(nw).unwrap_or(max_w).min(max_w), max_h)
    }
}

/// Scale a BGRA picture to `dst_w` x `dst_h` with nearest-neighbour sampling.
///
/// Returns `None` if the sizes do not match the data or a size is 0. Nearest-neighbour is
/// cheap and keeps text edges crisp enough; a better filter is not needed until a device
/// test shows blurry text (UNCERTAIN: not looked at on real content).
#[must_use]
pub fn scale_bgra_nearest(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> Option<Vec<u8>> {
    let (sw, sh, dw, dh) = (
        src_w as usize,
        src_h as usize,
        dst_w as usize,
        dst_h as usize,
    );
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 || src.len() != sw * sh * BYTES_PER_PIXEL {
        return None;
    }
    if sw == dw && sh == dh {
        return Some(src.to_vec());
    }
    let mut out = vec![0u8; dw * dh * BYTES_PER_PIXEL];
    // Sample at the centre of each destination pixel.
    let xmap: Vec<usize> = (0..dw)
        .map(|x| ((x * 2 + 1) * sw / (dw * 2)).min(sw - 1))
        .collect();
    for y in 0..dh {
        let sy = ((y * 2 + 1) * sh / (dh * 2)).min(sh - 1);
        let srow = &src[sy * sw * BYTES_PER_PIXEL..(sy + 1) * sw * BYTES_PER_PIXEL];
        let drow = &mut out[y * dw * BYTES_PER_PIXEL..(y + 1) * dw * BYTES_PER_PIXEL];
        for (x, sx) in xmap.iter().enumerate() {
            drow[x * BYTES_PER_PIXEL..(x + 1) * BYTES_PER_PIXEL]
                .copy_from_slice(&srow[sx * BYTES_PER_PIXEL..(sx + 1) * BYTES_PER_PIXEL]);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(n: usize) -> Vec<u8> {
        vec![7; n * BYTES_PER_PIXEL]
    }

    #[test]
    fn newest_picture_wins_and_drops_are_counted() {
        let m = FrameMailbox::default();
        assert!(m.take().is_none());
        assert_eq!(m.publish(2, 1, px(2)), 1);
        assert_eq!(m.publish(2, 1, px(2)), 2);
        let f = m.take().unwrap();
        assert_eq!(f.seq, 2);
        assert!(m.take().is_none());
        let c = m.counters();
        assert_eq!((c.published, c.taken, c.dropped), (2, 1, 1));
        assert_eq!((c.width, c.height), (2, 1));
    }

    #[test]
    fn wrong_length_is_refused() {
        let m = FrameMailbox::default();
        assert_eq!(m.publish(2, 2, px(3)), 0);
        assert_eq!(m.publish(0, 2, vec![]), 0);
        assert!(m.take().is_none());
        assert_eq!(m.counters().failed, 2);
    }

    #[test]
    fn peek_clear_reset() {
        let m = FrameMailbox::default();
        m.publish(3, 2, px(6));
        assert_eq!(m.peek_size(), Some((3, 2)));
        m.clear();
        assert_eq!(m.peek_size(), None);
        assert_eq!(m.counters().published, 1);
        m.reset();
        assert_eq!(m.counters(), FrameCounters::default());
        assert_eq!(m.publish(1, 1, px(1)), 1);
    }

    #[test]
    fn fit_keeps_aspect_and_never_enlarges() {
        assert_eq!(fit_size(1280, 720, 1920, 1080), (1280, 720));
        assert_eq!(fit_size(3840, 2160, 1920, 1080), (1920, 1080));
        assert_eq!(fit_size(2560, 1600, 1920, 1080), (1728, 1080));
        // Portrait (a phone held upright).
        assert_eq!(fit_size(1080, 2400, 1920, 1080), (486, 1080));
        assert_eq!(fit_size(0, 5, 10, 10), (0, 0));
        // A strip never becomes 0 pixels high.
        assert_eq!(fit_size(10_000, 1, 100, 100), (100, 1));
        // A silly maximum is clamped.
        assert_eq!(fit_size(5000, 5000, 0, 0), (1, 1));
    }

    #[test]
    fn scale_down_samples_the_right_pixels() {
        // 4x2 source, left half red-ish (1), right half (2).
        let mut src = Vec::new();
        for _ in 0..2 {
            for x in 0..4u8 {
                src.extend_from_slice(&[x, 0, 0, 255]);
            }
        }
        let out = scale_bgra_nearest(&src, 4, 2, 2, 1).unwrap();
        assert_eq!(out.len(), 2 * BYTES_PER_PIXEL);
        // centres of the destination pixels map to source x = 1 and 3
        assert_eq!(out[0], 1);
        assert_eq!(out[4], 3);
    }

    #[test]
    fn scale_same_size_copies_and_bad_input_is_none() {
        let src = px(4);
        assert_eq!(scale_bgra_nearest(&src, 2, 2, 2, 2).unwrap(), src);
        assert!(scale_bgra_nearest(&src, 2, 2, 0, 2).is_none());
        assert!(scale_bgra_nearest(&src, 3, 2, 1, 1).is_none());
    }
}
