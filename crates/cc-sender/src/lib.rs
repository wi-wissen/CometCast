// SPDX-License-Identifier: GPL-3.0-or-later
//! Sender core: joins a room, creates the offer and sends video.
//!
//! Three modes:
//!
//! * Test pattern (default): the sender replays a few pre-made VP8 key frames from
//!   `assets/test-frames.ivf` (made by `tools/gen-test-frames.py`). Every frame is a key
//!   frame, which is valid VP8 but not efficient.
//! * Screen ([`SenderOptions::screen`]): the screen is captured (`cc-capture`, X11 only),
//!   unchanged pictures are skipped, changed ones are encoded as H.264 (`cc-media`, OpenH264)
//!   and sent as RTP. See [`screen`] for the rules (one frame in flight, key frame on request,
//!   static refresh) and docs/capture.md.
//!
//! * External H.264 ([`SenderOptions::external`]): the host pushes finished H.264 access units
//!   (Android: `MediaCodec` output) into an [`ExternalInput`]; see [`external`] for the accepted
//!   formats and the queue rules (one frame in flight, bounded queue, drops are counted, key
//!   frame requests the host can read).
//!
//! Use: [`SenderCore::start`] -> loop on [`SenderCore::next_event`].

pub mod content;
pub mod external;
pub mod presenter;
mod screen;

pub use cc_transport::JoinProgress;
pub use content::{ContentSendError, ContentSender, ContentSenderOptions, ControlSender};
pub use external::{ExternalInput, ExternalOptions, ExternalStats, PushOutcome};
pub use presenter::{EndReason, Presentation, PresenterEvent, PresenterOptions, PresenterSession};
pub use screen::{ScreenOptions, ScreenStats};

use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use cc_proto::{ClientMsg, Role, RoomCode, ServerMsg};
use cc_transport::{
    ConnectionState, IceServerConfig, ServerAddr, Session, SessionConfig, SessionEvent,
    SignalingClient, TlsTrust, TransportError, VideoCodec, VideoSender, ice, join_room,
};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

/// The built-in test frames (IVF file with VP8 key frames).
const TEST_FRAMES_IVF: &[u8] = include_bytes!("../assets/test-frames.ivf");

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
    /// Send the screen instead of the test pattern (H.264). `None` = test pattern (VP8).
    pub screen: Option<ScreenOptions>,
    /// Send H.264 access units that the host pushes (cannot be combined with `screen`). The
    /// host keeps a clone of the [`ExternalInput`] and pushes into it.
    pub external: Option<ExternalInput>,
    /// Also open the `control` data channel next to the video, for images, PDFs, links and page
    /// commands: see [`SenderCore::control`]. Default `false`.
    pub control: bool,
    /// WebRTC settings. The codec list is forced to VP8 (test pattern) or H.264 (screen,
    /// external).
    /// If `session.ice_servers` is empty the core asks the server for its STUN/TURN list.
    pub session: SessionConfig,
    /// Extra trusted certificates for `wss://` / `https://` (default: system store only).
    pub trust: TlsTrust,
    /// Name shown on the display device when it has to approve the join (the server cleans
    /// it up; `None` = "Guest N").
    pub name: Option<String>,
    /// Receives [`JoinProgress::Pending`] when the room waits for the host's approval.
    pub join_progress: Option<mpsc::UnboundedSender<JoinProgress>>,
}

