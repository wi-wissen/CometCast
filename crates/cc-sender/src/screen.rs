// SPDX-License-Identifier: GPL-3.0-or-later
//! The screen pipeline of the sender: capture -> change detection -> H.264 encoder, on one
//! dedicated thread (a thread, not an async task, because capture and encoding block).
//!
//! Rules (docs/measurement-points.md, A10 and A11; the ideas are described in docs/capture.md):
//!
//! * Unchanged pictures are not encoded.
//! * After the picture has been still for a short time it is encoded again a few times
//!   ([`cc_media::refresh::StaticRefresh`]) and then nothing more is sent.
//! * At most one frame is in flight: the thread only captures after the async side reported
//!   that the previous frame was handed to the transport ("token").
//! * A key frame request (new peer, picture loss indication) re-encodes the last picture as an
//!   IDR frame, even when the screen is static.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cc_capture::{Capture, RawFrame, ScreenSource};
use cc_media::refresh::{RefreshConfig, StaticRefresh};
use cc_media::{BgraFrame, H264Config, H264Encoder};

/// Settings for the screen mode of the sender.
#[derive(Debug, Clone)]
pub struct ScreenOptions {
    /// X11 display such as `:99`. `None` uses `$DISPLAY`.
    pub display: Option<String>,
    /// Id of the capture source (from `ScreenSource::list_sources`). `None` = whole desktop.
    pub source_id: Option<String>,
    /// How often the screen is looked at.
    pub fps: u32,
    /// Encoder target bit rate in bits per second.
    pub bitrate_bps: u32,
    /// Static refresh timing.
    pub refresh: RefreshConfig,
}

impl Default for ScreenOptions {
    /// UNCERTAIN: 15 fps and 2 Mbit/s are starting values, not tuned.
    fn default() -> Self {
        Self {
            display: None,
            source_id: None,
            fps: 15,
            bitrate_bps: 2_000_000,
            refresh: RefreshConfig::default(),
        }
    }
}

/// Counters of the screen pipeline, readable while it runs.
#[derive(Debug, Default)]
pub(crate) struct Counters {
    pub(crate) polls: AtomicU64,
    pub(crate) changed: AtomicU64,
    pub(crate) unchanged: AtomicU64,
    pub(crate) encoded: AtomicU64,
    pub(crate) refreshes: AtomicU64,
    pub(crate) keyframes: AtomicU64,
    pub(crate) encoded_bytes: AtomicU64,
    pub(crate) keyframe_requests: AtomicU64,
}

/// A copy of the pipeline counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScreenStats {
    /// Times the capture source was asked for a picture.
    pub polls: u64,
    /// Pictures that differed from the previous one.
    pub changed: u64,
    /// Polls that found the screen unchanged.
    pub unchanged: u64,
    /// Frames produced by the encoder (new pictures, refreshes and key frame requests).
    pub encoded: u64,
    /// Of those, frames made by static refresh.
    pub refreshes: u64,
    /// Of those, key frames.
    pub keyframes: u64,
    /// Total size of the encoded frames in bytes.
    pub encoded_bytes: u64,
    /// Key frame requests (PLI/FIR) received from the receiver.
    pub keyframe_requests: u64,
}

impl Counters {
    pub(crate) fn snapshot(&self) -> ScreenStats {
        let g = |a: &AtomicU64| a.load(Ordering::Relaxed);
        ScreenStats {
            polls: g(&self.polls),
            changed: g(&self.changed),
            unchanged: g(&self.unchanged),
            encoded: g(&self.encoded),
            refreshes: g(&self.refreshes),
            keyframes: g(&self.keyframes),
            encoded_bytes: g(&self.encoded_bytes),
            keyframe_requests: g(&self.keyframe_requests),
        }
    }
}

/// An encoded frame on its way to the transport.
#[derive(Debug)]
pub(crate) struct Unit {
    pub(crate) data: Vec<u8>,
}

/// Why the pipeline thread ended.
#[derive(Debug)]
pub(crate) enum Ended {
    /// The capture source says it is over.
    SourceGone,
    /// Capture or encoding failed.
    Failed(String),
    /// Asked to stop.
    Stopped,
}

/// Handle to the running pipeline thread.
pub(crate) struct Pipeline {
    stop: Arc<AtomicBool>,
    keyframe: Arc<AtomicBool>,
    token_tx: mpsc::Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl Pipeline {
    /// The flag that asks the thread for a key frame with its next frame.
    pub(crate) fn keyframe_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.keyframe)
    }

    /// Tell the thread that the previous frame was handed to the transport.
    pub(crate) fn frame_taken(&self) {
        let _ = self.token_tx.send(());
    }

    pub(crate) fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Everything the thread needs, built in `SenderCore::start` so errors show up early.
pub(crate) struct Prepared {
    pub(crate) source: Box<dyn ScreenSource + Send>,
    pub(crate) encoder: H264Encoder,
    pub(crate) options: ScreenOptions,
    pub(crate) counters: Arc<Counters>,
}

