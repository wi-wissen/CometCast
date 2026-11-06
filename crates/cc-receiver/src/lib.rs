// SPDX-License-Identifier: GPL-3.0-or-later
//! Receiver core: creates a room, answers the sender's offer and reports incoming video.
//!
//! This is the M1 slice (native peer for the signalling protocol). There is no decoding
//! or rendering yet: the core only counts RTP packets, bytes, frames and key frames. Later
//! parts (queue, toolbar state, deletion rule) arrive with the apps.
//!
//! On a server that requires proof-of-work (open mode) the core fetches and solves the
//! ALTCHA puzzle by itself when the server answers `altcha_required`.
//!
//! Content: if [`ReceiverOptions::content`] is set, the `control` data channel of the sender
//! is accepted and its images, links, PDF documents and page commands arrive as
//! [`ContentEvent`]s from [`ReceiverCore::take_content_events`] (docs/web-m2.md, `cc-files`).
//! Links are only reported, never opened. Without the option the channel is closed.
//!
//! Use: [`ReceiverCore::start`] -> show [`ReceiverCore::room_code`] -> loop on
//! [`ReceiverCore::next_event`].

mod content;

use std::sync::Arc;
use std::time::Duration;

use cc_proto::{ClientMsg, Role, RoomCode, ServerMsg};
use cc_transport::{
    ConnectionState, IceServerConfig, MediaSnapshot, ServerAddr, Session, SessionConfig,
    SessionEvent, SignalingClient, TlsTrust, TransportError, VideoCodec, altcha, ice,
};
pub use cc_files::{Blob, ContentError, ContentEvent, ContentOptions, ImageMime};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

/// How long we wait for the server to hand out a room code.
/// UNCERTAIN: placeholder, not measured.
const ROOM_TIMEOUT: Duration = Duration::from_secs(10);

/// How long after the sender's offer arrived the WebRTC connection may take to become connected.
/// UNCERTAIN: 30 s is a placeholder; real values depend on networks and are not measured.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Settings for [`ReceiverCore::start`].
#[derive(Debug, Clone)]
pub struct ReceiverOptions {
    /// Signalling server as `host:port` or a `ws://`, `wss://`, `http://`, `https://` URL.
    pub server: String,
    /// WebRTC settings (local bind address, codecs). If `session.ice_servers` is empty the
    /// core asks the server for its STUN/TURN list (`/ice-servers`).
    pub session: SessionConfig,
    /// Extra trusted certificates for `wss://` / `https://` (default: system store only).
    pub trust: TlsTrust,
    /// Accept content (images, links, PDFs, page commands) on the `control` data channel.
    /// `None` (default): the channel is closed. The application must then read
    /// [`ReceiverCore::take_content_events`]; when it does not, the sender is slowed down.
    pub content: Option<ContentOptions>,
}

impl ReceiverOptions {
    /// Options for the given server with default WebRTC settings.
    pub fn new(server: impl Into<String>) -> Self {
        Self {
            server: server.into(),
            session: SessionConfig::default(),
            trust: TlsTrust::system(),
            content: None,
        }
    }
}

/// The content events of a [`ReceiverCore`] (from [`ReceiverCore::take_content_events`]).
/// The queue behind it is bounded: while the application does not read, the sender is slowed down.
pub struct ContentEvents {
    rx: mpsc::Receiver<ContentEvent>,
}

impl ContentEvents {
    /// Next event, or `None` after the core has stopped and all were read.
    pub async fn next(&mut self) -> Option<ContentEvent> {
        self.rx.recv().await
    }
}

/// How long the core waits, after the sender left, for the content that was still in the
/// queue to be handed to the application. UNCERTAIN: placeholder.
const CONTENT_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

/// What the receiver reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiverEvent {
    /// A sender joined the room.
    PeerJoined,
    /// WebRTC connection state changed.
    State(ConnectionState),
    /// The sender's video track arrived.
    VideoTrack {
        /// MIME type, e.g. `video/VP8`.
        mime: String,
        /// Parsed codec, if known.
        codec: Option<VideoCodec>,
    },
    /// Counters of incoming video, sent about once per second after the track arrived.
    Stats(MediaSnapshot),
    /// The sender left. The core stops after this event.
    /// UNCERTAIN: one sender per core in this version; a second sender needs a new core.
    PeerLeft,
    /// Something failed. The core stops after this event.
    Error(String),
}

/// The server's answer to `create_room`.
enum RoomReply {
    Created(RoomCode),
    AltchaRequired,
}