impl SenderOptions {
    /// Options for the given server and code with default settings.
    pub fn new(server: impl Into<String>, code: RoomCode) -> Self {
        Self {
            server: server.into(),
            code,
            duration: None,
            screen: None,
            external: None,
            control: false,
            session: SessionConfig::default(),
            trust: TlsTrust::system(),
            name: None,
            join_progress: None,
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
    /// The connection check code (`"123 456"`), once per connection, as soon as it is connected;
    /// the display shows the same digits (`cc_transport::sas`).
    ConnectionCode(String),
    /// The configured duration is over. The core stops after this event.
    Finished {
        /// Frames written to the track.
        frames_sent: u64,
    },
    /// The receiver left. The core stops after this event.
    PeerLeft,
    /// The host removed this sender from the room. The core stops after this event.
    RemovedByHost,
    /// Something failed. The core stops after this event.
    Error(String),
}

/// A presentation that could not start: the error, and the signalling connection (still in the
/// room) so that the presenter session stays joined.
pub(crate) struct StartFailed {
    pub(crate) sig: SignalingClient,
    pub(crate) error: TransportError,
}

/// How a presentation (one `run`) ended; the presenter session needs more than the events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RunEnd {
    /// Stopped on request, the duration was over, or the capture ended. `by_system`: the
    /// capture source went away (the system ended it).
    Stopped { by_system: bool },
    /// The display left the room.
    PeerLeft,
    /// The host removed this sender.
    RemovedByHost,
    /// A local or protocol error: `code` is a protocol code (`room_locked`, ...) or one of the
    /// local ones (`connection_lost`, `peer_failed`, `capture_unavailable`).
    Failed { code: String, message: String },
}

impl RunEnd {
    /// The end for an error message of the server.
    pub(crate) fn server(code: String, message: String) -> Self {
        Self::Failed { code, message }
    }

    pub(crate) fn lost() -> Self {
        Self::Failed {
            code: "connection_lost".into(),
            message: "signalling connection closed".into(),
        }
    }

    pub(crate) fn peer_failed(message: impl Into<String>) -> Self {
        Self::Failed {
            code: "peer_failed".into(),
            message: message.into(),
        }
    }
}

/// A running sender.
pub struct SenderCore {
    room_name: Option<String>,
    ice_servers: Vec<IceServerConfig>,
    counters: Arc<screen::Counters>,
    external: Option<ExternalInput>,
    control: Option<ControlSender>,
    events: mpsc::UnboundedReceiver<SenderEvent>,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<(SignalingClient, RunEnd)>,
}

impl SenderCore {
    /// Join the room, send the offer and start streaming once connected.
    /// Returns after the offer was sent; a wrong code fails here with `room_not_found`.
    pub async fn start(options: SenderOptions) -> Result<Self, TransportError> {
        let mut sig =
            SignalingClient::connect_with(&options.server, Role::Sender, &options.trust).await?;
        let room_name = join_room(
            &mut sig,
            options.code.clone(),
            options.name.clone(),
            options.join_progress.as_ref(),
        )
        .await?;
        Self::start_joined(sig, room_name, options)
            .await
            .map_err(|f| f.error)
    }

    /// Start a presentation on a signalling connection that is in the room already (the
    /// presenter session joins once and presents several times). On an error the connection is
    /// dropped, which leaves the room.
    pub(crate) async fn start_joined(
        sig: SignalingClient,
        room_name: Option<String>,
        mut options: SenderOptions,
    ) -> Result<Self, StartFailed> {
        let built = async {
            let addr = ServerAddr::parse(&options.server)?;
            if options.screen.is_some() && options.external.is_some() {
                return Err(TransportError::Invalid(
                    "screen and external input cannot be used together".into(),
                ));
            }
            let counters = Arc::new(screen::Counters::default());
            // Open the screen and the encoder before anything is negotiated, so a missing
            // display or encoder is an error of `start`.
            let prepared = match options.screen.take() {
                Some(screen_options) => Some(
                    screen::prepare(screen_options, Arc::clone(&counters))
                        .map_err(TransportError::Invalid)?,
                ),
                None => None,
            };
            let codec = if prepared.is_some() || options.external.is_some() {
                VideoCodec::H264
            } else {
                VideoCodec::Vp8
            };
            options.session.video_codecs = Some(vec![codec]);
            if options.session.ice_servers.is_empty() {
                // Best effort: no list (404, or a failed request) means no STUN/TURN.
                options.session.ice_servers =
                    ice::fetch_ice_servers_or_none(&addr, &options.trust).await;
            }
            let ice_servers = options.session.ice_servers.clone();
            let (session, session_events) = Session::new(options.session.clone()).await?;
            let control = match options.control {
                true => Some(ControlSender::new(session.create_control_channel().await?)),
                false => None,
            };
            let video = Arc::new(session.add_video_track(codec).await?);
            let offer = session.create_offer().await?;
            // The offer goes out first; candidates found meanwhile follow from the event channel.
            sig.send(ClientMsg::Offer { sdp: offer }).await?;
            Ok((
                counters,
                prepared,
                session,
                session_events,
                control,
                video,
                ice_servers,
            ))
        }
        .await;
        let (counters, prepared, session, session_events, control, video, ice_servers) = match built
        {
            Ok(b) => b,
            Err(error) => return Err(StartFailed { sig, error }),
        };

        let external = options.external.take();
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (stop_tx, stop_rx) = oneshot::channel();
        let task = tokio::spawn(run(
            sig,
            Arc::new(session),
            video,
            prepared,
            external.clone(),
            control.clone(),
            session_events,
            events_tx,
            stop_rx,
            options.duration,
        ));
        Ok(Self {
            room_name,
            ice_servers,
            counters,
            external,
            control,
            events: events_rx,
            stop: Some(stop_tx),
            task,
        })
    }

