// SPDX-License-Identifier: GPL-3.0-or-later
//! H.264 encoder and decoder on top of the `openh264` crate (BSD-2-Clause bindings to Cisco's
//! OpenH264).
//!
//! Two ways to get the OpenH264 code, chosen by Cargo features of this crate:
//!
//! * `h264-source`: the crate compiles OpenH264 from source (needs a C++ compiler and `nasm`).
//! * `h264-libloading`: the program loads Cisco's prebuilt shared library at run time from a
//!   path given in [`H264Backend::Library`]. The `openh264` crate only accepts well-known
//!   builds (it checks the file's SHA-256).
//!
//! UNCERTAIN: patents and licences. Cisco's FAQ says that the patent licence it grants covers
//! the binary that Cisco itself builds and distributes, not a build from source; whether that
//! matters for Comet Cast, and whether downloading the binary "during installation" is enough,
//! has not been checked by a lawyer. See docs/capture.md.

use std::path::PathBuf;

use openh264::Timestamp;
use openh264::decoder::{Decoder, DecoderConfig};
use openh264::encoder::{
    BitRate, Encoder, EncoderConfig, FrameRate, FrameType, IntraFramePeriod, Profile,
    RateControlMode, UsageType,
};
use openh264::formats::{BgraSliceU8, YUVBuffer};

use crate::{BgraFrame, EncodedFrame, VideoEncoder};

/// Errors of the H.264 encoder and decoder.
#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    /// OpenH264 reported an error.
    #[error("OpenH264: {0}")]
    OpenH264(String),
    /// The picture does not fit what was promised (size, buffer length).
    #[error("invalid picture: {0}")]
    InvalidPicture(&'static str),
    /// The requested way to get OpenH264 was not compiled in.
    #[error("OpenH264 backend not available: {0}")]
    BackendUnavailable(&'static str),
}

impl From<openh264::Error> for MediaError {
    fn from(e: openh264::Error) -> Self {
        Self::OpenH264(e.to_string())
    }
}

/// Where the OpenH264 code comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum H264Backend {
    /// Compiled into this program (Cargo feature `h264-source`).
    Source,
    /// Cisco's prebuilt shared library at this path (Cargo feature `h264-libloading`).
    Library(PathBuf),
}

impl H264Backend {
    /// Name of the environment variable that selects [`H264Backend::Library`].
    pub const LIBRARY_ENV: &'static str = "COMETCAST_OPENH264_LIB";

    /// `Library(path)` if [`Self::LIBRARY_ENV`] is set, else `Source`.
    pub fn from_env() -> Self {
        match std::env::var_os(Self::LIBRARY_ENV) {
            Some(p) if !p.is_empty() => Self::Library(PathBuf::from(p)),
            _ => Self::Source,
        }
    }

    fn api(&self) -> Result<openh264::OpenH264API, MediaError> {
        match self {
            #[cfg(feature = "h264-source")]
            Self::Source => Ok(openh264::OpenH264API::from_source()),
            #[cfg(not(feature = "h264-source"))]
            Self::Source => Err(MediaError::BackendUnavailable(
                "built without the h264-source feature",
            )),
            #[cfg(feature = "h264-libloading")]
            Self::Library(path) => openh264::OpenH264API::from_blob_path(path)
                .map_err(|e| MediaError::OpenH264(e.to_string())),
            #[cfg(not(feature = "h264-libloading"))]
            Self::Library(_) => Err(MediaError::BackendUnavailable(
                "built without the h264-libloading feature",
            )),
        }
    }
}

/// Encoder settings.
#[derive(Debug, Clone, PartialEq)]
pub struct H264Config {
    /// Where OpenH264 comes from.
    pub backend: H264Backend,
    /// Target bit rate in bits per second.
    pub bitrate_bps: u32,
    /// Maximum frame rate in Hz (the encoder's rate control uses it).
    pub fps: f32,
    /// Let the encoder drop pictures to hold the bit rate. With `false`, every picture
    /// produces output but OpenH264 then does not control the bit rate (it says so in its log).
    pub allow_frame_skip: bool,
}