/// Wait for the answer to `create_room`.
async fn next_room_reply(sig: &mut SignalingClient) -> Result<RoomReply, TransportError> {
    tokio::time::timeout(ROOM_TIMEOUT, async {
        match sig.recv().await {
            Some(ServerMsg::RoomCreated { code }) => Ok(RoomReply::Created(code)),
            Some(ServerMsg::Error { code, .. }) if code == "altcha_required" => {
                Ok(RoomReply::AltchaRequired)
            }
            Some(ServerMsg::Error { code, message }) => {
                Err(TransportError::Server { code, message })
            }
            Some(other) => Err(TransportError::Protocol(format!(
                "expected room_created, got {other:?}"
            ))),
            None => Err(TransportError::Closed),
        }
    })
    .await
    .map_err(|_| TransportError::Timeout("waiting for room_created"))?
}

/// A running receiver.
pub struct ReceiverCore {
    code: RoomCode,
    ice_servers: Vec<IceServerConfig>,
    session: Arc<Session>,
    events: mpsc::UnboundedReceiver<ReceiverEvent>,
    content_events: Option<mpsc::Receiver<ContentEvent>>,
    content_width: mpsc::UnboundedSender<u32>,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl ReceiverCore {
    /// Connect to the server, create a room and start answering. Returns once the room code is known.
    pub async fn start(options: ReceiverOptions) -> Result<Self, TransportError> {
        let addr = ServerAddr::parse(&options.server)?;
        let mut sig =
            SignalingClient::connect_with(&options.server, Role::Receiver, &options.trust).await?;
        sig.send(ClientMsg::CreateRoom { altcha: None }).await?;
        let code = match next_room_reply(&mut sig).await? {
            RoomReply::Created(code) => code,
            // Open servers always want a solved puzzle (docs/server.md). The connection
            // stays open after `altcha_required`, so we ask again on the same one.
            RoomReply::AltchaRequired => {
                let payload = altcha::fetch_and_solve(&addr, &options.trust).await?;
                sig.send(ClientMsg::CreateRoom {
                    altcha: Some(payload),
                })
                .await?;
                match next_room_reply(&mut sig).await? {
                    RoomReply::Created(code) => code,
                    RoomReply::AltchaRequired => {
                        return Err(TransportError::Server {
                            code: "altcha_required".into(),
                            message: "the server still wants a puzzle solution".into(),
                        });
                    }
                }
            }
        };

        let mut session_config = options.session;
        if session_config.ice_servers.is_empty() {
            // Best effort: no list (404, or a failed request) means no STUN/TURN.
            session_config.ice_servers =
                ice::fetch_ice_servers_or_none(&addr, &options.trust).await;
        }
        let ice_servers = session_config.ice_servers.clone();
        let (session, session_events) = Session::new(session_config).await?;
        let session = Arc::new(session);
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (stop_tx, stop_rx) = oneshot::channel();
        let (width_tx, width_rx) = mpsc::unbounded_channel();
        let (content_tx, content_rx) = mpsc::channel(content::EVENT_QUEUE);
        let content = options.content.map(|o| ContentSetup {
            options: o,
            widths: width_rx,
            events: content_tx,
        });
        let has_content = content.is_some();
        let task = tokio::spawn(run(
            sig,
            Arc::clone(&session),
            session_events,
            events_tx,
            stop_rx,
            content,
        ));
        Ok(Self {
            code,
            ice_servers,
            session,
            events: events_rx,
            content_events: has_content.then_some(content_rx),
            content_width: width_tx,
            stop: Some(stop_tx),
            task,
        })
    }

    /// The STUN/TURN servers this receiver uses (from the server's `/ice-servers`, or what the
    /// caller configured). Empty = none.
    pub fn ice_servers(&self) -> &[IceServerConfig] {
        &self.ice_servers
    }

    /// The code the sender must enter.
    pub fn room_code(&self) -> &RoomCode {
        &self.code
    }

    /// Ask the sender for a new key frame (picture loss indication), for example after a
    /// decoding error. Fails if no video track has arrived yet.
    pub async fn request_keyframe(&self) -> Result<(), TransportError> {
        self.session.request_keyframe().await
    }

    /// Current video counters.
    pub fn stats(&self) -> MediaSnapshot {
        self.session.stats()
    }

    /// Next event, or `None` after the core has stopped and all events were read.
    pub async fn next_event(&mut self) -> Option<ReceiverEvent> {
        self.events.recv().await
    }

    /// Take the stream of content events (once; `None` if [`ReceiverOptions::content`] was not
    /// set or the stream was taken already). It is separate from [`Self::next_event`] so that
    /// an application can wait for both at the same time.
    pub fn take_content_events(&mut self) -> Option<ContentEvents> {
        self.content_events.take().map(|rx| ContentEvents { rx })
    }

    /// Set the width in pixels that document pages are rendered at (the size of the window
    /// or screen). The shown page is rendered again; the result arrives as a
    /// `PageRendered` event. The value is clamped to the output limits of `cc-docs`.
    pub fn set_content_target_width(&self, width: u32) {
        let _ = self.content_width.send(width);
    }

    /// Stop and close everything.
    pub async fn stop(mut self) {
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
        let _ = (&mut self.task).await;
    }
}

impl Drop for ReceiverCore {
    fn drop(&mut self) {
        // Ask the background task to finish: it closes the WebRTC session and the
        // signalling connection itself. (It is not aborted, so the cleanup runs.)
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
    }
}

/// What the background loop needs to run the content task.
struct ContentSetup {
    options: ContentOptions,
    widths: mpsc::UnboundedReceiver<u32>,
    events: mpsc::Sender<ContentEvent>,
}

/// The background loop: signalling messages in, session events in, receiver events out.
async fn run(
    mut sig: SignalingClient,
    session: Arc<Session>,
    mut session_events: mpsc::UnboundedReceiver<SessionEvent>,
    out: mpsc::UnboundedSender<ReceiverEvent>,
    mut stop: oneshot::Receiver<()>,
    mut content: Option<ContentSetup>,
) {
    let mut content_task: Option<tokio::task::JoinHandle<()>> = None;
    let emit = |e: ReceiverEvent| {
        let _ = out.send(e);
    };
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    let mut have_track = false;
    let mut offer_at: Option<tokio::time::Instant> = None;
    let mut connected = false;
    loop {
        tokio::select! {
            _ = &mut stop => break,
            msg = sig.recv() => match msg {
                None => {
                    emit(ReceiverEvent::Error("signalling connection closed".into()));
                    break;
                }
                Some(ServerMsg::PeerJoined) => emit(ReceiverEvent::PeerJoined),
                Some(ServerMsg::Offer { sdp }) => {
                    offer_at.get_or_insert_with(tokio::time::Instant::now);
                    let answer = match session.accept_offer(sdp).await {
                        Ok(a) => a,
                        Err(e) => {
                            emit(ReceiverEvent::Error(format!("cannot answer offer: {e}")));
                            break;
                        }
                    };
                    if sig.send(ClientMsg::Answer { sdp: answer }).await.is_err() {
                        emit(ReceiverEvent::Error("signalling connection closed".into()));
                        break;
                    }
                }
                Some(ServerMsg::IceCandidate { candidate }) => {
                    // A bad candidate is not fatal: others may still work.
                    let _ = session.add_remote_candidate(&candidate).await;
                }
                Some(ServerMsg::PeerLeft) => {
                    // Content that is still on its way is handed over first.
                    if let Some(t) = content_task.take() {
                        let _ = tokio::time::timeout(CONTENT_DRAIN_TIMEOUT, t).await;
                    }
                    emit(ReceiverEvent::PeerLeft);
                    break;
                }
                Some(ServerMsg::Error { code, message }) => {
                    emit(ReceiverEvent::Error(format!("server error {code}: {message}")));
                    break;
                }
                // hello/room_created/joined/answer/control: nothing to do here yet.
                Some(_) => {}
            },
            ev = session_events.recv() => match ev {
                None => break,
                Some(SessionEvent::LocalIceCandidate(c)) => {
                    let _ = sig.send(ClientMsg::IceCandidate { candidate: c }).await;
                }
                Some(SessionEvent::State(s)) => {
                    emit(ReceiverEvent::State(s));
                    if s == ConnectionState::Connected {
                        connected = true;
                    }
                    if s == ConnectionState::Failed {
                        emit(ReceiverEvent::Error("WebRTC connection failed".into()));
                        break;
                    }
                }
                Some(SessionEvent::ControlChannel(channel)) => match content.take() {
                    Some(c) => {
                        content_task =
                            Some(content::spawn(channel, c.options, c.widths, c.events));
                    }
                    // Content is not wanted: refuse the channel.
                    None => channel.close().await,
                },
                Some(SessionEvent::VideoTrack { mime, codec }) => {
                    have_track = true;
                    emit(ReceiverEvent::VideoTrack { mime, codec });
                }
            },
            _ = tick.tick() => {
                if !connected && offer_at.is_some_and(|t| t.elapsed() > CONNECT_TIMEOUT) {
                    emit(ReceiverEvent::Error(format!(
                        "no WebRTC connection {} s after the offer arrived",
                        CONNECT_TIMEOUT.as_secs()
                    )));
                    break;
                }
                if have_track {
                    emit(ReceiverEvent::Stats(session.stats()));
                }
            }
        }
    }
    let _ = session.close().await;
}
