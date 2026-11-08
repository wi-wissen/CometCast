// SPDX-License-Identifier: GPL-3.0-or-later
//! Pause from both sides (docs/design/pause.md): native `PresenterSession` and `ReceiverCore`
//! against a real cc-server in this process and real UDP on 127.0.0.1 (test pattern video).
//!
//! * the presenter's pause stops the frames reaching the receiver and the display reports
//!   `PauseChanged`; the presenter hears `pause_state`;
//! * 30 s without frames produce no error event on either side;
//! * the display's own flag leaves the video running; resuming one side keeps the other frozen;
//! * the end of the presentation clears both flags.

use std::time::{Duration, Instant};

use cc_docs::Isolation;
use cc_proto::JoinMode;
use cc_receiver::{ContentEvents, ContentOptions, ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_sender::{Presentation, PresenterEvent, PresenterOptions, PresenterSession};
use cc_server::{ServerConfig, run};
use cc_transport::{ConnectionState, SessionConfig};
use tokio::net::TcpListener;

async fn start_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(run(
        listener,
        ServerConfig::default(),
        std::future::pending(),
    ));
    addr.to_string()
}

fn local_session() -> SessionConfig {
    SessionConfig {
        udp_bind: vec!["127.0.0.1:0".into()],
        ..SessionConfig::default()
    }
}

struct Rig {
    core: ReceiverCore,
    // Kept so that the content queue exists (no content is sent here).
    _content: ContentEvents,
    presenter: PresenterSession,
}

async fn rig() -> Rig {
    let server = start_server().await;
    let mut o = ReceiverOptions::new(&server);
    o.session = local_session();
    o.join_mode = JoinMode::Open;
    o.content = Some(ContentOptions {
        isolation: Isolation::InProcess,
        ..ContentOptions::default()
    });
    let mut core = ReceiverCore::start(o).await.expect("receiver start");
    let content = core.take_content_events().unwrap();
    let mut po = PresenterOptions::new(&server, core.room_code().clone());
    po.session = local_session();
    po.name = Some("Ada".into());
    let presenter = PresenterSession::join(po).await.expect("presenter joins");
    Rig {
        core,
        _content: content,
        presenter,
    }
}