impl Default for H264Config {
    /// UNCERTAIN: 2 Mbit/s and 15 fps are starting values, not tuned or measured.
    fn default() -> Self {
        Self {
            backend: H264Backend::from_env(),
            bitrate_bps: 2_000_000,
            fps: 15.0,
            allow_frame_skip: true,
        }
    }
}

/// H.264 encoder for BGRA pictures.
///
/// Output is Annex-B (start codes). The first picture and every picture after
/// [`H264Encoder::request_keyframe`] is an IDR frame with SPS and PPS in front, so any
/// receiver that gets such a frame can start decoding. There are no periodic key frames.
pub struct H264Encoder {
    inner: Encoder,
    yuv: Option<YUVBuffer>,
    cropped: Vec<u8>,
}

impl std::fmt::Debug for H264Encoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("H264Encoder").finish_non_exhaustive()
    }
}

impl H264Encoder {
    /// Create an encoder. Size is taken from the first picture.
    pub fn new(config: &H264Config) -> Result<Self, MediaError> {
        let api = config.backend.api()?;
        // Settings chosen for browsers and for screen content: Baseline profile (what
        // WebRTC's H.264 profile-level-id 42xx1f announces), screen-content mode, no periodic
        // key frames. Frame skipping stays on: OpenH264 only controls the bit rate with it, so
        // `encode` can return `None` for a picture the rate control dropped.
        // UNCERTAIN: whether OpenH264's "Baseline" output satisfies every browser's decoder
        // was only tested with the Chromium that ships with the e2e tests (docs/capture.md).
        let enc_config = EncoderConfig::new()
            .bitrate(BitRate::from_bps(config.bitrate_bps))
            .max_frame_rate(FrameRate::from_hz(config.fps))
            .usage_type(UsageType::ScreenContentRealTime)
            .rate_control_mode(RateControlMode::Bitrate)
            .profile(Profile::Baseline)
            .skip_frames(config.allow_frame_skip)
            .intra_frame_period(IntraFramePeriod::from_num_frames(0));
        let inner = Encoder::with_api_config(api, enc_config)?;
        Ok(Self {
            inner,
            yuv: None,
            cropped: Vec::new(),
        })
    }

    /// Encode one picture (see [`VideoEncoder::encode`]).
    ///
    /// Odd widths or heights lose their last column or row (4:2:0 needs even sizes).
    pub fn encode_bgra(
        &mut self,
        frame: BgraFrame<'_>,
        timestamp_us: u64,
    ) -> Result<Option<EncodedFrame>, MediaError> {
        let (w, h) = (frame.width as usize, frame.height as usize);
        if w < 2 || h < 2 {
            return Err(MediaError::InvalidPicture("smaller than 2x2"));
        }
        if frame.bgra.len() != w * h * 4 {
            return Err(MediaError::InvalidPicture("buffer length is not w*h*4"));
        }
        let (ew, eh) = (w & !1, h & !1);
        let data: &[u8] = if (ew, eh) == (w, h) {
            frame.bgra
        } else {
            self.cropped.clear();
            for row in frame.bgra.chunks_exact(w * 4).take(eh) {
                self.cropped.extend_from_slice(&row[..ew * 4]);
            }
            &self.cropped
        };
        let yuv = match &mut self.yuv {
            Some(y) if openh264::formats::YUVSource::dimensions(y) == (ew, eh) => y,
            slot => slot.insert(YUVBuffer::new(ew, eh)),
        };
        yuv.read_bgra8(BgraSliceU8::new(data, (ew, eh)));
        let ts = Timestamp::from_millis(timestamp_us / 1000);
        let stream = self.inner.encode_at(yuv, ts)?;
        let kind = stream.frame_type();
        if matches!(kind, FrameType::Skip | FrameType::Invalid) {
            return Ok(None);
        }
        let data = stream.to_vec();
        if data.is_empty() {
            return Ok(None);
        }
        Ok(Some(EncodedFrame {
            data,
            is_keyframe: matches!(kind, FrameType::IDR | FrameType::I),
            timestamp_us,
        }))
    }

    /// Make the next encoded picture an IDR key frame.
    pub fn force_keyframe(&mut self) {
        self.inner.force_intra_frame();
    }
}

impl VideoEncoder for H264Encoder {
    type Error = MediaError;

