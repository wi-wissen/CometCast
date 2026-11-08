// SPDX-License-Identifier: GPL-3.0-or-later
//! The presenter session: join the room **once**, present several times (native-apps.md 20.1).
//!
//! [`PresenterSession::join`] connects and joins the room (the host approves once). Inside the
//! joined session [`PresenterSession::present`] starts a presentation on a fresh WebRTC session
//! over the same signalling connection (a screen, host-pushed H.264, or content only) and
//! [`PresenterSession::stop_presenting`] ends only that presentation: the display returns to
//! Idle with the same code, this sender stays in the room, and the next presentation needs no
//! new approval. [`PresenterSession::leave`] leaves the room.
//!
//! How the display learns that a presentation ended while this sender stays: every
//! presentation has a `control` channel next to the media; stopping sends `cmd stop` (the web
//! display closes what it shows), then closes the channel gracefully. The native receiver
//! reports the end of the channel as `PresentationStopped`; a new offer on the same signalling
//! connection does the same. UNCERTAIN: a web display does not make a fresh session for the
//! next offer of a sender that is still in the room (it never needed to), so a second
//! presentation to a web display is not covered here.
//!
//! The display's `page` messages are read from the channel and reported as
//! [`PresenterEvent::Page`]; the connection check code as [`PresenterEvent::ConnectionCode`].
//!
//! Pause (docs/design/pause.md): [`PresenterSession::pause`] stops the video of a media
//! presentation at once and tells the display (`cmd pause`); [`PresenterSession::resume`]
//! tells it and sends again, beginning with a key frame. The display's `pause_state` answers
//! arrive as [`PresenterEvent::PauseState`] and are the truth for the UI: the presenter's own
//! flag counts as confirmed only when the display echoes it. A content-only presentation has
//! no screen: pause is ignored there.

use std::time::Duration;

use cc_proto::{ControlCmd, Role, RoomCode, ServerMsg};
use cc_transport::{
    ConnectionState, JoinProgress, ServerAddr, SessionConfig, SignalingClient, TlsTrust,
    TransportError, ice, join_room_with,
};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use crate::content::{ContentSender, ContentSenderOptions, ControlSender, DisplayMessage};
use crate::{
    ExternalInput, RunEnd, ScreenOptions, SenderCore, SenderEvent, SenderOptions, StartFailed,
};

/// How long a stop waits for `cmd stop` to leave. UNCERTAIN: placeholder.
const STOP_FLUSH: Duration = Duration::from_secs(2);

/// How long [`PresenterSession::request_handover`] waits for the control channel to open.
/// UNCERTAIN: placeholder.
const HANDOVER_READY: Duration = Duration::from_secs(10);

/// How long [`PresenterSession::request_handover`] waits for the display's `handover_ok`
/// (a display without hand-over support never answers). UNCERTAIN: placeholder.
const HANDOVER_ACK: Duration = Duration::from_secs(4);

/// Settings for [`PresenterSession::join`].
#[derive(Debug, Clone)]
pub struct PresenterOptions {
    /// Signalling server as `host:port` (or a full `ws://` URL).
    pub server: String,
    /// Room code from the display.
    pub code: RoomCode,
    /// WebRTC settings of every presentation. If `session.ice_servers` is empty each
    /// presentation asks the server for its STUN/TURN list.
    pub session: SessionConfig,
    /// Extra trusted certificates for `wss://` / `https://`.
    pub trust: TlsTrust,
    /// Name shown on the display when it has to approve the join.
    pub name: Option<String>,
    /// Receives [`JoinProgress::Pending`] when the room waits for the host's approval.
    pub join_progress: Option<mpsc::UnboundedSender<JoinProgress>>,
    /// A single-use hand-over token the display issued to this person (iOS, native-apps.md
    /// 20.5): the display admits the join without asking. `None` = an ordinary join.
    pub handover: Option<String>,
}

impl PresenterOptions {
    /// Options for the given server and code with default settings.
    pub fn new(server: impl Into<String>, code: RoomCode) -> Self {
        Self {
            server: server.into(),
            code,
            session: SessionConfig::default(),
            trust: TlsTrust::system(),
            name: None,
            join_progress: None,
            handover: None,
        }
    }
}

