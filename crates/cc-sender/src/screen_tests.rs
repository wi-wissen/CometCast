// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests of the screen pipeline with a scripted capture source (no X server needed).

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cc_capture::{Capture, CaptureError, CaptureSource, RawFrame, ScreenSource};
use cc_media::refresh::RefreshConfig;
use cc_media::{EncodedFrame, H264Backend, H264Config, H264Encoder, annexb_nal_types};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::screen::{
    Counters, Ended, FrameEncoder, KEYFRAME_MIN_INTERVAL, Pipeline, Prepared, ScreenOptions, Unit,
    spawn,
};

/// A capture source that shows a solid picture and changes it only when told to.
struct Scripted {
    next: Arc<Mutex<Option<RawFrame>>>,
    first: bool,
}

fn solid(r: u8, g: u8, b: u8) -> RawFrame {
    let (w, h) = (160u32, 120u32);
    let mut bgra = Vec::new();
    for _ in 0..w * h {
        bgra.extend_from_slice(&[b, g, r, 255]);
    }
    RawFrame {
        width: w,
        height: h,
        bgra,
    }
}

impl ScreenSource for Scripted {
    fn list_sources(&self) -> Result<Vec<CaptureSource>, CaptureError> {
        Ok(Vec::new())
    }
    fn start(&mut self, _s: &CaptureSource) -> Result<(), CaptureError> {
        Ok(())
    }
    fn next_frame(&mut self) -> Result<Capture, CaptureError> {
        std::thread::sleep(Duration::from_millis(20));
        if self.first {
            self.first = false;
            return Ok(Capture::Frame(solid(200, 30, 40)));
        }
        Ok(match self.next.lock().unwrap().take() {
            Some(f) => Capture::Frame(f),
            None => Capture::Unchanged,
        })
    }
    fn stop(&mut self) {}
}

struct Rig {
    pipeline: Pipeline,
    frames: UnboundedReceiver<Unit>,
    counters: Arc<Counters>,
    change: Arc<Mutex<Option<RawFrame>>>,
}

fn rig() -> Rig {
    let encoder = H264Encoder::new(&H264Config {
        backend: H264Backend::Source,
        ..H264Config::default()
    })
    .unwrap();
    rig_with(Box::new(encoder), None).0
}

/// A rig with the given encoder (and source, default: the scripted solid picture). The second
/// value is the receiver of the "thread ended" messages.
fn rig_with(
    encoder: Box<dyn FrameEncoder>,
    source: Option<Box<dyn ScreenSource + Send>>,
) -> (Rig, UnboundedReceiver<Ended>) {
    let change = Arc::new(Mutex::new(None));
    let counters = Arc::new(Counters::default());
    let options = ScreenOptions {
        fps: 50,
        ..ScreenOptions::default()
    };
    let prepared = Prepared {
        source: source.unwrap_or_else(|| {
            Box::new(Scripted {
                next: Arc::clone(&change),
                first: true,
            })
        }),
        encoder,
        options,
        counters: Arc::clone(&counters),
    };
    let (tx, frames) = tokio::sync::mpsc::unbounded_channel();
    let (ended_tx, ended_rx) = tokio::sync::mpsc::unbounded_channel();
    (
        Rig {
            pipeline: spawn(prepared, tx, ended_tx),
            frames,
            counters,
            change,
        },
        ended_rx,
    )
}

/// An encoder that returns `None` for the next `skip` calls (like OpenH264's rate control after
/// a large first frame) and then dummy frames. Records what it was asked to do.
#[derive(Clone, Default)]
struct SkippingEncoder {
    state: Arc<Mutex<SkipState>>,
}

#[derive(Default)]
struct SkipState {
    /// Calls that return `None` before the next frame comes out.
    skip: usize,
    /// Skips are only used up by calls that ask for a key frame (instead of every call).
    skip_only_keyframes: bool,
    key_calls: usize,
}