    fn encode(
        &mut self,
        frame: BgraFrame<'_>,
        timestamp_us: u64,
    ) -> Result<Option<EncodedFrame>, MediaError> {
        self.encode_bgra(frame, timestamp_us)
    }

    fn request_keyframe(&mut self) {
        self.force_keyframe();
    }
}

/// One decoded picture in RGB (3 bytes per pixel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// R, G, B bytes, length = width * height * 3.
    pub rgb: Vec<u8>,
}

impl DecodedFrame {
    /// Mean colour over the whole picture.
    pub fn mean_rgb(&self) -> [f64; 3] {
        let mut sum = [0u64; 3];
        for px in self.rgb.chunks_exact(3) {
            for (s, c) in sum.iter_mut().zip(px) {
                *s += u64::from(*c);
            }
        }
        let n = (self.rgb.len() / 3).max(1) as f64;
        [sum[0] as f64 / n, sum[1] as f64 / n, sum[2] as f64 / n]
    }

    /// Share (0..1) of pixels whose channels are all within `tolerance` of `rgb`.
    pub fn share_near(&self, rgb: [u8; 3], tolerance: u8) -> f64 {
        let total = self.rgb.len() / 3;
        if total == 0 {
            return 0.0;
        }
        let hits = self
            .rgb
            .chunks_exact(3)
            .filter(|px| px.iter().zip(rgb).all(|(a, b)| a.abs_diff(b) <= tolerance))
            .count();
        hits as f64 / total as f64
    }
}

/// H.264 decoder. Meant for tests and the command line tool only: it proves that a stream can
/// be decoded. It is not used for showing video.
pub struct H264Decoder {
    inner: Decoder,
}

impl std::fmt::Debug for H264Decoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("H264Decoder").finish_non_exhaustive()
    }
}

impl H264Decoder {
    /// Create a decoder.
    pub fn new(backend: &H264Backend) -> Result<Self, MediaError> {
        let inner = Decoder::with_api_config(backend.api()?, DecoderConfig::new())?;
        Ok(Self { inner })
    }

    /// Decode one Annex-B access unit. `None` means no picture came out yet (for example the
    /// data was only SPS/PPS, or the decoder is still waiting for a key frame).
    pub fn decode(&mut self, annexb: &[u8]) -> Result<Option<DecodedFrame>, MediaError> {
        let Some(yuv) = self.inner.decode(annexb)? else {
            return Ok(None);
        };
        let (w, h) = openh264::formats::YUVSource::dimensions(&yuv);
        let mut rgb = vec![0u8; w * h * 3];
        yuv.write_rgb8(&mut rgb);
        Ok(Some(DecodedFrame {
            width: w as u32,
            height: h as u32,
            rgb,
        }))
    }
}

