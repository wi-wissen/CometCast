// SPDX-License-Identifier: GPL-3.0-or-later
//! The screen as an HLS live stream: capture (`cc-capture`, X11 today), encode H.264
//! (`cc-media`, OpenH264, Baseline), wrap into MPEG-TS segments and hand them to a
//! [`SegmentStore`] that [`crate::hls::HlsServer`] serves. The receiver is then told to play
//! the playlist URL.
//!
//! Latency: expect several seconds. The segments are short (1 s by default, each starts
//! with an IDR frame), the playlist keeps the newest 6, and receivers typically start about
//! three segments behind the live edge and buffer more. There is no low-latency HLS
//! (no partial segments, no blocking playlist reload): receivers for plain HLS links do not
//! all support it.
//! UNCERTAIN: the real delay on an Apple TV or TV set was not measured (none available).
//!
//! The picture is encoded at a constant rate (default 10 fps): the capture's "unchanged"
//! answers repeat the last picture, so the stream never stalls on a still screen.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cc_capture::{Capture, RawFrame};
use cc_media::{BgraFrame, H264Config, H264Encoder};

use crate::error::{Result, SenderError};
use crate::hls::SegmentStore;
use crate::ts::TsMuxer;

/// Settings of the screen stream.
#[derive(Debug, Clone)]
pub struct ScreenOptions {
    /// X11 display such as `:99`; `None` uses `$DISPLAY`.
    pub display: Option<String>,
    /// Frames per second (1 to 30).
    pub fps: u32,
    /// Target bit rate in bit/s.
    pub bitrate_bps: u32,
    /// Segment length in milliseconds (500 to 10000).
    pub segment_ms: u32,
}

impl Default for ScreenOptions {
    /// UNCERTAIN: 10 fps, 2 Mbit/s and 1 s segments are starting values, not tuned.
    fn default() -> Self {
        Self {
            display: None,
            fps: 10,
            bitrate_bps: 2_000_000,
            segment_ms: 1000,
        }
    }
}

/// Counters of a running stream.
#[derive(Debug, Default)]
pub struct ScreenStats {
    /// Encoded pictures.
    pub frames: AtomicU64,
    /// Finished segments.
    pub segments: AtomicU64,
    /// Bytes of TS data produced.
    pub bytes: AtomicU64,
}

/// A running capture and encode thread.
pub struct ScreenStreamer {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<()>>>,
    /// Counters.
    pub stats: Arc<ScreenStats>,
    /// Size of the captured screen.
    pub size: (u32, u32),
}

impl ScreenStreamer {
    /// Open the screen and the encoder, then start producing segments into `store`.
    /// Errors that happen while opening are returned here.
    pub fn start(options: &ScreenOptions, store: Arc<SegmentStore>) -> Result<Self> {
        if !(1..=30).contains(&options.fps) {
            return Err(SenderError::Input("fps must be 1 to 30".into()));
        }
        if !(500..=10_000).contains(&options.segment_ms) {
            return Err(SenderError::Input(
                "segment length must be 500 to 10000 ms".into(),
            ));
        }
        let stop = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(ScreenStats::default());
        let (ready_tx, ready_rx) = mpsc::channel::<std::result::Result<(u32, u32), String>>();
        let opts = options.clone();
        let (stop2, stats2) = (Arc::clone(&stop), Arc::clone(&stats));
        let thread = std::thread::Builder::new()
            .name("cc-airplay-screen".into())
            .spawn(move || run(&opts, &store, &stop2, &stats2, &ready_tx))
            .map_err(|e| SenderError::Io(e.to_string()))?;
        match ready_rx.recv_timeout(Duration::from_secs(20)) {
            Ok(Ok(size)) => Ok(Self {
                stop,
                thread: Some(thread),
                stats,
                size,
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(SenderError::Unsupported(e))
            }
            Err(_) => {
                stop.store(true, Ordering::Relaxed);
                Err(SenderError::Timeout("opening the screen"))
            }
        }
    }

    /// Whether the thread has ended by itself (capture ended or failed).
    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Stop and report how the thread ended.
    pub fn stop(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Relaxed);
        match self.thread.take() {
            Some(t) => t
                .join()
                .unwrap_or_else(|_| Err(SenderError::Io("the screen thread panicked".into()))),
            None => Ok(()),
        }
    }
}

