// SPDX-License-Identifier: GPL-3.0-or-later
//! Content sending over the `control` data channel: images, PDF documents, links and page
//! commands (message format: docs/web-m2.md; logic: `cc-files`).
//!
//! * [`ControlSender`] wraps an open channel. Sending a transfer waits whenever the channel's
//!   send buffer is full (back-pressure from `cc-transport`), so memory stays bounded.
//! * [`ContentSender`] is a content-only sender (no video): joins the room, offers a session
//!   that has only the data channel, and exposes a [`ControlSender`]. The web client has the
//!   same mode ("Send file or link").
//! * [`SenderCore`](crate::SenderCore) can also carry a channel next to the video
//!   ([`SenderOptions::control`](crate::SenderOptions::control)).

use std::sync::Arc;
use std::time::Duration;

use cc_files::{
    ImageMime, Message, Outgoing, SendError, cmd_message, document_transfer, handover_message,
    image_transfer, link_message,
};
use cc_proto::{ClientMsg, ControlCmd, Role, RoomCode, ServerMsg};
use cc_transport::JoinProgress;
use cc_transport::{
    ConnectionState, ControlChannel, ServerAddr, Session, SessionConfig, SessionEvent,
    SignalingClient, TlsTrust, TransportError, ice, join_room,
};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::{RunEnd, SenderEvent, StartFailed};

/// How long [`ControlSender::wait_ready`] callers typically wait for the channel to open.
/// UNCERTAIN: placeholder, not measured.
pub const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// How long closing the channel may wait for the peer. UNCERTAIN: placeholder.
pub(crate) const CLOSE_TIMEOUT: Duration = Duration::from_secs(3);

/// How long [`ControlSender::finish`] waits for the data to leave. UNCERTAIN: placeholder.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(30);

/// Why content could not be sent.
#[derive(Debug, thiserror::Error)]
pub enum ContentSendError {
    /// The content itself was refused before anything was sent (empty, too large, not a PDF,
    /// bad link).
    #[error("{0}")]
    Content(#[from] SendError),
    /// The channel failed.
    #[error("{0}")]
    Transport(#[from] TransportError),
}

/// What a presenter reads from the display on the `control` channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMessage {
    /// The display shows this PDF page of that many.
    Page {
        /// The shown page, from 1.
        page: u32,
        /// The page count, from 1.
        pages: u32,
    },
    /// The display stored the hand-over token that was sent (`handover_ok`).
    HandoverOk,
    /// The display's pause flags (`pause_state`, docs/design/pause.md): the source of truth for
    /// the presenter's UI.
    PauseState {
        /// The presenter's own flag, as the display holds it (the echo of `cmd pause`).
        presenter: bool,
        /// The display's own flag.
        display: bool,
    },
}

/// Sends content on an open `control` channel. Cheap to clone.
#[derive(Debug, Clone)]
pub struct ControlSender {
    channel: ControlChannel,
}

impl ControlSender {
    /// Wrap a channel (from [`Session::create_control_channel`]).
    pub fn new(channel: ControlChannel) -> Self {
        Self { channel }
    }

    /// The channel itself.
    pub fn channel(&self) -> &ControlChannel {
        &self.channel
    }

    /// Wait until the channel is open (the peer is connected).
    pub async fn wait_ready(&self, timeout: Duration) -> Result<(), TransportError> {
        self.channel.wait_open(timeout).await
    }

    async fn run<'a>(
        &self,
        items: impl Iterator<Item = Outgoing<'a>>,
    ) -> Result<(), ContentSendError> {
        for item in items {
            match item {
                Outgoing::Text(t) => self.channel.send_text(&t).await?,
                Outgoing::Binary(b) => self.channel.send_binary(b).await?,
            }
        }
        Ok(())
    }

    /// Send an image (PNG, JPEG, WebP or GIF, at most 10 MiB). The bytes are not decoded here.
    pub async fn send_image(
        &self,
        name: &str,
        mime: ImageMime,
        bytes: &[u8],
    ) -> Result<(), ContentSendError> {
        self.run(image_transfer(name, mime, bytes)?).await
    }

