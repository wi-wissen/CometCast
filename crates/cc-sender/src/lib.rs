// SPDX-License-Identifier: GPL-3.0-or-later
//! Sender core: joins a room, creates the offer and sends a synthetic video stream.
//!
//! This is the M1 slice. There is no screen capture and no video encoder yet (no GPL
//! encoders are allowed in this project). For tests the sender replays a few pre-made VP8
//! key frames from `assets/test-frames.ivf` (made by `tools/gen-test-frames.py`).
//! Every frame is a key frame, which is valid VP8 but not efficient.
//!
//! Use: [`SenderCore::start`] -> loop on [`SenderCore::next_event`].

use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use cc_proto::{ClientMsg, Role, RoomCode, ServerMsg};
use cc_transport::{
    ConnectionState, IceServerConfig, ServerAddr, Session, SessionConfig, SessionEvent,
    SignalingClient, TlsTrust, TransportError, VideoCodec, VideoSender, ice,
};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

/// The built-in test frames (IVF file with VP8 key frames).
const TEST_FRAMES_IVF: &[u8] = include_bytes!("../assets/test-frames.ivf");

/// How long we wait for the server to confirm that we joined.
/// UNCERTAIN: placeholder, not measured.
const JOIN_TIMEOUT: Duration = Duration::from_secs(10);

/// Pre-made VP8 frames and their frame rate.
#[derive(Debug, Clone)]
pub struct TestFrames {
    /// Encoded VP8 frames, played in a loop.
    pub frames: Vec<Bytes>,
    /// Frames per second from the IVF header.
    pub fps: u32,
}

/// Why an IVF file could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid IVF data: {0}")]
pub struct IvfError(&'static str);

/// Read an IVF container (32-byte header, then frames with a 12-byte header each).
pub fn parse_ivf(data: &[u8]) -> Result<TestFrames, IvfError> {
    if data.len() < 32 || &data[0..4] != b"DKIF" {
        return Err(IvfError("missing DKIF header"));
    }
    let header_len = usize::from(u16::from_le_bytes([data[6], data[7]]));
    let rate = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
    let scale = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
    if scale == 0 || rate == 0 {
        return Err(IvfError("zero time base"));
    }
    let mut pos = header_len.max(32);
    let mut frames = Vec::new();
    while pos + 12 <= data.len() {
        let len = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        let start = pos + 12;
        let end = start + len as usize;
        if end > data.len() {
            return Err(IvfError("frame is cut off"));
        }
        frames.push(Bytes::copy_from_slice(&data[start..end]));
        pos = end;
    }
    if frames.is_empty() {
        return Err(IvfError("no frames"));
    }
    Ok(TestFrames {
        frames,
        fps: (rate / scale).max(1),
    })
}

/// The frames that ship with this crate.
pub fn builtin_test_frames() -> TestFrames {
    // The file is part of the crate, so a parse error would be a build-time bug.
    parse_ivf(TEST_FRAMES_IVF).expect("built-in test-frames.ivf is valid")
}

/// Settings for [`SenderCore::start`].
#[derive(Debug, Clone)]
pub struct SenderOptions {
    /// Signalling server as `host:port` (or a full `ws://` URL).
    pub server: String,
    /// Room code from the receiver.
    pub code: RoomCode,
    /// Stop sending after this long. `None` means until stopped.
    pub duration: Option<Duration>,
    /// WebRTC settings. The codec list is forced to VP8.
    /// If `session.ice_servers` is empty the core asks the server for its STUN/TURN list.
    pub session: SessionConfig,
    /// Extra trusted certificates for `wss://` / `https://` (default: system store only).
    pub trust: TlsTrust,
}

impl SenderOptions {
    /// Options for the given server and code with default settings.
    pub fn new(server: impl Into<String>, code: RoomCode) -> Self {
        Self {
            server: server.into(),
            code,
            duration: None,
            session: SessionConfig::default(),
            trust: TlsTrust::system(),
        }
    }
}

/// What the sender reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SenderEvent {
    /// WebRTC connection state changed.
    State(ConnectionState),
    /// Number of frames sent so far, about once per second while streaming.
    Streaming {
        /// Frames written to the track.
        frames_sent: u64,
    },
    /// The configured duration is over. The core stops after this event.
    Finished {
        /// Frames written to the track.
        frames_sent: u64,
    },
    /// The receiver left. The core stops after this event.
    PeerLeft,
    /// Something failed. The core stops after this event.
    Error(String),
}

