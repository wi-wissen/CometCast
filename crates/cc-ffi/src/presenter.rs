// SPDX-License-Identifier: GPL-3.0-or-later
//! The presenter session of the Flutter app (native-apps.md 20.1): `start_presenter` joins the
//! room once, `present_*` and `stop_presenting` come and go inside it, `stop_presenter`
//! leaves. The work is done by `cc-sender`'s `PresenterSession`; this module turns its events
//! into [`PresenterUiEvent`], prepares content (the files are read here, never in Dart) and
//! sends it with progress.
//!
//! One task owns the session. Commands arrive through a process-wide channel (the Dart side
//! calls free functions, like `encoded.rs`); events leave through the stream sink.
//!
//! Errors that happen **while starting a presentation** (`capture_denied`,
//! `capture_unavailable`, `capture_os_old`, `capture_no_portal`) do not end the session: the stream stays open and the person is back
//! at "What to show". Every other `Error` and every `Ended` is final: the stream ends.

use std::sync::Mutex;

use cc_proto::{ControlCmd, RoomCode};
use cc_sender::content::READY_TIMEOUT;
use cc_sender::{
    ControlSender, ExternalInput, Presentation, PresenterEvent, PresenterOptions, PresenterSession,
    ScreenOptions,
};
use cc_transport::{JoinProgress, TransportError};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::api::{ContentSource, PresenterUiEvent, ScreenRequest, UiContentCmd, UiSendRejection};
use crate::content_sender::{self, Prepared};
use crate::frb_generated::StreamSink;
use crate::{content, encoded, office};

/// What the app asks of the running session.
pub(crate) enum PCmd {
    /// Share the screen: X11 capture with these settings (`None`: the system picker of the
    /// desktop, 20.3: portal on Linux, `GraphicsCapturePicker`, `SCContentSharingPicker`; a
    /// cancelled picker is `capture_denied`, a system without one `capture_unavailable`).
    Screen(Option<ScreenRequest>),
    /// Share H.264 that the host pushes with `push_encoded_frame` (Android, iOS).
    Encoded,
    /// Show a file or a link.
    Content(ContentSource),
    /// A page command for the shown PDF.
    Cmd(ControlCmd),
    /// Pause or resume the screen (docs/design/pause.md).
    Pause(bool),
    /// End the presentation, stay joined.
    StopPresenting,
    /// Ask the display to hold this hand-over token (iOS, native-apps.md 20.5).
    Handover(String),
    /// The code of a TV that asked for pairing (`tv_pair`); the answer says whether it worked.
    /// Only TV sessions (`tv.rs`) use it.
    Pair {
        /// The code the TV shows.
        pin: String,
        /// Whether the pairing worked.
        reply: oneshot::Sender<bool>,
    },
}

static CMDS: Mutex<Option<mpsc::UnboundedSender<PCmd>>> = Mutex::new(None);

fn cmds() -> std::sync::MutexGuard<'static, Option<mpsc::UnboundedSender<PCmd>>> {
    CMDS.lock().unwrap_or_else(|p| p.into_inner())
}

/// Queue a command; `false` when no presenter session runs.
pub(crate) fn send(cmd: PCmd) -> bool {
    cmds().as_ref().is_some_and(|tx| tx.send(cmd).is_ok())
}

/// Make `tx` the command channel of the session that starts now.
pub(crate) fn install(tx: mpsc::UnboundedSender<PCmd>) {
    *cmds() = Some(tx);
}

/// Whether a presenter session runs (the page commands go there then).
pub(crate) fn running() -> bool {
    cmds().as_ref().is_some_and(|tx| !tx.is_closed())
}

/// Forget the command channel (the session ended or was stopped).
pub(crate) fn clear() {
    *cmds() = None;
}

/// A page command of the presenter's view; `None` for a page outside the protocol's range.
pub(crate) fn cmd_from_ui(cmd: UiContentCmd) -> Option<PCmd> {
    use cc_files::wire::MAX_PAGE_NUMBER;
    Some(match cmd {
        UiContentCmd::NextPage => PCmd::Cmd(ControlCmd::NextPage),
        UiContentCmd::PrevPage => PCmd::Cmd(ControlCmd::PrevPage),
        UiContentCmd::GoToPage { page } if (1..=MAX_PAGE_NUMBER).contains(&page) => {
            PCmd::Cmd(ControlCmd::GoToPage { page })
        }
        UiContentCmd::GoToPage { .. } => return None,
        // "Stop showing" ends the presentation, not the session.
        UiContentCmd::Stop => PCmd::StopPresenting,
    })
}

