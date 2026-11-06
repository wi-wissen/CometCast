// SPDX-License-Identifier: GPL-3.0-or-later
//! Video codec traits. Plan: H.264 everywhere, preferably in platform hardware, with an
//! openh264 fallback on desktop. Mobile platforms encode natively and only hand finished
//! H.264 units to the core.
//!
//! UNCERTAIN: H.264 patent questions for platform hardware encoders are not clarified yet.
//! This crate only defines traits; no encoder is implemented.

/// One encoded video frame (for example one H.264 access unit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedFrame {
    /// The compressed bytes.
    pub data: Vec<u8>,
    /// True if the frame can be decoded without earlier frames.
    pub is_keyframe: bool,
    /// Presentation time in microseconds.
    pub timestamp_us: u64,
}

/// Something that turns raw video frames into compressed ones.
pub trait VideoEncoder {
    /// Error type of this encoder.
    type Error;

    /// Encode one raw frame (pixel format is decided by the implementation).
    fn encode(&mut self, raw_frame: &[u8], timestamp_us: u64) -> Result<Vec<EncodedFrame>, Self::Error>;
}