impl FrameEncoder for SkippingEncoder {
    fn encode(
        &mut self,
        _frame: &RawFrame,
        force_key: bool,
        timestamp_us: u64,
    ) -> Result<Option<EncodedFrame>, String> {
        let mut st = self.state.lock().unwrap();
        st.key_calls += usize::from(force_key);
        if st.skip > 0 && (force_key || !st.skip_only_keyframes) {
            st.skip -= 1;
            return Ok(None);
        }
        Ok(Some(EncodedFrame {
            data: vec![0, 0, 0, 1, if force_key { 0x65 } else { 0x41 }],
            is_keyframe: force_key,
            timestamp_us,
        }))
    }
}

/// Hand every frame back at once for `ms` milliseconds; returns the frames received.
fn drain_for(r: &mut Rig, ms: u64) -> Vec<Unit> {
    let mut got = Vec::new();
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        if let Some(u) = recv_within(&mut r.frames, 20) {
            got.push(u);
            r.pipeline.frame_taken();
        }
    }
    got
}

fn recv_within(rx: &mut UnboundedReceiver<Unit>, ms: u64) -> Option<Unit> {
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        if let Ok(u) = rx.try_recv() {
            return Some(u);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

#[test]
fn first_frame_is_a_key_frame_and_only_one_frame_is_in_flight() {
    let mut r = rig();
    let first = recv_within(&mut r.frames, 3000).expect("first frame");
    let types = annexb_nal_types(&first.data);
    assert!(
        types.contains(&7) && types.contains(&8) && types.contains(&5),
        "{types:?}"
    );

    // The screen changes, but the transport has not taken the first frame yet:
    // nothing more may be captured or encoded.
    let polls_before = r.counters.snapshot().polls;
    *r.change.lock().unwrap() = Some(solid(20, 160, 220));
    assert!(
        recv_within(&mut r.frames, 500).is_none(),
        "second frame without token"
    );
    assert_eq!(
        r.counters.snapshot().polls,
        polls_before,
        "no capture while a frame is in flight"
    );

    r.pipeline.frame_taken();
    // (OpenH264 may make this one an IDR frame by itself: a full-screen colour change counts
    // as a scene change for it.)
    recv_within(&mut r.frames, 2000).expect("frame after token");
    r.pipeline.stop();
}

#[test]
fn static_screen_gets_a_few_refreshes_and_then_silence() {
    let mut r = rig();
    let mut frames = 0;
    // Hand every frame back at once, like a fast transport, and count what comes.
    let end = Instant::now() + Duration::from_millis(2000);
    while Instant::now() < end {
        if recv_within(&mut r.frames, 50).is_some() {
            frames += 1;
            r.pipeline.frame_taken();
        }
    }
    let s = r.counters.snapshot();
    let max = 1 + u64::from(RefreshConfig::default().repeats);
    assert!(
        frames <= max,
        "{frames} frames for a static screen, wanted at most {max}"
    );
    assert_eq!(frames, 4, "one picture plus three refreshes: {s:?}");
    assert_eq!(s.refreshes, 3, "{s:?}");
    assert!(
        s.unchanged > 20,
        "the screen was looked at while quiet: {s:?}"
    );
    r.pipeline.stop();
}

#[test]
fn a_key_frame_request_is_answered_even_on_a_static_screen() {
    let mut r = rig();
    // Let the first frame and the refreshes pass.
    let end = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < end {
        if recv_within(&mut r.frames, 50).is_some() {
            r.pipeline.frame_taken();
        }
    }
    let before = r.counters.snapshot();
    r.pipeline.keyframe_flag().store(true, Ordering::Relaxed);
    let unit = recv_within(&mut r.frames, 2000).expect("key frame after request");
    assert!(annexb_nal_types(&unit.data).contains(&5));
    assert_eq!(r.counters.snapshot().keyframes, before.keyframes + 1);
    r.pipeline.stop();
}

#[test]
fn a_long_run_of_skipped_refreshes_still_ends_with_a_sent_picture() {
    // The first picture goes out; then the rate control skips the next 40 encodes (more than
    // 0.8 s at 50 fps, far more than the 3 refresh repeats). The picture must still be sent.
    let enc = SkippingEncoder::default();
    let state = Arc::clone(&enc.state);
    let (mut r, _ended) = rig_with(Box::new(enc), None);
    let first = recv_within(&mut r.frames, 3000).expect("first frame");
    assert_eq!(first.data.len(), 5);
    state.lock().unwrap().skip = 40;
    r.pipeline.frame_taken();
    let frames = drain_for(&mut r, 3000);
    let s = r.counters.snapshot();
    assert_eq!(s.skipped, 40, "{s:?}");
    // The 3 refreshes were not used up by the skips: all three come out after them.
    assert_eq!(frames.len(), 3, "{s:?}");
    assert_eq!(s.refreshes, 3, "{s:?}");
    r.pipeline.stop();
}

#[test]
fn a_skipped_new_picture_is_retried_until_it_is_sent() {
    let enc = SkippingEncoder::default();
    let state = Arc::clone(&enc.state);
    let (mut r, _ended) = rig_with(Box::new(enc), None);
    recv_within(&mut r.frames, 3000).expect("first frame");
    // Let the refreshes pass, then change the picture while the encoder skips 30 times.
    r.pipeline.frame_taken();
    drain_for(&mut r, 800);
    state.lock().unwrap().skip = 30;
    let before = r.counters.snapshot().encoded;
    *r.change.lock().unwrap() = Some(solid(20, 160, 220));
    let frames = drain_for(&mut r, 2500);
    assert!(!frames.is_empty(), "the new picture was never sent");
    assert!(r.counters.snapshot().encoded > before);
    assert_eq!(r.counters.snapshot().skipped, 30);
    r.pipeline.stop();
}

#[test]
fn the_key_frame_request_survives_skipped_pictures() {
    let enc = SkippingEncoder::default();
    let state = Arc::clone(&enc.state);
    state.lock().unwrap().skip_only_keyframes = true;
    let (mut r, _ended) = rig_with(Box::new(enc), None);
    drain_for(&mut r, 1000);
    // The next 5 attempts to make a key frame are skipped by the encoder.
    state.lock().unwrap().skip = 5;
    let before = r.counters.snapshot();
    r.pipeline.keyframe_flag().store(true, Ordering::Relaxed);
    let frames = drain_for(&mut r, 1500);
    assert_eq!(frames.len(), 1, "exactly one key frame");
    assert_eq!(frames[0].data[4], 0x65, "it is the key frame");
    let s = r.counters.snapshot();
    assert_eq!(s.skipped - before.skipped, 5, "{s:?}");
    assert_eq!(s.keyframes - before.keyframes, 1, "{s:?}");
    assert_eq!(state.lock().unwrap().key_calls, 6, "5 skipped asks + 1");
    assert!(
        !r.pipeline.keyframe_flag().load(Ordering::Relaxed),
        "flag cleared once the key frame was sent"
    );
    r.pipeline.stop();
}

#[test]
fn a_flood_of_key_frame_requests_is_coalesced_and_counted() {
    let (mut r, _ended) = rig_with(Box::new(SkippingEncoder::default()), None);
    drain_for(&mut r, 600);
    let before = r.counters.snapshot();
    let requester = r.pipeline.keyframe_requester();
    let t0 = Instant::now();
    // 1000 requests within half a second.
    let accepted = (0..1000u64)
        .filter(|i| requester.request_at(t0 + Duration::from_micros(i * 500)))
        .count();
    assert_eq!(accepted, 1);
    let s = r.counters.snapshot();
    assert_eq!(s.keyframe_requests - before.keyframe_requests, 1000);
    assert_eq!(
        s.keyframe_requests_ignored - before.keyframe_requests_ignored,
        999
    );
    // One key frame comes out for all of them.
    let frames = drain_for(&mut r, 800);
    assert_eq!(frames.len(), 1);
    // The flag is clear again, but the one-second window still blocks a new request ...
    assert!(!requester.request_at(t0 + Duration::from_millis(900)));
    // ... and after it a new request is accepted again.
    assert!(requester.request_at(t0 + KEYFRAME_MIN_INTERVAL + Duration::from_millis(1)));
    r.pipeline.stop();
}

/// A source that panics on its second call.
struct Panicking(u32);

impl ScreenSource for Panicking {
    fn list_sources(&self) -> Result<Vec<CaptureSource>, CaptureError> {
        Ok(Vec::new())
    }
    fn start(&mut self, _s: &CaptureSource) -> Result<(), CaptureError> {
        Ok(())
    }
    fn next_frame(&mut self) -> Result<Capture, CaptureError> {
        self.0 += 1;
        if self.0 > 1 {
            panic!("boom in the capture code");
        }
        Ok(Capture::Frame(solid(1, 2, 3)))
    }
    fn stop(&mut self) {}
}

#[test]
fn a_panic_in_the_capture_thread_is_reported() {
    let (mut r, mut ended) = rig_with(
        Box::new(SkippingEncoder::default()),
        Some(Box::new(Panicking(0))),
    );
    recv_within(&mut r.frames, 3000).expect("first frame");
    r.pipeline.frame_taken();
    let end = Instant::now() + Duration::from_secs(3);
    loop {
        match ended.try_recv() {
            Ok(Ended::Failed(msg)) => {
                assert!(msg.contains("panicked") && msg.contains("boom"), "{msg}");
                break;
            }
            Ok(other) => panic!("wrong end {other:?}"),
            Err(_) if Instant::now() < end => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => panic!("no end message"),
        }
    }
    assert!(r.pipeline.stop(), "a finished thread is joined");
}

/// A source stuck in a call, like a dead X server.
struct Stuck;

impl ScreenSource for Stuck {
    fn list_sources(&self) -> Result<Vec<CaptureSource>, CaptureError> {
        Ok(Vec::new())
    }
    fn start(&mut self, _s: &CaptureSource) -> Result<(), CaptureError> {
        Ok(())
    }
    fn next_frame(&mut self) -> Result<Capture, CaptureError> {
        std::thread::sleep(Duration::from_secs(4));
        Ok(Capture::Unchanged)
    }
    fn stop(&mut self) {}
}

#[test]
fn stop_does_not_wait_forever_for_a_stuck_capture_call() {
    let (mut r, _ended) = rig_with(Box::new(SkippingEncoder::default()), Some(Box::new(Stuck)));
    std::thread::sleep(Duration::from_millis(100));
    r.pipeline.stop_timeout = Duration::from_millis(300);
    let t = Instant::now();
    let joined = r.pipeline.stop();
    let took = t.elapsed();
    assert!(!joined, "the thread is still stuck");
    assert!(took < Duration::from_millis(1500), "stop took {took:?}");
}

#[test]
fn a_cancelled_picker_is_reported_as_denied_and_other_errors_as_failed() {
    use crate::screen::{PrepareError, capture_err};
    use cc_capture::CaptureError;
    assert!(matches!(
        capture_err(CaptureError::Denied),
        PrepareError::Denied(_)
    ));
    assert!(matches!(
        capture_err(CaptureError::Unsupported("x")),
        PrepareError::Failed(_)
    ));
    assert!(matches!(
        capture_err(CaptureError::Display("x".into())),
        PrepareError::Failed(_)
    ));
}

#[test]
fn the_picker_option_goes_through_the_picker_backend_selection() {
    use crate::screen::{PrepareError, prepare};
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        // A real Wayland session with the `wayland` feature would open its picker here.
        eprintln!("skipped: WAYLAND_DISPLAY is set");
        return;
    }
    // No X server there: the picker selection falls to the X11 fallback (or finds no
    // portal) and fails as "unavailable", never as "denied".
    let options = ScreenOptions {
        picker: true,
        display: Some(":9999".into()),
        ..ScreenOptions::default()
    };
    match prepare(options, Arc::new(Counters::default())) {
        Err(PrepareError::Failed(_)) => {}
        Err(PrepareError::Denied(m)) => panic!("unexpected denial: {m}"),
        Ok(_) => panic!("no capture backend can work here"),
    }
}