    /// Send a PDF document. The receiver renders it and shows the first page; use
    /// [`Self::send_cmd`] to turn pages.
    pub async fn send_document(&self, name: &str, bytes: &[u8]) -> Result<(), ContentSendError> {
        self.run(document_transfer(name, bytes)?).await
    }

    /// Send a link. Only `http` and `https` are allowed. The receiver's person decides
    /// whether to open it.
    pub async fn send_link(&self, url: &str) -> Result<(), ContentSendError> {
        let m = link_message(url)?;
        self.channel.send_text(&m).await?;
        Ok(())
    }

    /// Send a remote-control command (next/previous page, go to page, stop; `Pause` and
    /// `Resume` tell the display about the presenter's pause, see `SenderCore::pause`).
    pub async fn send_cmd(&self, cmd: ControlCmd) -> Result<(), ContentSendError> {
        self.channel.send_text(&cmd_message(cmd)).await?;
        Ok(())
    }

    /// Ask the display to hold a single-use hand-over token (`{"kind":"handover","token":T}`,
    /// native-apps.md 20.5). The answer is [`DisplayMessage::HandoverOk`] from
    /// [`Self::recv_message`]. Refused here when `token` is not 1 to 64 base64url characters.
    pub async fn send_handover(&self, token: &str) -> Result<(), ContentSendError> {
        let m = handover_message(token).ok_or(SendError::BadToken)?;
        self.channel.send_text(&m).await?;
        Ok(())
    }

    /// The next message of the display that a presenter acts on, validated like the web client
    /// does: a `page` message (the shown PDF page and the page count; decision 13.31) or the
    /// answer to a hand-over token. Other messages are skipped. `None` when the channel closed.
    /// Only one task should read the channel.
    pub async fn recv_message(&self) -> Option<DisplayMessage> {
        loop {
            match self.channel.recv().await? {
                cc_transport::ControlData::Text(t) => {
                    match cc_files::wire::parse_message(
                        &t,
                        cc_files::MAX_IMAGE_BYTES,
                        cc_files::MAX_DOCUMENT_BYTES,
                    ) {
                        Ok(Message::Page { page, pages }) => {
                            return Some(DisplayMessage::Page { page, pages });
                        }
                        Ok(Message::HandoverOk) => return Some(DisplayMessage::HandoverOk),
                        Ok(Message::PauseState { presenter, display }) => {
                            return Some(DisplayMessage::PauseState { presenter, display });
                        }
                        _ => {}
                    }
                }
                cc_transport::ControlData::Binary(_) => {}
            }
        }
    }

    /// The next `page` message of the display (see [`Self::recv_message`]; a hand-over answer
    /// is skipped here). `None` when the channel closed.
    pub async fn recv_page(&self) -> Option<(u32, u32)> {
        loop {
            if let DisplayMessage::Page { page, pages } = self.recv_message().await? {
                return Some((page, pages));
            }
        }
    }

    /// Wait until everything sent has left (acknowledged by the peer's transport), at most
    /// `timeout`. `false` on timeout or when the channel closed.
    pub async fn flush(&self, timeout: Duration) -> bool {
        self.channel.flush(timeout).await
    }

    /// [`Self::flush`] with the default timeout. Call it before stopping a content-only
    /// sender, otherwise the end of a transfer can be lost.
    pub async fn finish(&self) -> bool {
        self.flush(FLUSH_TIMEOUT).await
    }
}

/// Settings for [`ContentSender::start`].
#[derive(Debug, Clone)]
pub struct ContentSenderOptions {
    /// Signalling server as `host:port` (or a full `ws://` URL).
    pub server: String,
    /// Room code from the receiver.
    pub code: RoomCode,
    /// WebRTC settings. If `session.ice_servers` is empty the core asks the server for its list.
    pub session: SessionConfig,
    /// Extra trusted certificates for `wss://` / `https://`.
    pub trust: TlsTrust,
    /// Name shown on the display device when it has to approve the join.
    pub name: Option<String>,
    /// Receives [`JoinProgress::Pending`] when the room waits for the host's approval.
    pub join_progress: Option<mpsc::UnboundedSender<JoinProgress>>,
    /// Do not ask the server for its STUN/TURN list even if `session.ice_servers` is empty (the
    /// caller asked already and keeps the outcome). Default `false`.
    pub skip_ice_fetch: bool,
}

