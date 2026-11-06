// SPDX-License-Identifier: GPL-3.0-or-later
//! Cheap inspection of incoming RTP video: counters and key frame detection.
//!
//! There is no decoding here. We only look at the first bytes of the RTP payload.

use std::sync::atomic::{AtomicU64, Ordering};

/// Video codecs the receiving side can recognise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    /// VP8 (RFC 7741 payload format).
    Vp8,
    /// VP9.
    Vp9,
    /// H.264.
    H264,
    /// AV1.
    Av1,
}

impl VideoCodec {
    /// Read a MIME type such as `video/VP8` (case-insensitive).
    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime.to_ascii_lowercase().as_str() {
            "video/vp8" => Some(Self::Vp8),
            "video/vp9" => Some(Self::Vp9),
            "video/h264" => Some(Self::H264),
            "video/av1" => Some(Self::Av1),
            _ => None,
        }
    }

    /// Short name for printing.
    pub fn name(self) -> &'static str {
        match self {
            Self::Vp8 => "VP8",
            Self::Vp9 => "VP9",
            Self::H264 => "H264",
            Self::Av1 => "AV1",
        }
    }
}

/// Does this RTP payload start a key frame?
///
/// UNCERTAIN: this is a quick check of the payload header, not a full parser.
/// VP8 and H.264 follow RFC 7741 / RFC 6184; VP9 only looks at the B and P bits of the
/// first payload descriptor byte and ignores spatial layers; AV1 is not detected
/// (always `false`). A real decoder will replace this.
pub fn starts_key_frame(codec: VideoCodec, payload: &[u8]) -> bool {
    match codec {
        VideoCodec::Vp8 => vp8_starts_key_frame(payload),
        VideoCodec::Vp9 => payload
            .first()
            .is_some_and(|b| b & 0x08 != 0 && b & 0x40 == 0),
        VideoCodec::H264 => h264_has_idr_start(payload),
        VideoCodec::Av1 => false,
    }
}

fn vp8_starts_key_frame(p: &[u8]) -> bool {
    let Some(&b0) = p.first() else { return false };
    // S bit (start of partition) = 0x10, partition index in the low 3 bits must be 0.
    if b0 & 0x10 == 0 || b0 & 0x07 != 0 {
        return false;
    }
    let mut i = 1;
    if b0 & 0x80 != 0 {
        // X bit: extension byte follows with I, L, T, K flags.
        let Some(&x) = p.get(i) else { return false };
        i += 1;
        if x & 0x80 != 0 {
            // PictureID: one byte, or two if its top bit (M) is set.
            let Some(&pid) = p.get(i) else { return false };
            i += if pid & 0x80 != 0 { 2 } else { 1 };
        }
        if x & 0x40 != 0 {
            i += 1; // TL0PICIDX
        }
        if x & 0x20 != 0 || x & 0x10 != 0 {
            i += 1; // TID / KEYIDX
        }
    }
    // First byte of the VP8 frame header: bit 0 is 0 for a key frame.
    p.get(i).is_some_and(|b| b & 0x01 == 0)
}

fn h264_has_idr_start(p: &[u8]) -> bool {
    let Some(&b0) = p.first() else { return false };
    match b0 & 0x1f {
        5 => true,
        // STAP-A: a list of (size, NAL) pairs after the first byte.
        24 => {
            let mut i = 1;
            while i + 2 < p.len() {
                let size = usize::from(p[i]) << 8 | usize::from(p[i + 1]);
                if p.get(i + 2).is_some_and(|n| n & 0x1f == 5) {
                    return true;
                }
                i += 2 + size;
            }
            false
        }
        // FU-A: start bit in the second byte, NAL type in its low 5 bits.
        28 => p.get(1).is_some_and(|b| b & 0x80 != 0 && b & 0x1f == 5),
        _ => false,
    }
}

/// Live counters, written by the packet reader task, read by anyone.
#[derive(Debug, Default)]
pub struct MediaStats {
    packets: AtomicU64,
    bytes: AtomicU64,
    frames: AtomicU64,
    keyframes: AtomicU64,
}

impl MediaStats {
    /// Count one RTP packet of a video track.
    ///
    /// `marker` is the RTP marker bit, which ends a frame for VP8/VP9/H.264.
    pub fn record(&self, payload_len: usize, marker: bool, keyframe_start: bool) {
        self.packets.fetch_add(1, Ordering::Relaxed);
        self.bytes.fetch_add(payload_len as u64, Ordering::Relaxed);
        if marker {
            self.frames.fetch_add(1, Ordering::Relaxed);
        }
        if keyframe_start {
            self.keyframes.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Copy of the current values.
    pub fn snapshot(&self) -> MediaSnapshot {
        MediaSnapshot {
            packets: self.packets.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
            frames: self.frames.load(Ordering::Relaxed),
            keyframes: self.keyframes.load(Ordering::Relaxed),
        }
    }
}

/// A point-in-time copy of [`MediaStats`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MediaSnapshot {
    /// RTP packets received.
    pub packets: u64,
    /// RTP payload bytes received (headers not counted).
    pub bytes: u64,
    /// Frames, counted as packets with the RTP marker bit set.
    /// UNCERTAIN: assumes one marker per frame, which holds for plain VP8/VP9/H.264.
    pub frames: u64,
    /// Key frames detected by [`starts_key_frame`] (see its limits).
    pub keyframes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vp8_key_and_delta_frames() {
        // Minimal descriptor (S=1, no extension) followed by a frame tag.
        assert!(starts_key_frame(VideoCodec::Vp8, &[0x10, 0x10, 0x00, 0x00]));
        assert!(!starts_key_frame(
            VideoCodec::Vp8,
            &[0x10, 0x11, 0x00, 0x00]
        ));
        // Not the start of a partition.
        assert!(!starts_key_frame(VideoCodec::Vp8, &[0x00, 0x10]));
        // With X: PictureID (15 bit, M set) + TL0PICIDX + TID.
        assert!(starts_key_frame(
            VideoCodec::Vp8,
            &[0x90, 0xe0, 0x80, 0x05, 0x01, 0x00, 0x10]
        ));
        assert!(!starts_key_frame(VideoCodec::Vp8, &[]));
    }

    #[test]
    fn h264_idr_detection() {
        assert!(starts_key_frame(VideoCodec::H264, &[0x65, 0x88]));
        assert!(!starts_key_frame(VideoCodec::H264, &[0x41, 0x9a]));
        // FU-A start of an IDR NAL.
        assert!(starts_key_frame(VideoCodec::H264, &[0x7c, 0x85, 0x00]));
        // FU-A middle fragment.
        assert!(!starts_key_frame(VideoCodec::H264, &[0x7c, 0x05, 0x00]));
        // STAP-A with SPS (7) then IDR (5).
        assert!(starts_key_frame(
            VideoCodec::H264,
            &[0x18, 0x00, 0x02, 0x67, 0x00, 0x00, 0x02, 0x65, 0x00]
        ));
    }

    #[test]
    fn stats_count() {
        let s = MediaStats::default();
        s.record(100, false, true);
        s.record(50, true, false);
        assert_eq!(
            s.snapshot(),
            MediaSnapshot {
                packets: 2,
                bytes: 150,
                frames: 1,
                keyframes: 1
            }
        );
    }

    #[test]
    fn codec_from_mime() {
        assert_eq!(VideoCodec::from_mime("video/VP8"), Some(VideoCodec::Vp8));
        assert_eq!(VideoCodec::from_mime("video/h264"), Some(VideoCodec::H264));
        assert_eq!(VideoCodec::from_mime("audio/opus"), None);
    }
}