/// A running sender.
pub struct SenderCore {
    ice_servers: Vec<IceServerConfig>,
    events: mpsc::UnboundedReceiver<SenderEvent>,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl SenderCore {
    /// Join the room, send the offer and start streaming once connected.
    /// Returns after the offer was sent; a wrong code fails here with `room_not_found`.
    pub async fn start(mut options: SenderOptions) -> Result<Self, TransportError> {
        let addr = ServerAddr::parse(&options.server)?;
        let mut sig =
            SignalingClient::connect_with(&options.server, Role::Sender, &options.trust).await?;
        sig.send(ClientMsg::JoinRoom {
            code: options.code.clone(),
        })
        .await?;
        tokio::time::timeout(JOIN_TIMEOUT, async {
            match sig.recv().await {
                Some(ServerMsg::Joined) => Ok(()),
                Some(ServerMsg::Error { code, message }) => {
                    Err(TransportError::Server { code, message })
                }
                Some(other) => Err(TransportError::Protocol(format!(
                    "expected joined, got {other:?}"
                ))),
                None => Err(TransportError::Closed),
            }
        })
        .await
        .map_err(|_| TransportError::Timeout("waiting for joined"))??;

        options.session.video_codecs = Some(vec![VideoCodec::Vp8]);
        if options.session.ice_servers.is_empty() {
            // Best effort: no list (404, or a failed request) means no STUN/TURN.
            options.session.ice_servers =
                ice::fetch_ice_servers_or_none(&addr, &options.trust).await;
        }
        let ice_servers = options.session.ice_servers.clone();
        let (session, session_events) = Session::new(options.session).await?;
        let video = session.add_video_track(VideoCodec::Vp8).await?;
        let offer = session.create_offer().await?;
        // The offer goes out first; candidates found meanwhile follow from the event channel.
        sig.send(ClientMsg::Offer { sdp: offer }).await?;

        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (stop_tx, stop_rx) = oneshot::channel();
        let task = tokio::spawn(run(
            sig,
            Arc::new(session),
            video,
            session_events,
            events_tx,
            stop_rx,
            options.duration,
        ));
        Ok(Self {
            ice_servers,
            events: events_rx,
            stop: Some(stop_tx),
            task,
        })
    }

    /// The STUN/TURN servers this sender uses (from the server's `/ice-servers`, or what the
    /// caller configured). Empty = none.
    pub fn ice_servers(&self) -> &[IceServerConfig] {
        &self.ice_servers
    }

    /// Next event, or `None` after the core has stopped and all events were read.
    pub async fn next_event(&mut self) -> Option<SenderEvent> {
        self.events.recv().await
    }

    /// Stop and close everything.
    pub async fn stop(mut self) {
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
        let _ = (&mut self.task).await;
    }
}

impl Drop for SenderCore {
    fn drop(&mut self) {
        // Ask the background task to finish: it closes the WebRTC session and the
        // signalling connection itself. (It is not aborted, so the cleanup runs.)
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
    }
}

async fn run(
    mut sig: SignalingClient,
    session: Arc<Session>,
    video: VideoSender,
    mut session_events: mpsc::UnboundedReceiver<SessionEvent>,
    out: mpsc::UnboundedSender<SenderEvent>,
    mut stop: oneshot::Receiver<()>,
    duration: Option<Duration>,
) {
    let emit = |e: SenderEvent| {
        let _ = out.send(e);
    };
    let test = builtin_test_frames();
    let frame_time = Duration::from_secs(1) / test.fps;
    let mut frame_tick = tokio::time::interval(frame_time);
    frame_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut report_tick = tokio::time::interval(Duration::from_secs(1));
    let mut streaming_since: Option<Instant> = None;
    let mut frames_sent: u64 = 0;
    loop {
        tokio::select! {
            _ = &mut stop => break,
            msg = sig.recv() => match msg {
                None => {
                    emit(SenderEvent::Error("signalling connection closed".into()));
                    break;
                }
                Some(ServerMsg::Answer { sdp }) => {
                    if let Err(e) = session.accept_answer(sdp).await {
                        emit(SenderEvent::Error(format!("cannot use answer: {e}")));
                        break;
                    }
                }
                Some(ServerMsg::IceCandidate { candidate }) => {
                    // A bad candidate is not fatal: others may still work.
                    let _ = session.add_remote_candidate(&candidate).await;
                }
                Some(ServerMsg::PeerLeft) => {
                    emit(SenderEvent::PeerLeft);
                    break;
                }
                Some(ServerMsg::Error { code, message }) => {
                    emit(SenderEvent::Error(format!("server error {code}: {message}")));
                    break;
                }
                Some(_) => {}
            },
            ev = session_events.recv() => match ev {
                None => break,
                Some(SessionEvent::LocalIceCandidate(c)) => {
                    let _ = sig.send(ClientMsg::IceCandidate { candidate: c }).await;
                }
                Some(SessionEvent::State(s)) => {
                    if s == ConnectionState::Connected && streaming_since.is_none() {
                        streaming_since = Some(Instant::now());
                    }
                    if s == ConnectionState::Failed {
                        emit(SenderEvent::State(s));
                        emit(SenderEvent::Error("WebRTC connection failed".into()));
                        break;
                    }
                    emit(SenderEvent::State(s));
                }
                Some(SessionEvent::VideoTrack { .. }) => {}
            },
            _ = frame_tick.tick(), if streaming_since.is_some() => {
                let Some(since) = streaming_since else { continue };
                if duration.is_some_and(|d| since.elapsed() >= d) {
                    emit(SenderEvent::Finished { frames_sent });
                    break;
                }
                let frame = test.frames[(frames_sent as usize) % test.frames.len()].clone();
                if let Err(e) = video.send_frame(frame, frame_time).await {
                    emit(SenderEvent::Error(format!("cannot send frame: {e}")));
                    break;
                }
                frames_sent += 1;
            },
            _ = report_tick.tick(), if streaming_since.is_some() => {
                emit(SenderEvent::Streaming { frames_sent });
            }
        }
    }
    let _ = session.close().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_frames_are_vp8_key_frames() {
        let t = builtin_test_frames();
        assert_eq!(t.fps, 15);
        assert!(t.frames.len() >= 2);
        for f in &t.frames {
            // VP8 key frame: bit 0 of the frame tag is 0, start code 9d 01 2a follows.
            assert_eq!(f[0] & 1, 0);
            assert_eq!(&f[3..6], &[0x9d, 0x01, 0x2a]);
        }
    }

    #[test]
    fn parse_ivf_rejects_garbage() {
        assert!(parse_ivf(b"nope").is_err());
        let mut cut = TEST_FRAMES_IVF.to_vec();
        cut.truncate(cut.len() - 5);
        assert!(parse_ivf(&cut).is_err());
    }
}