/// The settings of a join, from the arguments of `start_presenter`.
pub(crate) struct Join {
    pub(crate) code: RoomCode,
    pub(crate) server: String,
    pub(crate) name: Option<String>,
    pub(crate) handover: Option<String>,
    pub(crate) udp_bind: Option<String>,
}

/// Join the room (cancellable through `stop`). `None` = stopped meanwhile or the app stopped
/// listening; errors are reported to the app here.
pub(crate) async fn join(
    j: Join,
    events: &StreamSink<PresenterUiEvent>,
    stop: &mut oneshot::Receiver<()>,
) -> Option<PresenterSession> {
    let mut options = PresenterOptions::new(j.server, j.code);
    options.device_key = crate::device::presenter_key();
    if let Some(b) = j.udp_bind {
        options.session.udp_bind = vec![b];
    }
    options.name = j.name;
    options.handover = j
        .handover
        .filter(|h| cc_proto::sanitize_handover(Some(h)).is_some());
    let (ptx, mut prx) = mpsc::unbounded_channel();
    options.join_progress = Some(ptx);
    let notice_sink = events.clone();
    tokio::spawn(async move {
        while let Some(JoinProgress::Pending {
            timeout_secs,
            check,
            room_name,
        }) = prx.recv().await
        {
            let _ = notice_sink.add(PresenterUiEvent::WaitingForApproval {
                timeout_secs: u32::try_from(timeout_secs).unwrap_or(u32::MAX),
                check,
                room_name,
            });
        }
    });
    // Dropping the join while it waits withdraws the request on the server.
    let joined = tokio::select! {
        r = PresenterSession::join(options) => r,
        _ = &mut *stop => return None,
    };
    match joined {
        Ok(s) => Some(s),
        Err(e) => {
            let _ = events.add(PresenterUiEvent::Error {
                code: e.code(),
                message: e.to_string(),
            });
            None
        }
    }
}

/// Messages of the jobs to the session task.
enum Job {
    /// The content was read and checked (or refused).
    Prepared(Result<Prepared, UiSendRejection>),
}

/// The code of a presentation that could not start. Only the screen can refuse: the capture
/// was denied or is not available here.
fn start_failure(e: &TransportError) -> (String, String) {
    let code = match e {
        TransportError::Invalid(_) => "capture_unavailable".into(),
        other => other.code(),
    };
    (code, e.to_string())
}

