// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests of the screen pipeline with a scripted capture source (no X server needed).

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cc_capture::{Capture, CaptureError, CaptureSource, RawFrame, ScreenSource};
use cc_media::refresh::RefreshConfig;
use cc_media::{H264Backend, H264Config, H264Encoder, annexb_nal_types};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::screen::{Counters, Pipeline, Prepared, ScreenOptions, Unit, spawn};

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
    let change = Arc::new(Mutex::new(None));
    let counters = Arc::new(Counters::default());
    let options = ScreenOptions {
        fps: 50,
        ..ScreenOptions::default()
    };
    let encoder = H264Encoder::new(&H264Config {
        backend: H264Backend::Source,
        ..H264Config::default()
    })
    .unwrap();
    let prepared = Prepared {
        source: Box::new(Scripted {
            next: Arc::clone(&change),
            first: true,
        }),
        encoder,
        options,
        counters: Arc::clone(&counters),
    };
    let (tx, frames) = tokio::sync::mpsc::unbounded_channel();
    let (ended_tx, ended_rx) = tokio::sync::mpsc::unbounded_channel();
    // Keep the receiver of the "ended" messages alive for the whole test process.
    std::mem::forget(ended_rx);
    Rig {
        pipeline: spawn(prepared, tx, ended_tx),
        frames,
        counters,
        change,
    }
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
