// SPDX-License-Identifier: GPL-3.0-or-later
//! Video codecs for Comet Cast: an H.264 encoder and decoder built on OpenH264, a helper
//! for "static refresh", and a trait for a future hardware-encoder probe.
//!
//! Plan: H.264 everywhere, preferably in platform hardware, with an OpenH264 fallback on
//! desktop. Mobile platforms encode natively and only hand finished H.264 units to the core.
//! This crate implements the software fallback only.
//!
//! * [`h264::H264Encoder`]: BGRA frames in, Annex-B H.264 out (Constrained-Baseline-style
//!   settings that browsers accept), key frames on request.
//! * [`h264::H264Decoder`]: only for tests and the command line tool, to prove that a
//!   received stream decodes. It is not a playback path.
//! * [`rtp::H264Assembler`]: puts received RTP payloads back into access units (tests/CLI).
//! * [`refresh::StaticRefresh`]: decides when to re-encode the last picture after the screen
//!   stopped changing.
//! * [`hwtest::HardwareEncoderProbe`]: trait only; no hardware encoder exists yet.
//!
//! UNCERTAIN: H.264 patent and licence questions are not clarified; see docs/capture.md,
//! section "H.264 licensing".

pub mod h264;
pub mod hwtest;
pub mod refresh;
pub mod rtp;

pub use h264::{
    DecodedFrame, H264Backend, H264Config, H264Decoder, H264Encoder, MediaError, annexb_nal_types,
};

/// One encoded video frame (for example one H.264 access unit in Annex-B form).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedFrame {
    /// The compressed bytes.
    pub data: Vec<u8>,
    /// True if the frame can be decoded without earlier frames.
    pub is_keyframe: bool,
    /// Presentation time in microseconds.
    pub timestamp_us: u64,
}

/// A borrowed raw picture in BGRA byte order (4 bytes per pixel, rows without padding).
#[derive(Debug, Clone, Copy)]
pub struct BgraFrame<'a> {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel bytes, length = width * height * 4.
    pub bgra: &'a [u8],
}

/// Something that turns raw video frames into compressed ones.
pub trait VideoEncoder {
    /// Error type of this encoder.
    type Error;

    /// Encode one raw frame. `None` means the encoder chose to drop the frame.
    fn encode(
        &mut self,
        frame: BgraFrame<'_>,
        timestamp_us: u64,
    ) -> Result<Option<EncodedFrame>, Self::Error>;

    /// Make the next frame a key frame (new receiver, picture loss indication).
    fn request_keyframe(&mut self);
}
