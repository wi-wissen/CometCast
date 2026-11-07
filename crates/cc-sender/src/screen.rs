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
//!   IDR frame, even when the screen is static. The request flag is cleared only when a frame
//!   was really sent (the rate control may skip the picture), and requests are rate limited
//!   ([`KEYFRAME_MIN_INTERVAL`]).
//! * When the encoder skips a picture (rate control, `encode` returns `None`) the picture is
//!   "owed": it is offered again on every tick until a frame comes out, without using up the
//!   static refresh repeats ([`OWED_TIMEOUT`]).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cc_capture::{Capture, RawFrame, ScreenSource};
use cc_media::refresh::{RefreshConfig, StaticRefresh};
use cc_media::{BgraFrame, EncodedFrame, H264Config, H264Encoder};

/// At most one key frame request is accepted per this time; the others are counted as ignored
/// (a flood of PLI/FIR must not make the sender produce a key frame per request).
/// UNCERTAIN: 1 s is a placeholder, not tuned.
pub(crate) const KEYFRAME_MIN_INTERVAL: Duration = Duration::from_secs(1);

/// How long a skipped picture is retried on every tick before the sender gives up on it.
/// UNCERTAIN: placeholder. A probe (1080p content, 2 Mbit/s, 15 fps) saw skips last over 1.6 s
/// after a 358 KB first frame.
pub(crate) const OWED_TIMEOUT: Duration = Duration::from_secs(30);

/// How long [`Pipeline::stop`] waits for the thread (it may be stuck in a call to a dead X
/// server). UNCERTAIN: placeholder.
const STOP_TIMEOUT: Duration = Duration::from_secs(3);

/// Settings for the screen mode of the sender.
#[derive(Debug, Clone)]
pub struct ScreenOptions {
    /// X11 display such as `:99`. `None` uses `$DISPLAY`.
    pub display: Option<String>,
    /// Id of the capture source (from `ScreenSource::list_sources`). `None` = whole desktop.
    pub source_id: Option<String>,
    /// Use the system's own screen picker (`cc_capture::open_picker`): the person chooses a
    /// screen or window in a dialog the system draws. `display` is then only the X11 fallback
    /// display, and `source_id` is ignored. A cancelled picker ends `prepare` with
    /// [`PrepareError::Denied`].
    pub picker: bool,
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
            picker: false,
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
    pub(crate) keyframe_requests_ignored: AtomicU64,
    pub(crate) skipped: AtomicU64,
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
    /// Of those, key frames (IDR frames only).
    pub keyframes: u64,
    /// Total size of the encoded frames in bytes.
    pub encoded_bytes: u64,
    /// Key frame requests (PLI/FIR) received from the receiver.
    pub keyframe_requests: u64,
    /// Of those, requests that were ignored: one was already pending, or one was accepted
    /// less than a second before (rate limit).
    pub keyframe_requests_ignored: u64,
    /// Encoder calls that produced no frame (the rate control skipped the picture).
    pub skipped: u64,
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
            keyframe_requests_ignored: g(&self.keyframe_requests_ignored),
            skipped: g(&self.skipped),
        }
    }
}

/// What the pipeline needs from an encoder (a trait so tests can script it).
pub(crate) trait FrameEncoder: Send {
    /// Encode one picture; `None` = the rate control skipped it.
    fn encode(
        &mut self,
        frame: &RawFrame,
        force_key: bool,
        timestamp_us: u64,
    ) -> Result<Option<EncodedFrame>, String>;
}

impl FrameEncoder for H264Encoder {
    fn encode(
        &mut self,
        frame: &RawFrame,
        force_key: bool,
        timestamp_us: u64,
    ) -> Result<Option<EncodedFrame>, String> {
        if force_key {
            self.force_keyframe();
        }
        self.encode_bgra(
            BgraFrame {
                width: frame.width,
                height: frame.height,
                bgra: &frame.bgra,
            },
            timestamp_us,
        )
        .map_err(|e| e.to_string())
    }
}

/// Turns key frame requests (PLI/FIR) into the flag the pipeline thread reads, with a rate
/// limit. Cheap to clone; all clones share the state.
#[derive(Clone)]
pub(crate) struct KeyframeRequester {
    flag: Arc<AtomicBool>,
    counters: Arc<Counters>,
    last_accepted: Arc<Mutex<Option<Instant>>>,
    min_interval: Duration,
}

impl KeyframeRequester {
    /// Register a request that arrived now. Returns whether it was accepted.
    pub(crate) fn request(&self) -> bool {
        self.request_at(Instant::now())
    }