/// How the server's STUN/TURN list (`/ice-servers`) turned out for this session (spec 6,
/// decision 13.45): only when a connection then fails the apps use it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IceState {
    /// A list, none (404) or the caller's own: nothing to say.
    Fine,
    /// The server refused the list (`rate_limited`, `banned`, `forbidden`).
    Refused,
    /// The request failed otherwise (unexpected status, no answer, a bad answer).
    Failed,
}

impl IceState {
    fn of(e: &TransportError) -> Self {
        match e {
            TransportError::Server { code, .. }
                if matches!(code.as_str(), "rate_limited" | "banned" | "forbidden") =>
            {
                Self::Refused
            }
            _ => Self::Failed,
        }
    }

    /// The code of a failed WebRTC connection: `ice_refused` or `ice_failed` when the list
    /// was the problem, else the code as it is.
    fn refine(self, code: String) -> String {
        match (code.as_str(), self) {
            ("peer_failed", Self::Refused) => "ice_refused".into(),
            ("peer_failed", Self::Failed) => "ice_failed".into(),
            _ => code,
        }
    }
}

/// What to present.
#[derive(Debug, Clone)]
pub enum Presentation {
    /// Content (images, PDFs, links): a data channel without media.
    Content,
    /// Media: the screen (`screen`), H.264 pushed by the host (`external`) or, with neither,
    /// the built-in test stream. The control channel opens next to the video.
    Media {
        /// Capture the screen.
        screen: Option<ScreenOptions>,
        /// H.264 access units the host pushes.
        external: Option<ExternalInput>,
        /// Stop by itself after this long.
        duration: Option<Duration>,
    },
}

/// Why the session ended (terminal): [`PresenterEvent::Ended`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// The host removed this sender (`removed_by_host`).
    RemovedByHost,
    /// The display left the room: the room is gone (`display_left`).
    DisplayLeft,
}

impl EndReason {
    /// The code the apps use.
    pub fn code(self) -> &'static str {
        match self {
            Self::RemovedByHost => "removed_by_host",
            Self::DisplayLeft => "display_left",
        }
    }
}

/// What a [`PresenterSession`] reports after the join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenterEvent {
    /// The WebRTC connection of the running presentation changed state.
    State(ConnectionState),
    /// The connection check code of the running presentation (`"123 456"`), once per connection.
    ConnectionCode(String),
    /// The display shows this PDF page of that many (decision 13.31).
    Page {
        /// The shown page, from 1.
        page: u32,
        /// The page count, from 1.
        pages: u32,
    },
    /// The display's pause flags (`pause_state`, docs/design/pause.md): sent whenever either
    /// flag changes and once when the control channel opens (both `false`). `presenter` is the
    /// echo of [`PresenterSession::pause`]; `display` is set when the display froze the
    /// picture on its own. The picture is frozen while either is set.
    PauseState {
        /// The presenter's flag, as the display holds it.
        presenter: bool,
        /// The display's own flag.
        display: bool,
    },
    /// Frames sent so far, about once per second while streaming media.
    Streaming {
        /// Frames written to the track.
        frames_sent: u64,
    },
    /// The presentation is connected (media flows, or the channel is open).
    Presenting,
    /// The presentation ended and the session is still joined: on request, by the duration, or
    /// because the system ended the capture.
    PresentingStopped,
    /// The session is over: the host removed this sender or the display left. Terminal.
    Ended(EndReason),
    /// Something failed. Terminal. `code` is a protocol code (`room_locked`, ...) or a local one
    /// (`connection_lost`, `peer_failed`, `capture_unavailable`, `unknown`); `message` is for
    /// logs only.
    Error {
        /// The code the apps map.
        code: String,
        /// English text for logs.
        message: String,
    },
}

enum Cmd {
    Present {
        what: Presentation,
        reply: oneshot::Sender<Result<ControlSender, TransportError>>,
    },
    StopPresenting,
    /// Pause (`true`) or resume (`false`) the running screen presentation.
    Pause(bool),
    /// The control channel for a hand-over: the running presentation's, or a content-only one
    /// that is started for it (and stays quiet: no events for the app).
    Handover {
        reply: oneshot::Sender<Result<ControlSender, TransportError>>,
    },
    Leave,
}