/// Open the capture source and the encoder.
pub(crate) fn prepare(options: ScreenOptions, counters: Arc<Counters>) -> Result<Prepared, String> {
    let mut source = cc_capture::open_default(options.display.as_deref(), options.fps)
        .map_err(|e| e.to_string())?;
    let list = source.list_sources().map_err(|e| e.to_string())?;
    let chosen = match &options.source_id {
        Some(id) => list
            .iter()
            .find(|s| &s.id == id)
            .ok_or_else(|| format!("no capture source {id:?}"))?,
        None => list.first().ok_or("no capture source available")?,
    };
    source.start(chosen).map_err(|e| e.to_string())?;
    let encoder = H264Encoder::new(&H264Config {
        bitrate_bps: options.bitrate_bps,
        fps: options.fps as f32,
        ..H264Config::default()
    })
    .map_err(|e| e.to_string())?;
    Ok(Prepared {
        source,
        encoder,
        options,
        counters,
    })
}

/// Start the thread. Frames arrive on `out`; the end of the thread is reported on `ended`.
pub(crate) fn spawn(
    prepared: Prepared,
    out: tokio::sync::mpsc::UnboundedSender<Unit>,
    ended: tokio::sync::mpsc::UnboundedSender<Ended>,
) -> Pipeline {
    let stop = Arc::new(AtomicBool::new(false));
    let keyframe = Arc::new(AtomicBool::new(false));
    let (token_tx, token_rx) = mpsc::channel();
    // The first frame may go out at once.
    let _ = token_tx.send(());
    let thread = {
        let stop = Arc::clone(&stop);
        let keyframe = Arc::clone(&keyframe);
        std::thread::Builder::new()
            .name("cometcast-screen".into())
            .spawn(move || {
                let result = run(prepared, &out, &token_rx, &stop, &keyframe);
                let _ = ended.send(result);
            })
            .ok()
    };
    Pipeline {
        stop,
        keyframe,
        token_tx,
        thread,
    }
}

fn run(
    mut p: Prepared,
    out: &tokio::sync::mpsc::UnboundedSender<Unit>,
    tokens: &mpsc::Receiver<()>,
    stop: &AtomicBool,
    keyframe: &AtomicBool,
) -> Ended {
    let c = &p.counters;
    let started = Instant::now();
    let mut refresh = StaticRefresh::new(p.options.refresh);
    let mut last: Option<RawFrame> = None;
    let mut have_token = false;
    let tick = Duration::from_secs(1) / p.options.fps.max(1);

    // Encode `frame`; send it if the encoder produced something. Returns whether a frame
    // was sent, or an error text.
    let encode_and_send =
        |enc: &mut H264Encoder, frame: &RawFrame, force_key: bool, is_refresh: bool| {
            if force_key {
                enc.force_keyframe();
            }
            let ts = started.elapsed().as_micros() as u64;
            let encoded = enc
                .encode_bgra(
                    BgraFrame {
                        width: frame.width,
                        height: frame.height,
                        bgra: &frame.bgra,
                    },
                    ts,
                )
                .map_err(|e| e.to_string())?;
            let Some(encoded) = encoded else {
                return Ok(false);
            };
            c.encoded.fetch_add(1, Ordering::Relaxed);
            c.encoded_bytes
                .fetch_add(encoded.data.len() as u64, Ordering::Relaxed);
            if is_refresh {
                c.refreshes.fetch_add(1, Ordering::Relaxed);
            }
            if encoded.is_keyframe {
                c.keyframes.fetch_add(1, Ordering::Relaxed);
            }
            if out.send(Unit { data: encoded.data }).is_err() {
                return Err("sender is gone".to_string());
            }
            Ok(true)
        };

    loop {
        if stop.load(Ordering::Relaxed) {
            return Ended::Stopped;
        }
        // One frame in flight: wait for the transport to take the previous one before looking
        // at the screen again.
        if !have_token {
            match tokens.recv_timeout(tick) {
                Ok(()) => have_token = true,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ended::Stopped,
            }
        }
        c.polls.fetch_add(1, Ordering::Relaxed);
        let capture = match p.source.next_frame() {
            Ok(v) => v,
            Err(e) => return Ended::Failed(e.to_string()),
        };
        let want_key = keyframe.load(Ordering::Relaxed);
        match capture {
            Capture::Ended => return Ended::SourceGone,
            Capture::Frame(frame) => {
                c.changed.fetch_add(1, Ordering::Relaxed);
                let key = keyframe.swap(false, Ordering::Relaxed);
                match encode_and_send(&mut p.encoder, &frame, key, false) {
                    Ok(true) => have_token = false,
                    Ok(false) => {}
                    Err(e) => return Ended::Failed(e),
                }
                refresh.picture_changed(Instant::now());
                last = Some(frame);
            }
            Capture::Unchanged => {
                c.unchanged.fetch_add(1, Ordering::Relaxed);
                let Some(frame) = last.as_ref() else {
                    continue;
                };
                let due_refresh = !want_key && refresh.poll(Instant::now());
                if want_key || due_refresh {
                    let key = keyframe.swap(false, Ordering::Relaxed);
                    match encode_and_send(&mut p.encoder, frame, key, due_refresh) {
                        Ok(true) => have_token = false,
                        Ok(false) => {}
                        Err(e) => return Ended::Failed(e),
                    }
                }
            }
        }
    }
}