    /// Like [`Self::request`] with the clock passed in (for tests).
    pub(crate) fn request_at(&self, now: Instant) -> bool {
        self.counters
            .keyframe_requests
            .fetch_add(1, Ordering::Relaxed);
        let mut last = self
            .last_accepted
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let too_soon = last.is_some_and(|t| now.saturating_duration_since(t) < self.min_interval);
        // A request while one is still pending is already covered by that one.
        let pending = self.flag.load(Ordering::Relaxed);
        if too_soon || pending {
            self.counters
                .keyframe_requests_ignored
                .fetch_add(1, Ordering::Relaxed);
            return false;
        }
        *last = Some(now);
        self.flag.store(true, Ordering::Relaxed);
        true
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
    /// Capture or encoding failed, or the thread panicked or could not be started.
    Failed(String),
    /// Asked to stop.
    Stopped,
}

/// Handle to the running pipeline thread.
pub(crate) struct Pipeline {
    stop: Arc<AtomicBool>,
    keyframe: Arc<AtomicBool>,
    counters: Arc<Counters>,
    token_tx: mpsc::Sender<()>,
    thread: Option<JoinHandle<()>>,
    /// Disconnects when the thread has finished (it holds the only sender).
    done_rx: mpsc::Receiver<()>,
    /// How long [`Pipeline::stop`] waits for the thread.
    pub(crate) stop_timeout: Duration,
}

impl Pipeline {
    /// The flag that asks the thread for a key frame with its next frame. Setting it
    /// directly bypasses the rate limit; use [`Self::keyframe_requester`] for PLI/FIR.
    #[cfg(test)]
    pub(crate) fn keyframe_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.keyframe)
    }

    /// The rate limited way to ask for a key frame.
    pub(crate) fn keyframe_requester(&self) -> KeyframeRequester {
        KeyframeRequester {
            flag: Arc::clone(&self.keyframe),
            counters: Arc::clone(&self.counters),
            last_accepted: Arc::new(Mutex::new(None)),
            min_interval: KEYFRAME_MIN_INTERVAL,
        }
    }

    /// Tell the thread that the previous frame was handed to the transport.
    pub(crate) fn frame_taken(&self) {
        let _ = self.token_tx.send(());
    }

    /// Ask the thread to stop and wait for it, but not longer than `stop_timeout`: the thread
    /// may be stuck in a call to an X server that died. A thread that does not finish in time
    /// is left running on its own (it ends as soon as the call returns). Returns whether the
    /// thread was joined.
    pub(crate) fn stop(&mut self) -> bool {
        self.stop.store(true, Ordering::Relaxed);
        let Some(t) = self.thread.take() else {
            return true;
        };
        match self.done_rx.recv_timeout(self.stop_timeout) {
            Err(mpsc::RecvTimeoutError::Timeout) => false,
            // `Ok` never happens (nothing is sent); `Disconnected` = the thread is done.
            _ => {
                let _ = t.join();
                true
            }
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
    pub(crate) encoder: Box<dyn FrameEncoder>,
    pub(crate) options: ScreenOptions,
    pub(crate) counters: Arc<Counters>,
}

/// Why the screen could not be prepared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PrepareError {
    /// The person cancelled the system picker or the system refused (`capture_denied`).
    Denied(String),
    /// macOS before 14 (`capture_os_old`).
    OsOld(String),
    /// Wayland without the ScreenCast portal or libpipewire (`capture_no_portal`).
    NoPortal(String),
    /// Anything else (`capture_unavailable`).
    Failed(String),
}

impl From<String> for PrepareError {
    fn from(s: String) -> Self {
        Self::Failed(s)
    }
}

pub(crate) fn capture_err(e: cc_capture::CaptureError) -> PrepareError {
    match e {
        cc_capture::CaptureError::Denied => PrepareError::Denied(e.to_string()),
        cc_capture::CaptureError::OsTooOld => PrepareError::OsOld(e.to_string()),
        cc_capture::CaptureError::NoPortal(_) => PrepareError::NoPortal(e.to_string()),
        other => PrepareError::Failed(other.to_string()),
    }
}

/// Open the capture source and the encoder. With `options.picker` this blocks while the
/// system picker is open (call it from a blocking thread).
pub(crate) fn prepare(
    options: ScreenOptions,
    counters: Arc<Counters>,
) -> Result<Prepared, PrepareError> {
    let mut source = if options.picker {
        cc_capture::open_picker(&cc_capture::PickerOptions {
            fps: options.fps,
            x11_display: options.display.clone(),
            // The system picker names the app (decision N57).
            app_id: Some(cc_capture::APP_ID.to_owned()),
            ..cc_capture::PickerOptions::default()
        })
    } else {
        cc_capture::open_default(options.display.as_deref(), options.fps)
    }
    .map_err(capture_err)?;
    let list = source.list_sources().map_err(capture_err)?;
    let chosen = match (&options.source_id, options.picker) {
        (Some(id), false) => list
            .iter()
            .find(|s| &s.id == id)
            .ok_or_else(|| format!("no capture source {id:?}"))?,
        _ => list
            .first()
            .ok_or_else(|| "no capture source available".to_string())?,
    };
    source.start(chosen).map_err(capture_err)?;
    let encoder = H264Encoder::new(&H264Config {
        bitrate_bps: options.bitrate_bps,
        fps: options.fps as f32,
        ..H264Config::default()
    })
    .map_err(|e| e.to_string())?;
    Ok(Prepared {
        source,
        encoder: Box::new(encoder),
        options,
        counters,
    })
}