    /// The name the display device gave its room (`joined.room_name`); `None` when it gave
    /// none or the server is older.
    pub fn room_name(&self) -> Option<&str> {
        self.room_name.as_deref()
    }

    /// The STUN/TURN servers this sender uses (from the server's `/ice-servers`, or what the
    /// caller configured). Empty = none.
    pub fn ice_servers(&self) -> &[IceServerConfig] {
        &self.ice_servers
    }

    /// The content sender, if [`SenderOptions::control`] was set. Wait for
    /// [`ControlSender::wait_ready`] before sending.
    pub fn control(&self) -> Option<ControlSender> {
        self.control.clone()
    }

    /// Counters of the screen pipeline (all zero in test pattern mode).
    pub fn screen_stats(&self) -> ScreenStats {
        self.counters.snapshot()
    }

    /// Counters of the external input (all zero unless `SenderOptions::external` was set).
    pub fn external_stats(&self) -> ExternalStats {
        self.external
            .as_ref()
            .map(ExternalInput::stats)
            .unwrap_or_default()
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

    /// Ask the background task to finish; the events end when it has.
    pub(crate) fn request_stop(&mut self) {
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
    }

    /// Wait for the task and take the signalling connection back, with the way it ended.
    pub(crate) async fn join(mut self) -> (Option<SignalingClient>, RunEnd) {
        match (&mut self.task).await {
            Ok((sig, end)) => (Some(sig), end),
            Err(_) => (None, RunEnd::peer_failed("the sender task panicked")),
        }
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

/// The running screen pipeline and the tasks around it.
struct ScreenRun {
    pipeline: screen::Pipeline,
    frames: mpsc::UnboundedReceiver<screen::Unit>,
    ended: mpsc::UnboundedReceiver<screen::Ended>,
    /// When the previous frame was written; the RTP time step of the next one.
    last_sent: Option<Instant>,
    frame_time: Duration,
}

enum ScreenMsg {
    Frame(Option<screen::Unit>),
    Ended(Option<screen::Ended>),
}

#[allow(clippy::too_many_arguments)]
async fn run(
    mut sig: SignalingClient,
    session: Arc<Session>,
    video: Arc<VideoSender>,
    mut prepared: Option<screen::Prepared>,
    external: Option<ExternalInput>,
    control: Option<ControlSender>,
    mut session_events: mpsc::UnboundedReceiver<SessionEvent>,
    out: mpsc::UnboundedSender<SenderEvent>,
    mut stop: oneshot::Receiver<()>,
    duration: Option<Duration>,
) -> (SignalingClient, RunEnd) {
    let mut end = RunEnd::Stopped { by_system: false };
    let emit = |e: SenderEvent| {
        let _ = out.send(e);
    };
    let test = builtin_test_frames();
    let test_mode = prepared.is_none() && external.is_none();
    let frame_time = Duration::from_secs(1) / test.fps;
    let mut frame_tick = tokio::time::interval(frame_time);
    frame_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut report_tick = tokio::time::interval(Duration::from_secs(1));
    let mut streaming_since: Option<Instant> = None;
    let mut frames_sent: u64 = 0;
    let mut screen_run: Option<ScreenRun> = None;
    let mut keyframe_task: Option<tokio::task::JoinHandle<()>> = None;
    // External mode: presentation time and wall time of the previous frame (RTP time step).
    let mut ext_last: Option<(Option<u64>, Instant)> = None;
    loop {
        // The channels of the screen pipeline only exist once it runs; until then these
        // futures never complete.
        let screen_msg = async {
            match screen_run.as_mut() {
                Some(r) => tokio::select! {
                    unit = r.frames.recv() => ScreenMsg::Frame(unit),
                    ended = r.ended.recv() => ScreenMsg::Ended(ended),
                },
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            _ = &mut stop => break,
            msg = sig.recv() => match msg {
                None => {
                    emit(SenderEvent::Error("signalling connection closed".into()));
                    end = RunEnd::lost();
                    break;
                }
                Some(ServerMsg::Answer { sdp }) => {
                    if let Err(e) = session.accept_answer(sdp).await {
                        emit(SenderEvent::Error(format!("cannot use answer: {e}")));
                        end = RunEnd::peer_failed(format!("cannot use answer: {e}"));
                        break;
                    }
                }
                Some(ServerMsg::IceCandidate { candidate }) => {
                    // A bad candidate is not fatal: others may still work.
                    let _ = session.add_remote_candidate(&candidate).await;
                }
                Some(ServerMsg::PeerLeft) => {
                    emit(SenderEvent::PeerLeft);
                    end = RunEnd::PeerLeft;
                    break;
                }
                Some(ServerMsg::Error { code, .. }) if code == "removed_by_host" => {
                    emit(SenderEvent::RemovedByHost);
                    end = RunEnd::RemovedByHost;
                    break;
                }
                Some(ServerMsg::Error { code, message }) => {
                    emit(SenderEvent::Error(format!("server error {code}: {message}")));
                    end = RunEnd::server(code, message);
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
                        if let Some(code) = session.connection_code() {
                            emit(SenderEvent::ConnectionCode(code));
                        }
                        // External mode: units are accepted from now on and the host is asked
                        // for a key frame; later ones come from the peer's PLI requests.
                        if let Some(ext) = external.clone() {
                            ext.begin_streaming();
                            let v = Arc::clone(&video);
                            keyframe_task = Some(tokio::spawn(async move {
                                while v.next_keyframe_request().await {
                                    // Counted and rate limited inside.
                                    ext.request_keyframe();
                                }
                            }));
                        }
                        // A new peer starts the screen pipeline, whose first frame is a key
                        // frame; later key frames come from the peer's picture loss requests.
                        if let Some(p) = prepared.take() {
                            let (frames_tx, frames_rx) = mpsc::unbounded_channel();
                            let (ended_tx, ended_rx) = mpsc::unbounded_channel();
                            let fps = p.options.fps.max(1);
                            let pipeline = screen::spawn(p, frames_tx, ended_tx);
                            let requester = pipeline.keyframe_requester();
                            let v = Arc::clone(&video);
                            keyframe_task = Some(tokio::spawn(async move {
                                while v.next_keyframe_request().await {
                                    // Counted and rate limited inside.
                                    requester.request();
                                }
                            }));
                            screen_run = Some(ScreenRun {
                                pipeline,
                                frames: frames_rx,
                                ended: ended_rx,
                                last_sent: None,
                                frame_time: Duration::from_secs(1) / fps,
                            });
                        }
                    }
                    if s == ConnectionState::Failed {
                        emit(SenderEvent::State(s));
                        emit(SenderEvent::Error("WebRTC connection failed".into()));
                        end = RunEnd::peer_failed("WebRTC connection failed");
                        break;
                    }
                    emit(SenderEvent::State(s));
                }
                Some(SessionEvent::VideoTrack { .. } | SessionEvent::ControlChannel(_)) => {}
            },
            _ = frame_tick.tick(), if test_mode && streaming_since.is_some() => {
                let Some(since) = streaming_since else { continue };
                if duration.is_some_and(|d| since.elapsed() >= d) {
                    emit(SenderEvent::Finished { frames_sent });
                    break;
                }
                let frame = test.frames[(frames_sent as usize) % test.frames.len()].clone();
                if let Err(e) = video.send_frame(frame, frame_time).await {
                    emit(SenderEvent::Error(format!("cannot send frame: {e}")));
                    end = RunEnd::peer_failed(format!("cannot send frame: {e}"));
                    break;
                }
                frames_sent += 1;
            },
            unit = async {
                match &external {
                    Some(e) => e.next_unit().await,
                    None => std::future::pending().await,
                }
            }, if streaming_since.is_some() => {
                let (Some(unit), Some(ext)) = (unit, external.as_ref()) else { break };
                let wall = ext_last.map_or(frame_time, |(_, t)| t.elapsed());
                let step = external::rtp_step(ext_last.and_then(|(p, _)| p), unit.pts_us, wall);
                ext_last = Some((unit.pts_us, Instant::now()));
                let (len, key) = (unit.data.len(), unit.key);
                if let Err(e) = video.send_frame(Bytes::from(unit.data), step).await {
                    emit(SenderEvent::Error(format!("cannot send frame: {e}")));
                    end = RunEnd::peer_failed(format!("cannot send frame: {e}"));
                    break;
                }
                ext.count_sent(len, key);
                frames_sent += 1;
            },
            msg = screen_msg => match msg {
            ScreenMsg::Frame(unit) => {
                let (Some(unit), Some(r)) = (unit, screen_run.as_mut()) else { continue };
                // The RTP time advances by the real time since the previous frame, so that
                // pauses on a static screen do not make the picture play too fast later.
                let now = Instant::now();
                let step = r.last_sent.map_or(r.frame_time, |t| now - t);
                r.last_sent = Some(now);
                let step = step.max(Duration::from_millis(1));
                if let Err(e) = video.send_frame(Bytes::from(unit.data), step).await {
                    emit(SenderEvent::Error(format!("cannot send frame: {e}")));
                    end = RunEnd::peer_failed(format!("cannot send frame: {e}"));
                    break;
                }
                frames_sent += 1;
                // The transport has the frame: the pipeline may look at the screen again.
                r.pipeline.frame_taken();
            },
            ScreenMsg::Ended(ended) => {
                match ended {
                    Some(screen::Ended::Failed(e)) => {
                        emit(SenderEvent::Error(format!("screen capture failed: {e}")));
                        end = RunEnd::Failed {
                            code: "capture_unavailable".into(),
                            message: format!("screen capture failed: {e}"),
                        };
                    }
                    Some(screen::Ended::SourceGone) => {
                        emit(SenderEvent::Error("screen capture ended".into()));
                        end = RunEnd::Stopped { by_system: true };
                    }
                    Some(screen::Ended::Stopped) | None => {}
                }
                break;
            }
            },
            _ = report_tick.tick(), if streaming_since.is_some() => {
                // In test pattern mode the frame tick ends the run; the screen mode ends here.
                if let (Some(since), false) = (streaming_since, test_mode)
                    && duration.is_some_and(|d| since.elapsed() >= d)
                {
                    emit(SenderEvent::Finished { frames_sent });
                    break;
                }
                emit(SenderEvent::Streaming { frames_sent });
            }
        }
    }
    if let Some(t) = keyframe_task {
        t.abort();
    }
    if let Some(ext) = &external {
        ext.close();
    }
    if let Some(mut r) = screen_run {
        // Joining waits for a capture call that may be sleeping up to one frame time.
        let _ = tokio::task::spawn_blocking(move || r.pipeline.stop()).await;
    }
    // Closing the channel first lets the display deliver what is still on its way and see that
    // the presentation ended while this sender stays in the room.
    if let Some(c) = &control {
        c.channel().close_graceful(content::CLOSE_TIMEOUT).await;
    }
    let _ = session.close().await;
    (sig, end)
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

#[cfg(test)]
mod screen_tests;