/// A joined session. See the module docs.
pub struct PresenterSession {
    room_name: Option<String>,
    cmds: mpsc::UnboundedSender<Cmd>,
    events: mpsc::UnboundedReceiver<PresenterEvent>,
    /// Counts the `handover_ok` answers of the display.
    handover_acks: watch::Sender<u64>,
    task: JoinHandle<()>,
}

impl PresenterSession {
    /// Connect and join the room. Waits for the host's approval where the room asks for it
    /// (progress on [`PresenterOptions::join_progress`]); dropping the future withdraws the
    /// request. Fails with the server's error (`room_not_found`, `join_denied`, ...).
    pub async fn join(options: PresenterOptions) -> Result<Self, TransportError> {
        let mut sig =
            SignalingClient::connect_with(&options.server, Role::Sender, &options.trust).await?;
        let room_name = join_room_with(
            &mut sig,
            options.code.clone(),
            options.name.clone(),
            options.handover.clone(),
            options.join_progress.as_ref(),
        )
        .await?;
        // Ask for the STUN/TURN list once per session; every presentation uses the outcome.
        let mut options = options;
        let mut ice_state = IceState::Fine;
        if options.session.ice_servers.is_empty() {
            let addr = ServerAddr::parse(&options.server)?;
            match ice::fetch_ice_servers(&addr, &options.trust).await {
                Ok(list) => options.session.ice_servers = list,
                Err(e) => {
                    eprintln!(
                        "cometcast: GET /ice-servers {}; connecting without STUN/TURN servers",
                        ice::describe_ice_failure(&e)
                    );
                    ice_state = IceState::of(&e);
                }
            }
        }
        let (cmds_tx, cmds_rx) = mpsc::unbounded_channel();
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (acks_tx, _) = watch::channel(0u64);
        let task = tokio::spawn(run(
            sig,
            options,
            ice_state,
            cmds_rx,
            events_tx,
            acks_tx.clone(),
        ));
        Ok(Self {
            room_name: room_name.clone(),
            cmds: cmds_tx,
            events: events_rx,
            handover_acks: acks_tx,
            task,
        })
    }

    /// The name the display gave its room, if any (decision 49: lenient).
    pub fn room_name(&self) -> Option<&str> {
        self.room_name.as_deref()
    }

    /// Start a presentation. Returns after the offer was sent with the handle of the
    /// presentation's control channel (wait for [`ControlSender::wait_ready`] before sending
    /// content). Fails when a presentation runs already, or when it cannot start (for example
    /// the capture is refused: then the session stays joined).
    pub async fn present(&self, what: Presentation) -> Result<ControlSender, TransportError> {
        let (reply, answer) = oneshot::channel();
        self.cmds
            .send(Cmd::Present { what, reply })
            .map_err(|_| TransportError::Closed)?;
        answer.await.map_err(|_| TransportError::Closed)?
    }

    /// Hand the approval over to another connection of this person (the iOS broadcast
    /// extension, native-apps.md 20.5): ask the display, on the content control channel, to
    /// hold the single-use `token` (128 random bits as base64url, valid 60 s on the display),
    /// and wait for its `handover_ok`. A content-only channel is opened for this when no
    /// presentation runs; it makes no event for the app. Call [`Self::leave`] afterwards. Fails
    /// when the display does not answer within a few seconds (an old display, a web display,
    /// a lost connection): the caller then lets the other connection join the ordinary way.
    pub async fn request_handover(&self, token: &str) -> Result<(), TransportError> {
        let (reply, answer) = oneshot::channel();
        self.cmds
            .send(Cmd::Handover { reply })
            .map_err(|_| TransportError::Closed)?;
        let control = answer.await.map_err(|_| TransportError::Closed)??;
        control.wait_ready(HANDOVER_READY).await?;
        let mut acks = self.handover_acks.subscribe();
        control
            .send_handover(token)
            .await
            .map_err(|e| TransportError::Invalid(e.to_string()))?;
        tokio::time::timeout(HANDOVER_ACK, acks.changed())
            .await
            .map_err(|_| TransportError::Timeout("waiting for handover_ok"))?
            .map_err(|_| TransportError::Closed)
    }