/// NAL unit types (the low 5 bits of the NAL header) found in an Annex-B byte stream, in
/// order. 7 = SPS, 8 = PPS, 5 = IDR slice, 1 = non-IDR slice.
pub fn annexb_nal_types(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 < data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            if let Some(h) = data.get(i + 3) {
                out.push(h & 0x1f);
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A solid BGRA picture.
    fn solid(w: u32, h: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..w * h {
            v.extend_from_slice(&[b, g, r, 255]);
        }
        v
    }

    /// Source build when compiled in, otherwise the Cisco library named by the environment.
    fn backend() -> H264Backend {
        if cfg!(feature = "h264-source") {
            H264Backend::Source
        } else {
            H264Backend::from_env()
        }
    }

    fn cfg() -> H264Config {
        H264Config {
            backend: backend(),
            ..H264Config::default()
        }
    }

    #[test]
    fn nal_types_are_found() {
        let s = [
            0, 0, 0, 1, 0x67, 1, 2, 0, 0, 1, 0x68, 3, 0, 0, 0, 1, 0x65, 9,
        ];
        assert_eq!(annexb_nal_types(&s), vec![7, 8, 5]);
        assert!(annexb_nal_types(&[]).is_empty());
    }

    #[test]
    fn round_trip_keeps_the_colour() {
        let (w, h) = (320, 240);
        let mut enc = H264Encoder::new(&cfg()).unwrap();
        let mut dec = H264Decoder::new(&backend()).unwrap();
        for (r, g, b) in [(200u8, 30u8, 40u8), (20, 160, 220), (240, 240, 20)] {
            // A new colour is a big change; force an IDR so each round trip is self-contained.
            enc.force_keyframe();
            let px = solid(w, h, r, g, b);
            let f = enc
                .encode_bgra(
                    BgraFrame {
                        width: w,
                        height: h,
                        bgra: &px,
                    },
                    0,
                )
                .unwrap()
                .expect("a frame");
            assert!(f.is_keyframe);
            let types = annexb_nal_types(&f.data);
            assert!(
                types.contains(&7) && types.contains(&8) && types.contains(&5),
                "{types:?}"
            );
            let pic = dec.decode(&f.data).unwrap().expect("a picture");
            assert_eq!((pic.width, pic.height), (w, h));
            let m = pic.mean_rgb();
            for (got, want) in m.iter().zip([r, g, b]) {
                assert!(
                    (got - f64::from(want)).abs() < 12.0,
                    "mean {m:?} vs {r},{g},{b}"
                );
            }
            assert!(pic.share_near([r, g, b], 20) > 0.95);
        }
    }

    #[test]
    fn keyframe_only_on_request_and_odd_sizes_are_cropped() {
        let (w, h) = (101, 75);
        let px = solid(w, h, 10, 200, 10);
        let mut enc = H264Encoder::new(&cfg()).unwrap();
        let mut dec = H264Decoder::new(&backend()).unwrap();
        let frame = BgraFrame {
            width: w,
            height: h,
            bgra: &px,
        };
        let first = enc.encode_bgra(frame, 0).unwrap().unwrap();
        assert!(first.is_keyframe);
        let second = enc.encode_bgra(frame, 66_000).unwrap().unwrap();
        assert!(!second.is_keyframe, "no periodic key frames");
        enc.request_keyframe();
        let third = enc.encode_bgra(frame, 133_000).unwrap().unwrap();
        assert!(third.is_keyframe, "key frame on request");
        let pic = dec.decode(&first.data).unwrap().unwrap();
        assert_eq!((pic.width, pic.height), (100, 74));
    }

    /// Returns (profile_idc, constraint flags byte, level_idc) of the first SPS.
    fn sps_profile(annexb: &[u8]) -> (u8, u8, u8) {
        let i = annexb
            .windows(4)
            .position(|w| w == [0, 0, 0, 1] || w[..3] == [0, 0, 1] && w[3] & 0x1f == 7)
            .expect("start code");
        let start = if annexb[i + 2] == 1 { i + 3 } else { i + 4 };
        assert_eq!(annexb[start] & 0x1f, 7, "not an SPS");
        (annexb[start + 1], annexb[start + 2], annexb[start + 3])
    }

    #[test]
    fn the_stream_is_baseline_profile() {
        for (w, h) in [(1280, 720), (1920, 1080)] {
            let px = solid(w, h, 10, 20, 30);
            let mut enc = H264Encoder::new(&cfg()).unwrap();
            let f = enc
                .encode_bgra(
                    BgraFrame {
                        width: w,
                        height: h,
                        bgra: &px,
                    },
                    0,
                )
                .unwrap()
                .unwrap();
            let (profile, constraints, level) = sps_profile(&f.data);
            eprintln!(
                "SPS {w}x{h}: profile_idc={profile} constraint_flags={constraints:#04x} level_idc={level}"
            );
            // Baseline family: profile_idc 66. UNCERTAIN: browsers match on the
            // profile-level-id of the SDP (42e01f); the level the encoder writes is only
            // printed here.
            assert_eq!(profile, 66);
        }
    }

    /// Experiment, not a guarantee: does encoding the same picture again make it closer to the
    /// original? Prints the PSNR (dB, over RGB bytes) after each repeat at a low bit rate.
    #[test]
    fn repeated_encoding_of_a_still_picture_is_measured() {
        let (w, h) = (640u32, 360u32);
        // Text-like content: dark 6x10 "glyph" blocks with pseudo-random pixels on light grey.
        let mut px = vec![0u8; (w * h * 4) as usize];
        let mut seed = 12345u32;
        for y in 0..h {
            for x in 0..w {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let glyph = (x / 6 + (y / 10) * 7) % 3 != 0;
                let on = glyph && (seed >> 24) % 5 < 2;
                let v = if on { 20 } else { 235 };
                let i = ((y * w + x) * 4) as usize;
                px[i..i + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        for (bitrate_bps, allow_frame_skip) in [
            (300_000, true),
            (300_000, false),
            (2_000_000, true),
            (2_000_000, false),
        ] {
            let c = H264Config {
                bitrate_bps,
                allow_frame_skip,
                ..cfg()
            };
            let mut enc = H264Encoder::new(&c).unwrap();
            let mut dec = H264Decoder::new(&backend()).unwrap();
            let frame = BgraFrame {
                width: w,
                height: h,
                bgra: &px,
            };
            let mut psnrs = Vec::new();
            let mut sizes = Vec::new();
            for n in 0..6u64 {
                if n >= 3 {
                    // Second half of the run: ask for an IDR frame each time.
                    enc.force_keyframe();
                }
                let Some(f) = enc.encode_bgra(frame, n * 100_000).unwrap() else {
                    sizes.push(0);
                    continue;
                };
                sizes.push(f.data.len());
                let pic = dec.decode(&f.data).unwrap().unwrap();
                let mut se = 0f64;
                for (i, d) in pic.rgb.chunks_exact(3).enumerate() {
                    let want = px[i * 4 + 2];
                    se += (f64::from(d[0]) - f64::from(want)).powi(2);
                }
                let mse = se / (pic.rgb.len() / 3) as f64;
                psnrs.push(10.0 * (255.0f64 * 255.0 / mse.max(1e-9)).log10());
            }
            eprintln!(
                "still picture, {bitrate_bps} bit/s, frame skip {allow_frame_skip}: sizes {sizes:?} (IDR requested for the last three), PSNR(dB) of the pictures that came out {psnrs:.1?}"
            );
            assert!(!psnrs.is_empty());
        }
    }

    #[test]
    fn bad_buffers_are_rejected() {
        let mut enc = H264Encoder::new(&cfg()).unwrap();
        let err = enc.encode_bgra(
            BgraFrame {
                width: 4,
                height: 4,
                bgra: &[0; 10],
            },
            0,
        );
        assert!(matches!(err, Err(MediaError::InvalidPicture(_))));
    }

    /// With `--no-default-features --features h264-libloading` and the Cisco library given in
    /// COMETCAST_OPENH264_LIB, the same round trip works through the prebuilt binary.
    #[cfg(feature = "h264-libloading")]
    #[test]
    fn prebuilt_library_round_trip() {
        let Some(path) = std::env::var_os(H264Backend::LIBRARY_ENV) else {
            eprintln!("NOTE: SKIPPING, set COMETCAST_OPENH264_LIB to Cisco's libopenh264 .so");
            return;
        };
        let backend = H264Backend::Library(path.into());
        let c = H264Config {
            backend: backend.clone(),
            ..H264Config::default()
        };
        let mut enc = H264Encoder::new(&c).unwrap();
        let mut dec = H264Decoder::new(&backend).unwrap();
        let px = solid(320, 240, 200, 30, 40);
        let f = enc
            .encode_bgra(
                BgraFrame {
                    width: 320,
                    height: 240,
                    bgra: &px,
                },
                0,
            )
            .unwrap()
            .unwrap();
        let pic = dec.decode(&f.data).unwrap().unwrap();
        assert!(pic.share_near([200, 30, 40], 20) > 0.95);
        // A file that is not a known Cisco build is refused.
        assert!(
            H264Encoder::new(&H264Config {
                backend: H264Backend::Library("/bin/ls".into()),
                ..H264Config::default()
            })
            .is_err()
        );
    }

    #[cfg(not(feature = "h264-libloading"))]
    #[test]
    fn library_backend_needs_its_feature() {
        let c = H264Config {
            backend: H264Backend::Library("/nonexistent.so".into()),
            ..H264Config::default()
        };
        assert!(matches!(
            H264Encoder::new(&c),
            Err(MediaError::BackendUnavailable(_))
        ));
    }
}
