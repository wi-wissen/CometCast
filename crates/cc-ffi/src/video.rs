// SPDX-License-Identifier: GPL-3.0-or-later
//! The path from received H.264 packets to RGBA pictures that the Flutter UI draws.
//!
//! ```text
//! cc-transport video tap (RTP payloads)   unbounded channel, filled by the WebRTC task
//!        v
//! decoder thread ("cc-video-decode")      H264Assembler -> OpenH264 -> RGB -> scale -> RGBA
//!        v
//! mailbox (ONE slot, newest wins)         a frame that Dart has not fetched yet is replaced
//!        v                                and counted as dropped
//! FrameReady event -> Dart -> take_frame()
//! ```
//!
//! Why a pull model: a `StreamSink` queues without limit. If the UI is slower than the
//! decoder, a pushed stream of 8 MB pictures would eat memory. Here only a tiny
//! "a frame is ready" event is pushed, and only while the previous one was not yet answered by
//! `take_frame`. So the memory is bounded by one pending picture plus the one being built.
//!
//! Other bounds: the decoder thread works on the packets as they come; if more than
//! [`MAX_BACKLOG_PACKETS`] pile up (decoder slower than the network) the backlog is thrown
//! away, the decoder waits for the next IDR frame and a key frame is requested. Pictures are
//! scaled down to the size the UI asked for (at most [`MAX_WIDTH`] x [`MAX_HEIGHT`]).
//!
//! The hand-written API of this module is `pub(crate)`; `api.rs` exposes what Dart may call.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Largest picture handed to the UI.
pub(crate) const MAX_WIDTH: u32 = 1920;
/// Largest picture handed to the UI.
pub(crate) const MAX_HEIGHT: u32 = 1080;

/// If the decoder thread finds more than this many packets waiting, it drops all of them.
/// UNCERTAIN: placeholder (about 2.4 MB of RTP payload), not tuned.
#[cfg(feature = "h264")]
const MAX_BACKLOG_PACKETS: usize = 2000;

/// At most one key frame request per this long.
const KEYFRAME_MIN_INTERVAL: Duration = Duration::from_secs(1);

/// A decoded, possibly downscaled picture, RGBA, rows without padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Frame {
    pub(crate) seq: u64,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

#[derive(Debug, Default)]
struct MailSlot {
    latest: Option<Frame>,
    /// A `FrameReady` event was sent and `take` has not been called since.
    notified: bool,
    seq: u64,
}

/// Counters of the video path, copied for the UI.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct VideoCounters {
    pub(crate) decoded: u64,
    pub(crate) delivered: u64,
    pub(crate) dropped: u64,
    pub(crate) dropped_packets: u64,
    pub(crate) decode_errors: u64,
    pub(crate) keyframe_requests: u64,
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Mean time OpenH264 + conversion took per decoded picture, in microseconds.
    pub(crate) avg_decode_us: u64,
    pub(crate) error: Option<String>,
}

/// All state of the one running receiver's video path.
pub(crate) struct Video {
    slot: Mutex<MailSlot>,
    max_w: AtomicU32,
    max_h: AtomicU32,
    /// True once the track was announced as H.264; other codecs are not decoded.
    h264: AtomicBool,
    want_keyframe: AtomicBool,
    last_keyframe_request: Mutex<Option<Instant>>,
    /// Bumped on every reset; a decoder thread of an older generation ends.
    generation: AtomicU64,
    decoded: AtomicU64,
    delivered: AtomicU64,
    dropped: AtomicU64,
    dropped_packets: AtomicU64,
    decode_errors: AtomicU64,
    keyframe_requests: AtomicU64,
    last_w: AtomicU32,
    last_h: AtomicU32,
    decode_us_total: AtomicU64,
    error: Mutex<Option<String>>,
}