impl Drop for ScreenStreamer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn run(
    opts: &ScreenOptions,
    store: &SegmentStore,
    stop: &AtomicBool,
    stats: &ScreenStats,
    ready: &mpsc::Sender<std::result::Result<(u32, u32), String>>,
) -> Result<()> {
    let opened = (|| {
        let mut source = cc_capture::open_default(opts.display.as_deref(), opts.fps)
            .map_err(|e| e.to_string())?;
        let list = source.list_sources().map_err(|e| e.to_string())?;
        let chosen = list.first().ok_or("no capture source available")?.clone();
        source.start(&chosen).map_err(|e| e.to_string())?;
        let encoder = H264Encoder::new(&H264Config {
            bitrate_bps: opts.bitrate_bps,
            fps: opts.fps as f32,
            ..H264Config::default()
        })
        .map_err(|e| e.to_string())?;
        Ok::<_, String>((source, encoder, (chosen.width, chosen.height)))
    })();
    let (mut source, mut encoder, size) = match opened {
        Ok(v) => v,
        Err(e) => {
            let _ = ready.send(Err(e.clone()));
            return Err(SenderError::Unsupported(e));
        }
    };
    let _ = ready.send(Ok(size));

    let segment_ticks = u64::from(opts.segment_ms) * 90;
    let t0 = Instant::now();
    let mut mux = TsMuxer::new();
    let mut latest: Option<RawFrame> = None;
    let mut seg: Vec<u8> = Vec::new();
    let mut seg_start: Option<u64> = None;
    let pts_of = |t: Instant| 90_000 + (t.duration_since(t0).as_micros() as u64) * 9 / 100;

    let result = (|| -> Result<()> {
        while !stop.load(Ordering::Relaxed) {
            match source
                .next_frame()
                .map_err(|e| SenderError::Io(e.to_string()))?
            {
                Capture::Frame(f) => latest = Some(f),
                Capture::Unchanged => {}
                Capture::Ended => return Ok(()),
            }
            let Some(frame) = latest.as_ref() else {
                continue;
            };
            let now = Instant::now();
            let pts = pts_of(now);
            let due = seg_start.is_none_or(|s| pts - s >= segment_ticks);
            if due {
                encoder.force_keyframe();
            }
            let encoded = encoder
                .encode_bgra(
                    BgraFrame {
                        width: frame.width,
                        height: frame.height,
                        bgra: &frame.bgra,
                    },
                    now.duration_since(t0).as_micros() as u64,
                )
                .map_err(|e| SenderError::Io(e.to_string()))?;
            let Some(ef) = encoded else {
                continue;
            };
            stats.frames.fetch_add(1, Ordering::Relaxed);
            if ef.is_keyframe && due {
                if let Some(start) = seg_start {
                    let dur = (pts - start) as f64 / 90_000.0;
                    let data = std::mem::take(&mut seg);
                    stats.bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
                    store.push(dur, data)?;
                    stats.segments.fetch_add(1, Ordering::Relaxed);
                }
                seg_start = Some(pts);
                mux.write_tables(&mut seg);
            }
            if seg_start.is_some() {
                mux.write_frame(&mut seg, &ef.data, pts, ef.is_keyframe);
                if seg.len() > crate::hls::MAX_SEGMENT_BYTES {
                    return Err(SenderError::TooLarge("video segment"));
                }
            }
        }
        Ok(())
    })();
    source.stop();
    result
}

/// Everything that belongs to one screen cast: the segment store, the HLS server, the
/// capture thread.
pub struct ScreenCast {
    /// The HLS server (listens on our address towards the receiver).
    pub server: crate::hls::HlsServer,
    /// The capture and encode thread.
    pub streamer: ScreenStreamer,
    /// The segments.
    pub store: Arc<SegmentStore>,
    /// The playlist URL given to the receiver.
    pub url: String,
}

/// Start capturing, serve it on our address towards the receiver (only the receiver may
/// fetch), wait for two segments, then tell the receiver to play the playlist.
pub fn start_cast(
    session: &mut crate::client::Session,
    options: &ScreenOptions,
) -> Result<ScreenCast> {
    let device_ip = session
        .device()
        .addrs
        .first()
        .copied()
        .ok_or_else(|| SenderError::Input("the device has no address".into()))?;
    let local = session.local_ip();
    let store = SegmentStore::new(6);
    let mut streamer = ScreenStreamer::start(options, Arc::clone(&store))?;
    let server = crate::hls::HlsServer::start(
        std::net::SocketAddr::new(local, 0),
        Arc::clone(&store),
        Some(vec![device_ip]),
    )?;
    let end = Instant::now() + Duration::from_secs(20);
    while store.total() < 2 {
        if streamer.is_finished() {
            streamer.stop()?;
            return Err(SenderError::Io("the screen capture ended early".into()));
        }
        if Instant::now() > end {
            return Err(SenderError::Timeout("waiting for the first video segments"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let url = server.playlist_url(local);
    session.play(&url)?;
    Ok(ScreenCast {
        server,
        streamer,
        store,
        url,
    })
}

impl ScreenCast {
    /// Stop the stream and the server. The playlist gets its end marker first.
    pub fn stop(&mut self) {
        self.store.end();
        let _ = self.streamer.stop();
        self.server.stop();
    }
}
