// SPDX-License-Identifier: GPL-3.0-or-later
//! The media path: screen capture (`cc-capture`) -> H.264 (`cc-media`) -> encrypted RTP over
//! UDP ([`crate::streamer`]), and the loop that serves RTCP and the Cast channel while it runs.
//!
//! UNCERTAIN: no scaling is done. The OFFER announces the capture size (rounded down to even
//! numbers); a receiver that cannot decode that size (for example 4K on an older Chromecast)
//! will fail. Static screens produce no frames; a key frame is re-sent every 2 s of
//! silence so the receiver does not starve (a guess, not Open Screen behaviour).

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use cc_capture::{Capture, CaptureSource, RawFrame, ScreenSource};
use cc_media::{H264Config, H264Encoder};
use tokio::sync::mpsc;

use crate::device_auth::DeviceAuth;
use crate::error::CastError;
use crate::offer::VideoOffer;
use crate::rtp::RtcpEvent;
use crate::session::{self, Negotiated, SessionConfig, parse_receiver_status};
use crate::streamer::{StreamStats, VideoStreamer};

/// One encoded picture on its way to the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoUnit {
    /// Annex-B H.264 access unit.
    pub data: Vec<u8>,
    /// IDR frame.
    pub key_frame: bool,
    /// Time since the stream started.
    pub pts: Duration,
}

/// Why [`run_stream`] returned without an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamEnd {
    /// The frame source ended.
    SourceEnded,
    /// The receiver's status no longer lists our app (stopped from the TV or another sender).
    AppStopped,
}

/// Send frames and serve the receiver until something ends. `key_request` is set when the
/// receiver asks for a picture refresh (the producer must then make a key frame).
pub async fn run_stream(
    negotiated: &mut Negotiated,
    streamer: &mut VideoStreamer,
    mut frames: mpsc::Receiver<VideoUnit>,
    key_request: &AtomicBool,
) -> Result<StreamEnd, CastError> {
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    loop {
        tokio::select! {
            unit = frames.recv() => {
                let Some(u) = unit else { return Ok(StreamEnd::SourceEnded) };
                streamer.send_frame(&u.data, u.key_frame, u.pts).await?;
            }
            events = streamer.recv_rtcp() => {
                for e in events? {
                    if e == RtcpEvent::PictureLoss {
                        key_request.store(true, Ordering::Relaxed);
                    }
                }
            }
            _ = tick.tick() => {
                streamer.sender_report_if_due().await?;
            }
            msg = negotiated.connection.recv() => {
                let m = msg?;
                // A RECEIVER_STATUS that no longer lists our session means the app was stopped.
                if let Some((_, st)) = m.text_payload().and_then(|t| parse_receiver_status(&t)) {
                    if !st.apps.iter().any(|a| a.session_id == negotiated.app.session_id) {
                        return Ok(StreamEnd::AppStopped);
                    }
                }
            }
        }
    }
}

/// Settings for the capture thread.
pub struct CaptureLoop {
    /// Started capture source.
    pub source: Box<dyn ScreenSource + Send>,
    /// Encoder.
    pub encoder: H264Encoder,
    /// Look at the screen this often.
    pub fps: u32,
}

/// Run the capture/encode loop on the current (blocking) thread until `stop` is set or the
/// source ends. Pictures go to `tx`.
pub fn capture_loop(
    mut c: CaptureLoop,
    tx: &mpsc::Sender<VideoUnit>,
    key_request: &AtomicBool,
    stop: &AtomicBool,
) -> Result<(), CastError> {
    const IDLE_KEYFRAME: Duration = Duration::from_secs(2);
    let started = Instant::now();
    let mut last: Option<RawFrame> = None;
    let mut last_sent = Instant::now();
    let _ = c.fps;
    while !stop.load(Ordering::Relaxed) {
        let cap = c
            .source
            .next_frame()
            .map_err(|e| CastError::Media(e.to_string()))?;
        let frame = match cap {
            Capture::Ended => return Ok(()),
            Capture::Frame(f) => {
                last = Some(f);
                last.as_ref()
            }
            Capture::Unchanged => {
                if key_request.load(Ordering::Relaxed) || last_sent.elapsed() >= IDLE_KEYFRAME {
                    key_request.store(true, Ordering::Relaxed);
                    last.as_ref()
                } else {
                    None
                }
            }
        };
        let Some(f) = frame else { continue };
        let want_key = key_request.swap(false, Ordering::Relaxed);
        if want_key {
            c.encoder.force_keyframe();
        }
        let pts = started.elapsed();
        let out = c
            .encoder
            .encode_bgra(
                cc_media::BgraFrame {
                    width: f.width,
                    height: f.height,
                    bgra: &f.bgra,
                },
                pts.as_micros() as u64,
            )
            .map_err(|e| CastError::Media(e.to_string()))?;
        match out {
            Some(e) => {
                last_sent = Instant::now();
                let unit = VideoUnit {
                    data: e.data,
                    key_frame: e.is_keyframe,
                    pts,
                };
                if tx.blocking_send(unit).is_err() {
                    return Ok(());
                }
            }
            None => {
                if want_key {
                    key_request.store(true, Ordering::Relaxed);
                }
            }
        }
    }
    Ok(())
}