pub(crate) static VIDEO: Video = Video::new();

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // A poisoned lock only means another thread panicked; the data is still usable.
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Video {
    pub(crate) const fn new() -> Self {
        Self {
            slot: Mutex::new(MailSlot {
                latest: None,
                notified: false,
                seq: 0,
            }),
            max_w: AtomicU32::new(MAX_WIDTH),
            max_h: AtomicU32::new(MAX_HEIGHT),
            h264: AtomicBool::new(false),
            want_keyframe: AtomicBool::new(false),
            last_keyframe_request: Mutex::new(None),
            generation: AtomicU64::new(0),
            decoded: AtomicU64::new(0),
            delivered: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            dropped_packets: AtomicU64::new(0),
            decode_errors: AtomicU64::new(0),
            keyframe_requests: AtomicU64::new(0),
            last_w: AtomicU32::new(0),
            last_h: AtomicU32::new(0),
            decode_us_total: AtomicU64::new(0),
            error: Mutex::new(None),
        }
    }

    /// Forget everything of the previous receiver and return the new generation number.
    /// The size limit set by the UI is kept (it describes the window, not the receiver).
    pub(crate) fn reset(&self) -> u64 {
        let g = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        *lock(&self.slot) = MailSlot::default();
        self.h264.store(false, Ordering::SeqCst);
        self.want_keyframe.store(false, Ordering::SeqCst);
        *lock(&self.last_keyframe_request) = None;
        for c in [
            &self.decoded,
            &self.delivered,
            &self.dropped,
            &self.dropped_packets,
            &self.decode_errors,
            &self.keyframe_requests,
            &self.decode_us_total,
        ] {
            c.store(0, Ordering::Relaxed);
        }
        self.last_w.store(0, Ordering::Relaxed);
        self.last_h.store(0, Ordering::Relaxed);
        *lock(&self.error) = None;
        g
    }

    /// Stop the decoder thread of the current generation (and forget the waiting picture).
    pub(crate) fn stop(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        *lock(&self.slot) = MailSlot::default();
    }

    pub(crate) fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
    }

    /// The UI's drawing area in physical pixels. Pictures are scaled down to fit (never up).
    pub(crate) fn set_max_size(&self, width: u32, height: u32) {
        self.max_w
            .store(width.clamp(1, MAX_WIDTH), Ordering::Relaxed);
        self.max_h
            .store(height.clamp(1, MAX_HEIGHT), Ordering::Relaxed);
    }

    pub(crate) fn max_size(&self) -> (u32, u32) {
        (
            self.max_w.load(Ordering::Relaxed),
            self.max_h.load(Ordering::Relaxed),
        )
    }

    /// The track was announced; only H.264 is decoded. A new H.264 track needs a key frame.
    pub(crate) fn set_codec_is_h264(&self, is_h264: bool) {
        self.h264.store(is_h264, Ordering::SeqCst);
        if is_h264 {
            self.request_keyframe();
        }
    }

    pub(crate) fn is_h264(&self) -> bool {
        self.h264.load(Ordering::SeqCst)
    }

    /// Ask for a key frame (rate limited). The receiver task sends the actual PLI.
    pub(crate) fn request_keyframe(&self) {
        let mut last = lock(&self.last_keyframe_request);
        if last.is_some_and(|t| t.elapsed() < KEYFRAME_MIN_INTERVAL) {
            return;
        }
        *last = Some(Instant::now());
        self.want_keyframe.store(true, Ordering::SeqCst);
    }

    /// True once per request; the caller then sends the PLI.
    pub(crate) fn take_keyframe_request(&self) -> bool {
        let wanted = self.want_keyframe.swap(false, Ordering::SeqCst);
        if wanted {
            self.keyframe_requests.fetch_add(1, Ordering::Relaxed);
        }
        wanted
    }

    /// Put a finished picture into the mailbox. Returns true if the caller must send a
    /// `FrameReady` event (none is outstanding).
    pub(crate) fn publish(&self, width: u32, height: u32, rgba: Vec<u8>) -> bool {
        self.decoded.fetch_add(1, Ordering::Relaxed);
        self.last_w.store(width, Ordering::Relaxed);
        self.last_h.store(height, Ordering::Relaxed);
        let mut slot = lock(&self.slot);
        slot.seq += 1;
        let frame = Frame {
            seq: slot.seq,
            width,
            height,
            rgba,
        };
        if slot.latest.replace(frame).is_some() {
            // Dart did not fetch the previous picture in time.
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
        let send = !slot.notified;
        slot.notified = true;
        send
    }

    /// The newest picture, if Dart has not seen it yet. Also re-arms the `FrameReady` event.
    pub(crate) fn take(&self) -> Option<Frame> {
        let mut slot = lock(&self.slot);
        slot.notified = false;
        let f = slot.latest.take();
        if f.is_some() {
            self.delivered.fetch_add(1, Ordering::Relaxed);
        }
        f
    }

    pub(crate) fn record_error(&self, message: String) {
        *lock(&self.error) = Some(message);
    }

    #[cfg(feature = "h264")]
    fn add_decode_time(&self, d: Duration) {
        let us = u64::try_from(d.as_micros()).unwrap_or(u64::MAX);
        self.decode_us_total.fetch_add(us, Ordering::Relaxed);
    }

    pub(crate) fn counters(&self) -> VideoCounters {
        let g = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let decoded = g(&self.decoded);
        VideoCounters {
            decoded,
            delivered: g(&self.delivered),
            dropped: g(&self.dropped),
            dropped_packets: g(&self.dropped_packets),
            decode_errors: g(&self.decode_errors),
            keyframe_requests: g(&self.keyframe_requests),
            width: self.last_w.load(Ordering::Relaxed),
            height: self.last_h.load(Ordering::Relaxed),
            avg_decode_us: g(&self.decode_us_total) / decoded.max(1),
            error: lock(&self.error).clone(),
        }
    }
}