/// The life of the session after the join: commands in, events out, until the person stops it,
/// the host removes this sender, the display leaves or something fails.
pub(crate) async fn drive(
    mut session: PresenterSession,
    events: StreamSink<PresenterUiEvent>,
    mut stop: oneshot::Receiver<()>,
    mut cmd_rx: mpsc::UnboundedReceiver<PCmd>,
) {
    let (job_tx, mut job_rx) = mpsc::unbounded_channel::<Job>();
    let mut control: Option<ControlSender> = None;
    let mut send_job: Option<JoinHandle<()>> = None;
    let mut preparing: Option<JoinHandle<()>> = None;
    let mut pushed_input: Option<ExternalInput> = None;
    let abort_jobs = |send_job: &mut Option<JoinHandle<()>>,
                      preparing: &mut Option<JoinHandle<()>>| {
        for j in [send_job.take(), preparing.take()].into_iter().flatten() {
            j.abort();
        }
    };
    loop {
        tokio::select! {
            biased;
            _ = &mut stop => break,
            cmd = cmd_rx.recv() => {
                let Some(cmd) = cmd else { break };
                match cmd {
                    PCmd::Screen(request) => {
                        abort_jobs(&mut send_job, &mut preparing);
                        // `Some`: the X11 whole-desktop settings of the request. `None`: the
                        // system picker (native-apps.md 20.3): cc-capture picks the portal, the
                        // Windows picker or the macOS picker; the defaults are 15 fps, 2 Mbit/s.
                        let screen = match request {
                            Some(r) => ScreenOptions {
                                display: r.display,
                                source_id: r.monitor_id,
                                fps: r.fps.clamp(1, 60),
                                bitrate_bps: r.bitrate_kbps.clamp(100, 50_000).saturating_mul(1000),
                                ..ScreenOptions::default()
                            },
                            None => ScreenOptions {
                                picker: true,
                                ..ScreenOptions::default()
                            },
                        };
                        match session
                            .present(Presentation::Media {
                                screen: Some(screen),
                                external: None,
                                duration: None,
                            })
                            .await
                        {
                            Ok(c) => control = Some(c),
                            Err(e) => {
                                let (code, message) = start_failure(&e);
                                let _ = events.add(PresenterUiEvent::Error { code, message });
                            }
                        }
                    }
                    PCmd::Encoded => {
                        abort_jobs(&mut send_job, &mut preparing);
                        let input = ExternalInput::default();
                        encoded::replace(Some(input.clone()));
                        pushed_input = Some(input.clone());
                        match session
                            .present(Presentation::Media {
                                screen: None,
                                external: Some(input),
                                duration: None,
                            })
                            .await
                        {
                            Ok(c) => control = Some(c),
                            Err(e) => {
                                let (code, message) = start_failure(&e);
                                let _ = events.add(PresenterUiEvent::Error { code, message });
                                encoded::clear_if(pushed_input.as_ref());
                            }
                        }
                    }
                    PCmd::Content(source) => {
                        abort_jobs(&mut send_job, &mut preparing);
                        let tx = job_tx.clone();
                        let sink = events.clone();
                        preparing = Some(tokio::spawn(async move {
                            let _ = tx.send(Job::Prepared(prepare(source, &sink).await));
                        }));
                    }
                    PCmd::Cmd(c) => {
                        if let Some(ctl) = control.clone() {
                            tokio::spawn(async move {
                                let _ = ctl.send_cmd(c).await;
                            });
                        }
                    }
                    PCmd::Pause(paused) => {
                        // Only a media presentation has a screen; others ignore it.
                        if paused {
                            session.pause();
                        } else {
                            session.resume();
                        }
                    }
                    PCmd::StopPresenting => {
                        abort_jobs(&mut send_job, &mut preparing);
                        if control.is_some() {
                            session.stop_presenting();
                        }
                    }
                    PCmd::Pair { reply, .. } => {
                        // Not a TV session: nothing to pair.
                        let _ = reply.send(false);
                    }
                    PCmd::Handover(token) => {
                        abort_jobs(&mut send_job, &mut preparing);
                        let ok = tokio::select! {
                            r = session.request_handover(&token) => r.is_ok(),
                            _ = &mut stop => break,
                        };
                        let _ = events.add(PresenterUiEvent::HandoverResult { ok });
                    }
                }
            }
            job = job_rx.recv() => {
                let Some(Job::Prepared(result)) = job else { continue };
                preparing = None;
                let prepared = match result {
                    Ok(p) => p,
                    Err(reason) => {
                        let _ = events.add(PresenterUiEvent::ContentRejected { reason });
                        continue;
                    }
                };
                let _ = events.add(PresenterUiEvent::ContentPrepared {
                    name: prepared.name.clone(),
                    kind: prepared.kind,
                    size: prepared.size,
                    pages: prepared.pages,
                    host: prepared.host.clone(),
                });
                match session.present(Presentation::Content).await {
                    Ok(ctl) => {
                        control = Some(ctl.clone());
                        let sink = events.clone();
                        send_job = Some(tokio::spawn(send_content(ctl, prepared, sink)));
                    }
                    Err(e) => {
                        let (code, message) = start_failure(&e);
                        let _ = events.add(PresenterUiEvent::Error { code, message });
                    }
                }
            }
            ev = session.next_event() => {
                let Some(ev) = ev else { break };
                let (ui, last) = match ev {
                    PresenterEvent::Presenting => (Some(PresenterUiEvent::Presenting), false),
                    PresenterEvent::PresentingStopped => {
                        abort_jobs(&mut send_job, &mut preparing);
                        control = None;
                        encoded::clear_if(pushed_input.take().as_ref());
                        (Some(PresenterUiEvent::PresentingStopped), false)
                    }
                    PresenterEvent::ConnectionCode(sas) => {
                        (Some(PresenterUiEvent::ConnectionCode { sas }), false)
                    }
                    PresenterEvent::Page { page, pages } => {
                        (Some(PresenterUiEvent::Page { page, pages }), false)
                    }
                    PresenterEvent::PauseState { presenter, display } => (
                        Some(PresenterUiEvent::PauseState { presenter, display }),
                        false,
                    ),
                    PresenterEvent::Ended(reason) => (
                        Some(PresenterUiEvent::Ended { code: reason.code().into() }),
                        true,
                    ),
                    PresenterEvent::Error { code, message } => {
                        (Some(PresenterUiEvent::Error { code, message }), true)
                    }
                    PresenterEvent::State(_) | PresenterEvent::Streaming { .. } => (None, false),
                };
                if let Some(ui) = ui
                    && events.add(ui).is_err()
                {
                    break;
                }
                if last {
                    break;
                }
            }
        }
    }
    abort_jobs(&mut send_job, &mut preparing);
    clear();
    encoded::clear_if(pushed_input.as_ref());
    session.leave().await;
}