/// Start the thread. Frames arrive on `out`; the end of the thread is reported on `ended`
/// (a panic and a failure to start the thread too, as [`Ended::Failed`]).
pub(crate) fn spawn(
    prepared: Prepared,
    out: tokio::sync::mpsc::UnboundedSender<Unit>,
    ended: tokio::sync::mpsc::UnboundedSender<Ended>,
) -> Pipeline {
    let stop = Arc::new(AtomicBool::new(false));
    let keyframe = Arc::new(AtomicBool::new(false));
    let counters = Arc::clone(&prepared.counters);
    let (token_tx, token_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel::<()>();
    // The first frame may go out at once.
    let _ = token_tx.send(());
    let thread = {
        let stop = Arc::clone(&stop);
        let keyframe = Arc::clone(&keyframe);
        let ended_in_thread = ended.clone();
        let spawned = std::thread::Builder::new()
            .name("cometcast-screen".into())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    run(prepared, &out, &token_rx, &stop, &keyframe)
                }))
                .unwrap_or_else(|payload| {
                    let what = payload
                        .downcast_ref::<&str>()
                        .map(|s| (*s).to_string())
                        .or_else(|| payload.downcast_ref::<String>().cloned())
                        .unwrap_or_else(|| "unknown panic".to_string());
                    Ended::Failed(format!("capture thread panicked: {what}"))
                });
                let _ = ended_in_thread.send(result);
                drop(done_tx);
            });
        match spawned {
            Ok(handle) => Some(handle),
            Err(e) => {
                let _ = ended.send(Ended::Failed(format!(
                    "cannot start the capture thread: {e}"
                )));
                None
            }
        }
    };
    Pipeline {
        stop,
        keyframe,
        counters,
        token_tx,
        thread,
        done_rx,
        stop_timeout: STOP_TIMEOUT,
    }
}

/// A picture the encoder skipped and that must still be sent.
#[derive(Debug, Clone, Copy)]
struct Owed {
    since: Instant,
    /// Whether the skipped encode was a static refresh (only for the counters).
    refresh: bool,
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
    let mut owed: Option<Owed> = None;
    let tick = Duration::from_secs(1) / p.options.fps.max(1);

    // Encode `frame`, honouring a pending key frame request, and send what comes out.
    // Returns whether a frame was sent, or an error text. The key frame flag is taken before
    // encoding and put back unless a frame went out, so a skipped picture does not lose the
    // request (and a request arriving meanwhile is not lost either).
    let encode_and_send = |enc: &mut dyn FrameEncoder, frame: &RawFrame, is_refresh: bool| {
        let key = keyframe.swap(false, Ordering::Relaxed);
        let ts = started.elapsed().as_micros() as u64;
        let encoded = match enc.encode(frame, key, ts) {
            Ok(Some(e)) => e,
            Ok(None) => {
                if key {
                    keyframe.store(true, Ordering::Relaxed);
                }
                c.skipped.fetch_add(1, Ordering::Relaxed);
                return Ok(false);
            }
            Err(e) => {
                if key {
                    keyframe.store(true, Ordering::Relaxed);
                }
                return Err(e);
            }
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
        let now = Instant::now();
        match capture {
            Capture::Ended => return Ended::SourceGone,
            Capture::Frame(frame) => {
                c.changed.fetch_add(1, Ordering::Relaxed);
                match encode_and_send(&mut *p.encoder, &frame, false) {
                    Ok(true) => {
                        have_token = false;
                        owed = None;
                    }
                    Ok(false) => {
                        owed.get_or_insert(Owed {
                            since: now,
                            refresh: false,
                        });
                    }
                    Err(e) => return Ended::Failed(e),
                }
                refresh.picture_changed(now);
                last = Some(frame);
            }
            Capture::Unchanged => {
                c.unchanged.fetch_add(1, Ordering::Relaxed);
                let Some(frame) = last.as_ref() else {
                    continue;
                };
                if owed.is_some_and(|o| now.duration_since(o.since) > OWED_TIMEOUT) {
                    // Give up on this picture; the next change or request starts afresh.
                    owed = None;
                }
                let want_key = keyframe.load(Ordering::Relaxed);
                // A skipped picture is offered again on every tick, outside the refresh
                // schedule, so skips do not use up its repeats.
                let retry = owed;
                let due_refresh = retry.is_none() && !want_key && refresh.poll(now);
                if want_key || retry.is_some() || due_refresh {
                    let is_refresh = !want_key && (due_refresh || retry.is_some_and(|o| o.refresh));
                    match encode_and_send(&mut *p.encoder, frame, is_refresh) {
                        Ok(true) => {
                            have_token = false;
                            owed = None;
                        }
                        Ok(false) => {
                            if due_refresh {
                                // This refresh counts as made once a retry gets it out; the
                                // retries themselves do not use up `repeats`.
                                owed = Some(Owed {
                                    since: now,
                                    refresh: true,
                                });
                            }
                        }
                        Err(e) => return Ended::Failed(e),
                    }
                }
            }
        }
    }
}