    /// End the running presentation only. The session stays joined; the end is reported as
    /// [`PresenterEvent::PresentingStopped`].
    pub fn stop_presenting(&self) {
        let _ = self.cmds.send(Cmd::StopPresenting);
    }

    /// Pause the running screen presentation: the video stops at once (nothing leaves the
    /// device) and the display keeps the last picture. Ignored without a media presentation.
    /// The display's answer is [`PresenterEvent::PauseState`]. Idempotent.
    pub fn pause(&self) {
        let _ = self.cmds.send(Cmd::Pause(true));
    }

    /// Resume after [`Self::pause`]: the display is told and video flows again, beginning
    /// with a key frame. Idempotent.
    pub fn resume(&self) {
        let _ = self.cmds.send(Cmd::Pause(false));
    }

    /// Next event, or `None` after the session ended and all events were read.
    pub async fn next_event(&mut self) -> Option<PresenterEvent> {
        self.events.recv().await
    }

    /// Leave the room (a running presentation ends with it).
    pub async fn leave(mut self) {
        let _ = self.cmds.send(Cmd::Leave);
        let _ = (&mut self.task).await;
    }
}

impl Drop for PresenterSession {
    fn drop(&mut self) {
        let _ = self.cmds.send(Cmd::Leave);
    }
}

/// The running presentation.
struct Running {
    core: Core,
    control: ControlSender,
    pages: JoinHandle<()>,
    presenting: bool,
    /// Started only to carry a hand-over: the app is told nothing about it.
    quiet: bool,
}

enum Core {
    Media(SenderCore),
    Content(ContentSender),
}

impl Core {
    async fn next_event(&mut self) -> Option<SenderEvent> {
        match self {
            Self::Media(c) => c.next_event().await,
            Self::Content(c) => c.next_event().await,
        }
    }

    /// Pause or resume the media (content-only presentations have none).
    async fn set_paused(&self, paused: bool) {
        if let Self::Media(c) = self {
            if paused {
                c.pause().await;
            } else {
                c.resume().await;
            }
        }
    }

    fn request_stop(&mut self) {
        match self {
            Self::Media(c) => c.request_stop(),
            Self::Content(c) => c.request_stop(),
        }
    }

    async fn join(self) -> (Option<SignalingClient>, RunEnd) {
        match self {
            Self::Media(c) => c.join().await,
            Self::Content(c) => c.join().await,
        }
    }
}

async fn start(
    sig: SignalingClient,
    options: &PresenterOptions,
    what: Presentation,
) -> Result<Running, StartFailed> {
    match what {
        Presentation::Content => {
            let mut o = ContentSenderOptions::new(options.server.clone(), options.code.clone());
            o.session = options.session.clone();
            o.trust = options.trust.clone();
            o.skip_ice_fetch = true;
            let core = ContentSender::start_joined(sig, None, o).await?;
            let control = core.control();
            Ok(Running::new(Core::Content(core), control))
        }
        Presentation::Media {
            screen,
            external,
            duration,
        } => {
            let mut o = SenderOptions::new(options.server.clone(), options.code.clone());
            o.session = options.session.clone();
            o.trust = options.trust.clone();
            o.skip_ice_fetch = true;
            o.screen = screen;
            o.external = external;
            o.duration = duration;
            o.control = true;
            let core = SenderCore::start_joined(sig, None, o).await?;
            let control = core.control().expect("the control channel was asked for");
            Ok(Running::new(Core::Media(core), control))
        }
    }
}

impl Running {
    fn new(core: Core, control: ControlSender) -> Self {
        Self {
            core,
            control,
            pages: tokio::spawn(std::future::ready(())),
            presenting: false,
            quiet: false,
        }
    }
}

enum Next {
    Cmd(Option<Cmd>),
    Idle(Option<ServerMsg>),
    Running(Option<SenderEvent>),
    Page(Option<DisplayMessage>),
}