/// Size of a `w` x `h` picture after fitting it into `max_w` x `max_h` (aspect ratio kept,
/// never enlarged, at least 1 x 1).
pub(crate) fn fit(w: u32, h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    let max_w = max_w.clamp(1, MAX_WIDTH);
    let max_h = max_h.clamp(1, MAX_HEIGHT);
    if w <= max_w && h <= max_h {
        return (w.max(1), h.max(1));
    }
    let scale = (f64::from(max_w) / f64::from(w)).min(f64::from(max_h) / f64::from(h));
    // Truncation is intended: the result is at most max_w / max_h, which are u32.
    let dw = ((f64::from(w) * scale) as u32).clamp(1, max_w);
    let dh = ((f64::from(h) * scale) as u32).clamp(1, max_h);
    (dw, dh)
}

/// RGB (3 bytes per pixel) to RGBA (alpha 255), scaled to `dw` x `dh` with nearest-neighbour
/// sampling. UNCERTAIN: nearest-neighbour is fast but aliases fine text when the picture is
/// shrunk by a large factor; a box filter would look better and cost more.
pub(crate) fn rgb_to_rgba_scaled(rgb: &[u8], w: u32, h: u32, dw: u32, dh: u32) -> Vec<u8> {
    let (w, h, dw, dh) = (w as usize, h as usize, dw as usize, dh as usize);
    let mut out = vec![255u8; dw * dh * 4];
    if rgb.len() < w * h * 3 || w == 0 || h == 0 {
        return out;
    }
    if (w, h) == (dw, dh) {
        for (src, dst) in rgb.chunks_exact(3).zip(out.chunks_exact_mut(4)) {
            dst[..3].copy_from_slice(src);
        }
        return out;
    }
    let xmap: Vec<usize> = (0..dw).map(|x| x * w / dw * 3).collect();
    for (y, row) in out.chunks_exact_mut(dw * 4).enumerate() {
        let src_row = &rgb[(y * h / dh) * w * 3..][..w * 3];
        for (dst, &sx) in row.chunks_exact_mut(4).zip(&xmap) {
            dst[..3].copy_from_slice(&src_row[sx..sx + 3]);
        }
    }
    out
}

/// Start the decoder thread. The returned sender goes into `SessionConfig::video_tap`.
/// `notify` is called (from the decoder thread) when a picture is ready and none is
/// outstanding. Returns `None` if the thread cannot be started.
#[cfg(feature = "h264")]
pub(crate) fn spawn_decoder(
    generation: u64,
    notify: impl Fn() + Send + 'static,
) -> Option<cc_transport::VideoTap> {
    // Bounded tap: when the decoder is too slow, packets are dropped and counted in the tap
    // (the decoder then waits for the next IDR frame). UNCERTAIN: capacity is a placeholder.
    let (tx, rx) = cc_transport::VideoTap::channel(4096);
    std::thread::Builder::new()
        .name("cc-video-decode".into())
        .spawn(move || run_decoder(rx, generation, notify))
        .ok()?;
    Some(tx)
}

/// Without the `h264` feature there is no decoder: the tap is not installed.
#[cfg(not(feature = "h264"))]
pub(crate) fn spawn_decoder(
    _generation: u64,
    _notify: impl Fn() + Send + 'static,
) -> Option<cc_transport::VideoTap> {
    VIDEO.record_error("this build has no H.264 decoder (feature `h264` is off)".into());
    None
}