/// Everything `mirror_screen` needs.
pub struct MirrorConfig {
    /// Receiver address (`ip:8009`).
    pub addr: SocketAddr,
    /// Device auth policy.
    pub auth: DeviceAuth,
    /// X11 display such as `:99`; `None` uses `$DISPLAY`.
    pub display: Option<String>,
    /// Capture rate.
    pub fps: u32,
    /// Encoder target bit rate in kbit/s.
    pub bitrate_kbps: u32,
    /// Use payload type 96 (see `offer::PAYLOAD_TYPE_ANDROID_HACK`).
    pub android_rtp_hack: bool,
}

/// Mirror the screen to the receiver until `stop` completes, the source ends, or the receiver
/// goes away. Messages for the user go through `log`.
pub async fn mirror_screen(
    cfg: MirrorConfig,
    log: &(dyn Fn(&str) + Sync),
    stop: impl Future<Output = ()>,
) -> Result<StreamStats, CastError> {
    let mut source = cc_capture::open_default(cfg.display.as_deref(), cfg.fps)
        .map_err(|e| CastError::Media(e.to_string()))?;
    let list = source
        .list_sources()
        .map_err(|e| CastError::Media(e.to_string()))?;
    let chosen: &CaptureSource = list
        .first()
        .ok_or_else(|| CastError::Media("no capture source".into()))?;
    let (w, h) = (chosen.width & !1, chosen.height & !1);
    let video = VideoOffer::new(w, h, cfg.fps, cfg.bitrate_kbps * 1000, cfg.android_rtp_hack);

    log(&format!("connecting to {} ...", cfg.addr));
    let mut sc = SessionConfig::new(cfg.addr, cfg.auth);
    sc.android_rtp_hack = cfg.android_rtp_hack;
    let mut negotiated = session::negotiate(&sc, &video).await?;
    for w in &negotiated.warnings {
        log(&format!("WARNING: {w}"));
    }
    match &negotiated.auth_outcome {
        Some(o) => log(&format!(
            "device authentication verified ({})",
            o.device_subject.as_deref().unwrap_or("?")
        )),
        None => log("device authentication NOT verified"),
    }
    log(&format!(
        "app {} started (session {}), streaming {w}x{h} to {}",
        negotiated.app.app_id, negotiated.app.session_id, negotiated.remote_udp
    ));

    source
        .start(chosen)
        .map_err(|e| CastError::Media(e.to_string()))?;
    let encoder = H264Encoder::new(&H264Config {
        bitrate_bps: cfg.bitrate_kbps * 1000,
        fps: cfg.fps as f32,
        ..H264Config::default()
    })
    .map_err(|e| CastError::Media(e.to_string()))?;

    let mut streamer = VideoStreamer::new(negotiated.remote_udp, negotiated.stream.clone()).await?;
    let key_request = Arc::new(AtomicBool::new(true));
    let stop_flag = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel(4);
    let thread = {
        let (k, s) = (Arc::clone(&key_request), Arc::clone(&stop_flag));
        let cl = CaptureLoop {
            source,
            encoder,
            fps: cfg.fps,
        };
        tokio::task::spawn_blocking(move || {
            let r = capture_loop(cl, &tx, &k, &s);
            // Dropping `tx` here ends the stream loop.
            r
        })
    };

    let result = tokio::select! {
        r = run_stream(&mut negotiated, &mut streamer, rx, &key_request) => r,
        () = stop => Ok(StreamEnd::SourceEnded),
    };
    stop_flag.store(true, Ordering::Relaxed);
    let stats = streamer.stats();
    negotiated.stop().await;
    match thread.await {
        Ok(Err(e)) if result.is_ok() => return Err(e),
        _ => {}
    }
    match result? {
        StreamEnd::AppStopped => log("the receiver stopped the app"),
        StreamEnd::SourceEnded => {}
    }
    Ok(stats)
}