/// Start a presentation on the signalling connection and keep it in `running`. When it cannot
/// start the connection comes back and the session stays joined.
async fn begin(
    sig: &mut Option<SignalingClient>,
    running: &mut Option<Running>,
    stopping: &mut bool,
    options: &PresenterOptions,
    what: Presentation,
    quiet: bool,
    page_tx: &mpsc::UnboundedSender<DisplayMessage>,
) -> Result<ControlSender, TransportError> {
    let Some(s) = sig.take().filter(|_| running.is_none()) else {
        return Err(TransportError::Invalid(
            "a presentation is running already".into(),
        ));
    };
    match start(s, options, what).await {
        Ok(mut r) => {
            let control = r.control.clone();
            let tx = page_tx.clone();
            let reader = control.clone();
            r.pages = tokio::spawn(async move {
                while let Some(m) = reader.recv_message().await {
                    let _ = tx.send(m);
                }
            });
            r.quiet = quiet;
            *stopping = false;
            *running = Some(r);
            Ok(control)
        }
        Err(StartFailed { sig: s, error }) => {
            // The session stays joined; the caller decides what the error means.
            *sig = Some(s);
            Err(error)
        }
    }
}

async fn run(
    sig: SignalingClient,
    options: PresenterOptions,
    ice_state: IceState,
    mut cmds: mpsc::UnboundedReceiver<Cmd>,
    out: mpsc::UnboundedSender<PresenterEvent>,
    acks: watch::Sender<u64>,
) {
    let emit = |e: PresenterEvent| {
        let _ = out.send(e);
    };
    // Between presentations this task holds the signalling connection itself.
    let mut sig: Option<SignalingClient> = Some(sig);
    let mut running: Option<Running> = None;
    let (page_tx, mut page_rx) = mpsc::unbounded_channel::<DisplayMessage>();
    let mut stopping = false;
    loop {
        let next = tokio::select! {
            c = cmds.recv() => Next::Cmd(c),
            m = async {
                // While a presentation runs it holds the connection (`sig` is `None`).
                match sig.as_mut() {
                    Some(s) => s.recv().await,
                    None => std::future::pending().await,
                }
            } => Next::Idle(m),
            e = async {
                match running.as_mut() {
                    Some(r) => r.core.next_event().await,
                    None => std::future::pending().await,
                }
            } => Next::Running(e),
            p = page_rx.recv() => Next::Page(p),
        };
        match next {
            Next::Cmd(None | Some(Cmd::Leave)) => break,
            Next::Cmd(Some(Cmd::Present { what, reply })) => {
                let r = begin(
                    &mut sig,
                    &mut running,
                    &mut stopping,
                    &options,
                    what,
                    false,
                    &page_tx,
                )
                .await;
                let _ = reply.send(r);
            }
            Next::Cmd(Some(Cmd::Handover { reply })) => {
                let r = match running.as_ref() {
                    Some(r) => Ok(r.control.clone()),
                    None => {
                        begin(
                            &mut sig,
                            &mut running,
                            &mut stopping,
                            &options,
                            Presentation::Content,
                            true,
                            &page_tx,
                        )
                        .await
                    }
                };
                let _ = reply.send(r);
            }
            Next::Cmd(Some(Cmd::StopPresenting)) => {
                if let Some(r) = running.as_mut()
                    && !stopping
                {
                    stopping = true;
                    // The web display closes what it shows on `cmd stop`; the native one
                    // sees the channel end.
                    let _ = r.control.send_cmd(ControlCmd::Stop).await;
                    let _ = r.control.flush(STOP_FLUSH).await;
                    r.core.request_stop();
                }
            }
            Next::Page(Some(DisplayMessage::Page { page, pages })) => {
                emit(PresenterEvent::Page { page, pages });
            }
            Next::Cmd(Some(Cmd::Pause(paused))) => {
                if let Some(r) = running.as_ref()
                    && !stopping
                    && !r.quiet
                {
                    r.core.set_paused(paused).await;
                }
            }
            Next::Page(Some(DisplayMessage::PauseState { presenter, display })) => {
                emit(PresenterEvent::PauseState { presenter, display });
                // A display that does not hold what this side wants is asked again (it may
                // have ignored the command before its first picture).
                if let Some(r) = running.as_ref()
                    && let Core::Media(c) = &r.core
                {
                    c.sync_pause(presenter).await;
                }
            }
            Next::Page(Some(DisplayMessage::HandoverOk)) => {
                acks.send_modify(|n| *n += 1);
            }
            Next::Page(None) => {}
            Next::Running(Some(ev)) => match ev {
                SenderEvent::State(s) => {
                    let quiet = running.as_ref().is_some_and(|r| r.quiet);
                    if s == ConnectionState::Connected
                        && let Some(r) = running.as_mut()
                        && !r.presenting
                    {
                        r.presenting = true;
                        if !quiet {
                            emit(PresenterEvent::Presenting);
                        }
                    }
                    if !quiet {
                        emit(PresenterEvent::State(s));
                    }
                }
                SenderEvent::ConnectionCode(code) => {
                    if !running.as_ref().is_some_and(|r| r.quiet) {
                        emit(PresenterEvent::ConnectionCode(code));
                    }
                }
                SenderEvent::Streaming { frames_sent } => {
                    emit(PresenterEvent::Streaming { frames_sent });
                }
                // The end itself arrives with the join below.
                SenderEvent::Finished { .. }
                | SenderEvent::PeerLeft
                | SenderEvent::RemovedByHost
                | SenderEvent::Error(_) => {}
            },
            Next::Running(None) => {
                // The presentation's task ended: take the signalling connection back.
                let Some(r) = running.take() else { continue };
                r.pages.abort();
                let quiet = r.quiet;
                let (back, end) = r.core.join().await;
                stopping = false;
                match end {
                    RunEnd::Stopped { .. } => {
                        if !quiet {
                            emit(PresenterEvent::PresentingStopped);
                        }
                        match back {
                            Some(s) => sig = Some(s),
                            None => {
                                emit(error_event("unknown", "the sender task panicked"));
                                break;
                            }
                        }
                    }
                    RunEnd::PeerLeft => {
                        emit(PresenterEvent::Ended(EndReason::DisplayLeft));
                        break;
                    }
                    RunEnd::RemovedByHost => {
                        emit(PresenterEvent::Ended(EndReason::RemovedByHost));
                        break;
                    }
                    RunEnd::Failed { code, message } => {
                        emit(PresenterEvent::Error {
                            code: ice_state.refine(code),
                            message,
                        });
                        break;
                    }
                }
            }
            Next::Idle(msg) => match msg {
                None => {
                    emit(error_event(
                        "connection_lost",
                        "signalling connection closed",
                    ));
                    break;
                }
                Some(ServerMsg::PeerLeft) => {
                    emit(PresenterEvent::Ended(EndReason::DisplayLeft));
                    break;
                }
                Some(ServerMsg::Error { code, .. }) if code == "removed_by_host" => {
                    emit(PresenterEvent::Ended(EndReason::RemovedByHost));
                    break;
                }
                Some(ServerMsg::Error { code, message }) => {
                    emit(PresenterEvent::Error { code, message });
                    break;
                }
                // Candidates and answers of a finished presentation, and the like.
                Some(_) => {}
            },
        }
    }
    if let Some(mut r) = running.take() {
        r.pages.abort();
        r.core.request_stop();
        let _ = r.core.join().await;
    }
    // Dropping the signalling connection leaves the room.
    drop(sig);
}

fn error_event(code: &str, message: &str) -> PresenterEvent {
    PresenterEvent::Error {
        code: code.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_connection_after_a_bad_ice_list_gets_the_ice_code() {
        let refused = IceState::of(&TransportError::Server {
            code: "rate_limited".into(),
            message: String::new(),
        });
        assert_eq!(refused, IceState::Refused);
        assert_eq!(refused.refine("peer_failed".into()), "ice_refused");
        let failed = IceState::of(&TransportError::Protocol(
            "unexpected HTTP status 500".into(),
        ));
        assert_eq!(failed, IceState::Failed);
        assert_eq!(failed.refine("peer_failed".into()), "ice_failed");
        // Other failures are not about the list; a good list changes nothing.
        assert_eq!(failed.refine("connection_lost".into()), "connection_lost");
        assert_eq!(IceState::Fine.refine("peer_failed".into()), "peer_failed");
        // A full server is an unexpected status for the list, not a refusal (as in the web client).
        let full = IceState::of(&TransportError::Server {
            code: "server_full".into(),
            message: String::new(),
        });
        assert_eq!(full, IceState::Failed);
    }
}