#[cfg(feature = "h264")]
fn run_decoder(
    mut rx: tokio::sync::mpsc::Receiver<cc_transport::VideoPacket>,
    generation: u64,
    notify: impl Fn(),
) {
    use cc_media::rtp::H264Assembler;
    use cc_media::{H264Backend, H264Decoder, annexb_nal_types};

    let mut decoder = match H264Decoder::new(&H264Backend::from_env()) {
        Ok(d) => d,
        Err(e) => {
            VIDEO.record_error(format!("cannot start the H.264 decoder: {e}"));
            return;
        }
    };
    let mut assembler = H264Assembler::new();
    // The decoder can only start at an IDR frame (which carries SPS and PPS here).
    let mut wait_idr = true;
    let mut seen_dropped_units = 0;
    // `blocking_recv` returns `None` when the sender side is gone (the session was closed).
    while let Some(p) = rx.blocking_recv() {
        if !VIDEO.is_current(generation) {
            break;
        }
        if !VIDEO.is_h264() {
            continue;
        }
        let backlog = rx.len();
        if backlog > MAX_BACKLOG_PACKETS {
            let mut n = 1u64;
            while rx.try_recv().is_ok() {
                n += 1;
            }
            VIDEO.dropped_packets.fetch_add(n, Ordering::Relaxed);
            assembler = H264Assembler::new();
            seen_dropped_units = 0;
            wait_idr = true;
            VIDEO.request_keyframe();
            continue;
        }
        let unit = assembler.push(&p.payload, p.marker, p.timestamp, p.sequence);
        if assembler.dropped_units != seen_dropped_units {
            // A packet was lost: pictures after this one would be wrong until the next IDR.
            seen_dropped_units = assembler.dropped_units;
            wait_idr = true;
            VIDEO.request_keyframe();
        }
        let Some(unit) = unit else { continue };
        if wait_idr {
            if !annexb_nal_types(&unit.data).contains(&5) {
                VIDEO.request_keyframe();
                continue;
            }
            wait_idr = false;
        }
        let started = Instant::now();
        match decoder.decode(&unit.data) {
            Ok(Some(pic)) => {
                let (max_w, max_h) = VIDEO.max_size();
                let (dw, dh) = fit(pic.width, pic.height, max_w, max_h);
                let rgba = rgb_to_rgba_scaled(&pic.rgb, pic.width, pic.height, dw, dh);
                VIDEO.add_decode_time(started.elapsed());
                if VIDEO.publish(dw, dh, rgba) {
                    notify();
                }
            }
            Ok(None) => {}
            Err(e) => {
                VIDEO.decode_errors.fetch_add(1, Ordering::Relaxed);
                VIDEO.record_error(format!(
                    "H.264 decode error: {e} (NAL types {:?}, {} bytes)",
                    annexb_nal_types(&unit.data),
                    unit.data.len()
                ));
                wait_idr = true;
                VIDEO.request_keyframe();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_keeps_aspect_and_never_enlarges() {
        assert_eq!(fit(640, 480, 1920, 1080), (640, 480));
        assert_eq!(fit(1920, 1080, 960, 540), (960, 540));
        assert_eq!(fit(1920, 1080, 800, 800), (800, 450));
        assert_eq!(fit(3840, 2160, 5000, 5000), (1920, 1080));
        assert_eq!(fit(100, 100, 0, 0), (1, 1));
    }

    #[test]
    fn rgb_to_rgba_same_size_and_scaled() {
        let rgb = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120];
        // 2 x 2, same size.
        let same = rgb_to_rgba_scaled(&rgb, 2, 2, 2, 2);
        assert_eq!(&same[..8], &[10, 20, 30, 255, 40, 50, 60, 255]);
        assert_eq!(&same[8..], &[70, 80, 90, 255, 100, 110, 120, 255]);
        // 2 x 2 down to 1 x 1: the top-left pixel.
        assert_eq!(rgb_to_rgba_scaled(&rgb, 2, 2, 1, 1), [10, 20, 30, 255]);
    }

    #[test]
    fn mailbox_keeps_the_newest_frame_and_counts_drops() {
        let v = Video::new();
        // First picture: an event must be sent. Second and third: one is outstanding.
        assert!(v.publish(1, 1, vec![1, 1, 1, 255]));
        assert!(!v.publish(1, 1, vec![2, 2, 2, 255]));
        assert!(!v.publish(1, 1, vec![3, 3, 3, 255]));
        let f = v.take().expect("a frame");
        assert_eq!(f.rgba, [3, 3, 3, 255]);
        assert_eq!(f.seq, 3);
        assert!(v.take().is_none());
        let c = v.counters();
        assert_eq!((c.decoded, c.delivered, c.dropped), (3, 1, 2));
        // After take the next picture raises an event again.
        assert!(v.publish(1, 1, vec![4, 4, 4, 255]));
    }

    #[test]
    fn keyframe_requests_are_rate_limited() {
        let v = Video::new();
        v.request_keyframe();
        assert!(v.take_keyframe_request());
        v.request_keyframe();
        assert!(!v.take_keyframe_request(), "second request within a second");
        assert_eq!(v.counters().keyframe_requests, 1);
    }

    #[test]
    fn reset_invalidates_the_old_decoder_generation() {
        let v = Video::new();
        let g = v.reset();
        assert!(v.is_current(g));
        v.stop();
        assert!(!v.is_current(g));
    }

    #[cfg(feature = "h264")]
    #[test]
    fn decoder_thread_ends_when_the_tap_is_dropped() {
        // Own generation number: do not disturb the global used by other tests.
        let tx = spawn_decoder(VIDEO.generation.load(Ordering::SeqCst), || {}).expect("thread");
        drop(tx);
        // Nothing to assert beyond "does not hang the test process": the thread ends at once.
    }
}