/// Read and check the content, with the notices of an Office conversion. Blocking work runs
/// on the blocking pool.
async fn prepare(
    source: ContentSource,
    events: &StreamSink<PresenterUiEvent>,
) -> Result<Prepared, UiSendRejection> {
    let worker = content::worker();
    let mut permit = None;
    if let ContentSource::File { path } = &source {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if office::is_office_name(&name) {
            let _ = events.add(PresenterUiEvent::Converting { name });
            // One conversion at a time.
            permit = Some(office::GATE.acquire().await);
        }
    }
    let office_options = office::options();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        content_sender::prepare(&source, &worker, &office_options)
    })
    .await
    .unwrap_or(Err(UiSendRejection::Unreadable))
}

/// Wait for the channel, send, report progress and `ContentSent`.
async fn send_content(
    ctl: ControlSender,
    prepared: Prepared,
    events: StreamSink<PresenterUiEvent>,
) {
    if let Err(e) = ctl.wait_ready(READY_TIMEOUT).await {
        let _ = events.add(PresenterUiEvent::Error {
            code: e.code(),
            message: e.to_string(),
        });
        return;
    }
    let report = |sent, total| {
        events
            .add(PresenterUiEvent::Progress { sent, total })
            .is_ok()
    };
    match content_sender::send_payload(&ctl, &prepared, &report).await {
        Ok(()) => {
            let _ = events.add(PresenterUiEvent::ContentSent);
        }
        Err(message) => {
            let _ = events.add(PresenterUiEvent::Error {
                code: "peer_failed".into(),
                message,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_commands_are_checked_like_the_protocol() {
        assert!(matches!(
            cmd_from_ui(UiContentCmd::NextPage),
            Some(PCmd::Cmd(ControlCmd::NextPage))
        ));
        assert!(matches!(
            cmd_from_ui(UiContentCmd::GoToPage { page: 3 }),
            Some(PCmd::Cmd(ControlCmd::GoToPage { page: 3 }))
        ));
        assert!(cmd_from_ui(UiContentCmd::GoToPage { page: 0 }).is_none());
        assert!(cmd_from_ui(UiContentCmd::GoToPage { page: 1_000_001 }).is_none());
        // "Stop showing" ends the presentation, not the session.
        assert!(matches!(
            cmd_from_ui(UiContentCmd::Stop),
            Some(PCmd::StopPresenting)
        ));
    }

    #[test]
    fn a_failed_start_is_a_capture_problem_only_for_invalid_input() {
        let (code, _) = start_failure(&TransportError::Invalid("no display".into()));
        assert_eq!(code, "capture_unavailable");
        let (code, _) = start_failure(&TransportError::Closed);
        assert_eq!(code, "connection_lost");
    }

    #[test]
    fn a_cancelled_picker_is_capture_denied() {
        // cc-sender reports a cancelled system picker as a server-style error with this code.
        let (code, _) = start_failure(&TransportError::Server {
            code: "capture_denied".into(),
            message: "screen capture was cancelled or refused".into(),
        });
        assert_eq!(code, "capture_denied");
    }

    #[test]
    fn the_new_desktop_codes_pass_through_and_do_not_end_the_session() {
        // Decision N56: macOS before 14 and Wayland without the portal or PipeWire.
        for code in ["capture_os_old", "capture_no_portal"] {
            let (c, _) = start_failure(&TransportError::Server {
                code: code.into(),
                message: "x".into(),
            });
            assert_eq!(c, code);
        }
    }

    #[test]
    fn commands_need_a_running_session() {
        clear();
        assert!(!running());
        assert!(!send(PCmd::StopPresenting));
        let (tx, mut rx) = mpsc::unbounded_channel();
        install(tx);
        assert!(running());
        assert!(send(PCmd::Encoded));
        assert!(matches!(rx.try_recv(), Ok(PCmd::Encoded)));
        // Pause and resume (docs/design/pause.md) go through the same channel.
        assert!(crate::api::set_presenter_paused(true));
        assert!(matches!(rx.try_recv(), Ok(PCmd::Pause(true))));
        assert!(crate::api::set_presenter_paused(false));
        assert!(matches!(rx.try_recv(), Ok(PCmd::Pause(false))));
        clear();
        assert!(!send(PCmd::Encoded));
        assert!(!crate::api::set_presenter_paused(true));
    }
}