impl Rig {
    async fn display_event<T>(&mut self, mut f: impl FnMut(&ReceiverEvent) -> Option<T>) -> T {
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let e = self.core.next_event().await.expect("receiver ended");
                if let ReceiverEvent::Error(m) = &e {
                    panic!("receiver error: {m}");
                }
                if let Some(t) = f(&e) {
                    return t;
                }
            }
        })
        .await
        .expect("timeout waiting for a display event")
    }

    async fn presenter_event<T>(&mut self, mut f: impl FnMut(&PresenterEvent) -> Option<T>) -> T {
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let e = self.presenter.next_event().await.expect("presenter ended");
                if let PresenterEvent::Error { code, message } = &e {
                    panic!("presenter error {code}: {message}");
                }
                if let Some(t) = f(&e) {
                    return t;
                }
            }
        })
        .await
        .expect("timeout waiting for a presenter event")
    }

    /// Wait for the display's `PauseChanged` with these flags.
    async fn display_flags(&mut self, presenter: bool, display: bool) {
        self.display_event(|e| {
            matches!(e, ReceiverEvent::PauseChanged { presenter: p, display: d }
                if *p == presenter && *d == display)
            .then_some(())
        })
        .await;
    }

    /// Wait for the presenter's `PauseState` with these flags.
    async fn presenter_flags(&mut self, presenter: bool, display: bool) {
        self.presenter_event(|e| {
            matches!(e, PresenterEvent::PauseState { presenter: p, display: d }
                if *p == presenter && *d == display)
            .then_some(())
        })
        .await;
    }

    /// Let the clock run for `d` and fail on any sign of trouble on either side. Returns the
    /// display events seen.
    async fn quiet_for(&mut self, d: Duration) -> Vec<ReceiverEvent> {
        let end = Instant::now() + d;
        let mut seen = Vec::new();
        while Instant::now() < end {
            tokio::select! {
                e = self.core.next_event() => match e.expect("receiver ended") {
                    ReceiverEvent::Error(m) => panic!("receiver error: {m}"),
                    ReceiverEvent::PresentationStopped => panic!("presentation stopped"),
                    ReceiverEvent::PeerLeft => panic!("peer left"),
                    ReceiverEvent::State(ConnectionState::Failed | ConnectionState::Closed) => {
                        panic!("connection lost")
                    }
                    ReceiverEvent::Stats(_) => {}
                    other => seen.push(other),
                },
                e = self.presenter.next_event() => match e.expect("presenter ended") {
                    PresenterEvent::Error { code, message } => {
                        panic!("presenter error {code}: {message}")
                    }
                    PresenterEvent::Ended(r) => panic!("presenter ended: {r:?}"),
                    PresenterEvent::PresentingStopped => panic!("presenting stopped"),
                    _ => {}
                },
                () = tokio::time::sleep_until(tokio::time::Instant::from_std(end)) => {}
            }
        }
        seen
    }

    async fn wait_frames(&mut self, more: u64) {
        let start = self.core.stats().frames;
        let end = Instant::now() + Duration::from_secs(20);
        while self.core.stats().frames < start + more {
            assert!(Instant::now() < end, "no frames arrive");
            self.quiet_for(Duration::from_millis(100)).await;
        }
    }

    async fn present_test_pattern(&mut self) {
        self.presenter
            .present(Presentation::Media {
                screen: None,
                external: None,
                duration: None,
            })
            .await
            .expect("present");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn presenter_pause_stops_the_frames_for_30_seconds_without_errors_and_resume_restarts_them() {
    let mut r = rig().await;
    r.present_test_pattern().await;
    // The presenter is told both flags are false once the channel is there.
    r.presenter_flags(false, false).await;
    r.wait_frames(10).await;

    r.presenter.pause();
    r.display_flags(true, false).await;
    r.presenter_flags(true, false).await;
    // Frames that were already on their way arrive; then it is quiet.
    r.quiet_for(Duration::from_millis(1500)).await;
    let frames = r.core.stats().frames;
    let packets = r.core.stats().packets;

    // 30 s without frames: no error, no stop, no connection loss on either side.
    let seen = r.quiet_for(Duration::from_secs(30)).await;
    assert!(
        seen.iter()
            .all(|e| !matches!(e, ReceiverEvent::PauseChanged { .. })),
        "the flags do not change by themselves: {seen:?}"
    );
    assert_eq!(r.core.stats().frames, frames, "no frames while paused");
    assert_eq!(r.core.stats().packets, packets, "no packets while paused");

    r.presenter.resume();
    r.display_flags(false, false).await;
    r.presenter_flags(false, false).await;
    r.wait_frames(10).await;
    r.presenter.leave().await;
    r.core.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn each_side_clears_only_its_own_flag_and_the_end_clears_both() {
    let mut r = rig().await;
    r.present_test_pattern().await;
    r.presenter_flags(false, false).await;
    r.wait_frames(5).await;

    // The display's own flag: the presenter goes on sending, and is told.
    r.core.set_display_paused(true);
    r.display_flags(false, true).await;
    r.presenter_flags(false, true).await;
    let f = r.core.stats().frames;
    r.wait_frames(5).await;
    assert!(r.core.stats().frames > f, "the presenter keeps sending");

    // Both flags: resuming one side keeps the picture frozen by the other.
    r.presenter.pause();
    r.display_flags(true, true).await;
    r.presenter_flags(true, true).await;
    r.presenter.resume();
    r.display_flags(false, true).await;
    r.presenter_flags(false, true).await;
    r.core.set_display_paused(false);
    r.display_flags(false, false).await;
    r.presenter_flags(false, false).await;

    // The end of the presentation clears both flags on the display.
    r.core.set_display_paused(true);
    r.display_flags(false, true).await;
    r.presenter.pause();
    r.display_flags(true, true).await;
    r.presenter.stop_presenting();
    r.display_flags(false, false).await;
    r.display_event(|e| matches!(e, ReceiverEvent::PresentationStopped).then_some(()))
        .await;
    // Without a screen the display's control is not offered: the call is ignored.
    r.core.set_display_paused(true);
    let mut seen = Vec::new();
    while let Ok(Some(e)) =
        tokio::time::timeout(Duration::from_millis(500), r.core.next_event()).await
    {
        seen.push(e);
    }
    assert!(
        seen.iter()
            .all(|e| !matches!(e, ReceiverEvent::PauseChanged { .. })),
        "{seen:?}"
    );
    r.presenter.leave().await;
    r.core.stop().await;
}