impl ContentSenderOptions {
    /// Options for the given server and code with default settings.
    pub fn new(server: impl Into<String>, code: RoomCode) -> Self {
        Self {
            server: server.into(),
            code,
            session: SessionConfig::default(),
            trust: TlsTrust::system(),
            name: None,
            join_progress: None,
            skip_ice_fetch: false,
        }
    }
}

/// A content-only sender: a WebRTC session with a data channel and no media.
pub struct ContentSender {
    room_name: Option<String>,
    control: ControlSender,
    events: mpsc::UnboundedReceiver<SenderEvent>,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<(SignalingClient, RunEnd)>,
}

impl ContentSender {
    /// Join the room and send the offer. Returns after the offer was sent; a wrong code fails
    /// here with `room_not_found`. Then call [`ControlSender::wait_ready`] before sending.
    pub async fn start(options: ContentSenderOptions) -> Result<Self, TransportError> {
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

    /// Start on a signalling connection that is in the room already (see
    /// `SenderCore::start_joined`).
    pub(crate) async fn start_joined(
        sig: SignalingClient,
        room_name: Option<String>,
        mut options: ContentSenderOptions,
    ) -> Result<Self, StartFailed> {
        let built = async {
            let addr = ServerAddr::parse(&options.server)?;
            // No video codecs at all: the offer has only the application section.
            options.session.video_codecs = Some(Vec::new());
            if options.session.ice_servers.is_empty() && !options.skip_ice_fetch {
                options.session.ice_servers =
                    ice::fetch_ice_servers_or_none(&addr, &options.trust).await;
            }
            let (session, session_events) = Session::new(options.session.clone()).await?;
            let channel = session.create_control_channel().await?;
            let offer = session.create_offer().await?;
            sig.send(ClientMsg::Offer { sdp: offer }).await?;
            Ok::<_, TransportError>((session, session_events, channel))
        }
        .await;
        let (session, session_events, channel) = match built {
            Ok(b) => b,
            Err(error) => return Err(StartFailed { sig, error }),
        };

        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (stop_tx, stop_rx) = oneshot::channel();
        let task = tokio::spawn(run(
            sig,
            Arc::new(session),
            channel.clone(),
            session_events,
            events_tx,
            stop_rx,
        ));
        Ok(Self {
            room_name,
            control: ControlSender::new(channel),
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

    /// The handle to send content with.
    pub fn control(&self) -> ControlSender {
        self.control.clone()
    }

    /// Next event, or `None` after the sender has stopped and all events were read.
    pub async fn next_event(&mut self) -> Option<SenderEvent> {
        self.events.recv().await
    }

    /// Stop and close everything. Call [`ControlSender::finish`] first if content was sent.
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

impl Drop for ContentSender {
    fn drop(&mut self) {
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
    }
}

async fn run(
    mut sig: SignalingClient,
    session: Arc<Session>,
    channel: ControlChannel,
    mut session_events: mpsc::UnboundedReceiver<SessionEvent>,
    out: mpsc::UnboundedSender<SenderEvent>,
    mut stop: oneshot::Receiver<()>,
) -> (SignalingClient, RunEnd) {
    let mut end = RunEnd::Stopped { by_system: false };
    let emit = |e: SenderEvent| {
        let _ = out.send(e);
    };
    let mut code_sent = false;
    loop {
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
                    emit(SenderEvent::State(s));
                    if s == ConnectionState::Connected && !code_sent {
                        code_sent = true;
                        if let Some(code) = session.connection_code() {
                            emit(SenderEvent::ConnectionCode(code));
                        }
                    }
                    if s == ConnectionState::Failed {
                        emit(SenderEvent::Error("WebRTC connection failed".into()));
                        end = RunEnd::peer_failed("WebRTC connection failed");
                        break;
                    }
                }
                Some(_) => {}
            },
        }
    }
    // Closing the channel first lets the peer deliver the last messages before the connection
    // goes away.
    channel.close_graceful(CLOSE_TIMEOUT).await;
    let _ = session.close().await;
    (sig, end)
}
