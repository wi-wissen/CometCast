// SPDX-License-Identifier: GPL-3.0-or-later
//! The room outlives a presenter, and a presenter joins once and presents several times
//! (native-apps.md 20.1 and 20.2): native receiver and `PresenterSession` against a real
//! cc-server in this process and real UDP on 127.0.0.1. Also the connection check code on both
//! sides, the `page` message of the display and the receive progress.
//!
//! PDFs render in process here: the files are the repository's own test files; the sandboxed
//! path is covered by `content.rs`.

use std::time::Duration;

use cc_docs::Isolation;
use cc_proto::{ControlCmd, JoinMode};
use cc_receiver::{
    ContentEvent, ContentEvents, ContentOptions, ImageMime, ReceiverCore, ReceiverEvent,
    ReceiverOptions,
};
use cc_sender::{EndReason, Presentation, PresenterEvent, PresenterOptions, PresenterSession};
use cc_server::{ServerConfig, run};
use cc_transport::SessionConfig;
use tokio::net::TcpListener;

const SAMPLE_PDF: &[u8] = include_bytes!("../../cc-docs/tests/data/slides.pdf");

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

struct Display {
    core: ReceiverCore,
    content: ContentEvents,
    events: Vec<ReceiverEvent>,
}

impl Display {
    async fn start(server: &str, mode: JoinMode, name: Option<&str>) -> Self {
        let mut o = ReceiverOptions::new(server);
        o.session = local_session();
        o.join_mode = mode;
        o.room_name = name.map(str::to_owned);
        o.content = Some(ContentOptions {
            isolation: Isolation::InProcess,
            target_width: 300,
            ..ContentOptions::default()
        });
        let mut core = ReceiverCore::start(o).await.expect("receiver start");
        let content = core.take_content_events().unwrap();
        Self {
            core,
            content,
            events: Vec::new(),
        }
    }

    fn code(&self) -> cc_proto::RoomCode {
        self.core.room_code().clone()
    }

    /// Wait for a receiver event that `f` accepts (events before it are kept in `events`).
    async fn event<T>(&mut self, mut f: impl FnMut(&ReceiverEvent) -> Option<T>) -> T {
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let e = self
                    .core
                    .next_event()
                    .await
                    .expect("receiver event stream ended");
                if let ReceiverEvent::Error(m) = &e {
                    panic!("receiver error: {m}");
                }
                if let Some(t) = f(&e) {
                    return t;
                }
                self.events.push(e);
            }
        })
        .await
        .expect("timeout waiting for a receiver event")
    }

    /// The next content event that is not a transfer notice.
    async fn content(&mut self) -> ContentEvent {
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                tokio::select! {
                    e = self.content.next() => {
                        let e = e.expect("content stream ended");
                        if !matches!(
                            e,
                            ContentEvent::TransferStarted { .. }
                                | ContentEvent::TransferProgress { .. }
                        ) {
                            return e;
                        }
                    }
                    e = self.core.next_event() => match e {
                        Some(ReceiverEvent::Error(m)) => panic!("receiver error: {m}"),
                        Some(other) => self.events.push(other),
                        None => panic!("receiver event stream ended"),
                    },
                }
            }
        })
        .await
        .expect("timeout waiting for a content event")
    }
}

async fn join(server: &str, display: &Display, name: &str) -> PresenterSession {
    let mut o = PresenterOptions::new(server, display.code());
    o.session = local_session();
    o.name = Some(name.into());
    PresenterSession::join(o).await.expect("presenter joins")
}

async fn presenter_event<T>(
    p: &mut PresenterSession,
    mut f: impl FnMut(&PresenterEvent) -> Option<T>,
) -> T {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let e = p.next_event().await.expect("presenter event stream ended");
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

fn tiny_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, 0x49, 0x48, 0x44, 0x52, 0, 0,
        0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89, 0, 0, 0, 13, 0x49, 0x44, 0x41,
        0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0, 0x1f, 0, 5, 0, 1, 0xff, 0x89, 0x99, 0x3d,
        0x1d, 0, 0, 0, 0, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_room_and_its_code_outlive_a_presenter() {
    let server = start_server().await;
    let mut d = Display::start(&server, JoinMode::Open, Some("Meeting room Elbe")).await;
    let code = d.code();

    // First presenter shows an image and leaves.
    let p1 = join(&server, &d, "Ada").await;
    assert_eq!(p1.room_name(), Some("Meeting room Elbe"));
    let ctl = p1.present(Presentation::Content).await.unwrap();
    ctl.wait_ready(Duration::from_secs(20)).await.unwrap();
    ctl.send_image("a.png", ImageMime::Png, &tiny_png())
        .await
        .unwrap();
    assert!(matches!(
        d.content().await,
        ContentEvent::ImageReceived { .. }
    ));
    assert!(ctl.finish().await);
    p1.leave().await;
    d.event(|e| matches!(e, ReceiverEvent::PeerLeft).then_some(()))
        .await;

    // The room is still there with the same code: a second presenter joins and shows a link.
    assert_eq!(d.code(), code);
    let mut p2 = join(&server, &d, "Bo").await;
    let ctl = p2.present(Presentation::Content).await.unwrap();
    ctl.wait_ready(Duration::from_secs(20)).await.unwrap();
    ctl.send_link("https://example.org/b").await.unwrap();
    assert_eq!(
        d.content().await,
        ContentEvent::LinkRequested {
            url: "https://example.org/b".into()
        }
    );
    assert!(ctl.finish().await);

    // Removing the presenter keeps the room as well; the presenter is told.
    d.core.remove_peer(false);
    d.event(|e| matches!(e, ReceiverEvent::PeerLeft).then_some(()))
        .await;
    presenter_event(&mut p2, |e| {
        matches!(e, PresenterEvent::Ended(EndReason::RemovedByHost)).then_some(())
    })
    .await;
    assert_eq!(d.code(), code);
    let mut p3 = join(&server, &d, "Cy").await;
    let ctl = p3.present(Presentation::Content).await.unwrap();
    ctl.wait_ready(Duration::from_secs(20)).await.unwrap();
    ctl.send_image("c.png", ImageMime::Png, &tiny_png())
        .await
        .unwrap();
    assert!(matches!(
        d.content().await,
        ContentEvent::ImageReceived { .. }
    ));
    assert!(ctl.finish().await);

    // Ending the room is what makes a new code: the core stops.
    d.core.end_room();
    d.event(|e| matches!(e, ReceiverEvent::RoomEnded).then_some(()))
        .await;
    presenter_event(&mut p3, |e| {
        matches!(e, PresenterEvent::Ended(_)).then_some(())
    })
    .await;
    d.core.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stopping_keeps_the_join_and_needs_no_second_approval() {
    let server = start_server().await;
    let mut d = Display::start(&server, JoinMode::Confirm, None).await;

    let mut ph = {
        let mut o = PresenterOptions::new(&server, d.code());
        o.session = local_session();
        o.name = Some("Ada".into());
        tokio::spawn(async move { PresenterSession::join(o).await })
    };
    // The host approves once.
    let id = d
        .event(|e| match e {
            ReceiverEvent::JoinRequest { id, .. } => Some(*id),
            _ => None,
        })
        .await;
    d.core.decide_join(id, true);
    let mut p = (&mut ph).await.unwrap().expect("joined after approval");
    d.event(|e| matches!(e, ReceiverEvent::PeerJoined { .. }).then_some(()))
        .await;

    for round in 0..2 {
        let ctl = p.present(Presentation::Content).await.unwrap();
        ctl.wait_ready(Duration::from_secs(20)).await.unwrap();
        presenter_event(&mut p, |e| {
            matches!(e, PresenterEvent::Presenting).then_some(())
        })
        .await;
        ctl.send_link(&format!("https://example.org/{round}"))
            .await
            .unwrap();
        // (The `cmd stop` of the previous round may still be queued: it closes what was shown.)
        loop {
            match d.content().await {
                ContentEvent::LinkRequested { .. } => break,
                ContentEvent::ContentStopped => assert!(round > 0),
                other => panic!("{other:?}"),
            }
        }
        p.stop_presenting();
        presenter_event(&mut p, |e| {
            matches!(e, PresenterEvent::PresentingStopped).then_some(())
        })
        .await;
        // The display goes Idle: no leave, no new request.
        d.event(|e| matches!(e, ReceiverEvent::PresentationStopped).then_some(()))
            .await;
    }
    assert!(
        !d.events.iter().any(|e| matches!(
            e,
            ReceiverEvent::PeerLeft | ReceiverEvent::JoinRequest { .. }
        )),
        "no leave and no second request: {:?}",
        d.events
    );

    // Disconnect leaves.
    p.leave().await;
    d.event(|e| matches!(e, ReceiverEvent::PeerLeft).then_some(()))
        .await;
    d.core.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn both_sides_show_the_same_connection_code() {
    let server = start_server().await;
    let mut d = Display::start(&server, JoinMode::Open, None).await;
    let mut p = join(&server, &d, "Ada").await;
    let ctl = p.present(Presentation::Content).await.unwrap();
    ctl.wait_ready(Duration::from_secs(20)).await.unwrap();
    let mine = presenter_event(&mut p, |e| match e {
        PresenterEvent::ConnectionCode(c) => Some(c.clone()),
        _ => None,
    })
    .await;
    let theirs = d
        .event(|e| match e {
            ReceiverEvent::ConnectionCode(c) => Some(c.clone()),
            _ => None,
        })
        .await;
    assert_eq!(mine, theirs);
    assert_eq!(mine.len(), 7, "{mine}");
    assert_eq!(mine.as_bytes()[3], b' ');
    assert!(mine.chars().filter(char::is_ascii_digit).count() == 6);

    // The next presentation has its own certificates, so its own code, again on both sides.
    p.stop_presenting();
    d.event(|e| matches!(e, ReceiverEvent::PresentationStopped).then_some(()))
        .await;
    let ctl = p.present(Presentation::Content).await.unwrap();
    ctl.wait_ready(Duration::from_secs(20)).await.unwrap();
    let mine2 = presenter_event(&mut p, |e| match e {
        PresenterEvent::ConnectionCode(c) => Some(c.clone()),
        _ => None,
    })
    .await;
    let theirs2 = d
        .event(|e| match e {
            ReceiverEvent::ConnectionCode(c) => Some(c.clone()),
            _ => None,
        })
        .await;
    assert_eq!(mine2, theirs2);
    p.leave().await;
    d.core.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_display_reports_the_page_and_the_progress_counts_up() {
    let server = start_server().await;
    let mut d = Display::start(&server, JoinMode::Open, None).await;
    let mut p = join(&server, &d, "Ada").await;
    let ctl = p.present(Presentation::Content).await.unwrap();
    ctl.wait_ready(Duration::from_secs(20)).await.unwrap();

    ctl.send_document("slides.pdf", SAMPLE_PDF).await.unwrap();
    let pages = match d.content().await {
        ContentEvent::DocumentOpened { pages, .. } => pages,
        other => panic!("{other:?}"),
    };
    assert!(pages >= 3, "the sample has several pages");
    let first = presenter_event(&mut p, |e| match e {
        PresenterEvent::Page { page, pages } => Some((*page, *pages)),
        _ => None,
    })
    .await;
    assert_eq!(first, (1, pages));
    ctl.send_cmd(ControlCmd::NextPage).await.unwrap();
    let second = presenter_event(&mut p, |e| match e {
        PresenterEvent::Page { page, pages } => Some((*page, *pages)),
        _ => None,
    })
    .await;
    assert_eq!(second, (2, pages));
    ctl.send_cmd(ControlCmd::GoToPage { page: 3 })
        .await
        .unwrap();
    let third = presenter_event(&mut p, |e| match e {
        PresenterEvent::Page { page, pages } => Some((*page, *pages)),
        _ => None,
    })
    .await;
    assert_eq!(third, (3, pages));

    // A large file: progress between the start and the file, non-decreasing, ending at the total.
    let big: Vec<u8> = (0..4 * 1024 * 1024u32)
        .map(|i| (i.wrapping_mul(2654435761) >> 24) as u8)
        .collect();
    let sender = ctl.clone();
    let send = tokio::spawn(async move {
        sender
            .send_image("big.png", ImageMime::Png, &big)
            .await
            .unwrap();
    });
    let mut started = false;
    let mut seen: Vec<(u64, u64)> = Vec::new();
    loop {
        let e = tokio::time::timeout(Duration::from_secs(30), d.content.next())
            .await
            .expect("timeout")
            .expect("content stream ended");
        match e {
            ContentEvent::TransferStarted { size, .. } => {
                assert_eq!(size, 4 * 1024 * 1024);
                started = true;
            }
            ContentEvent::TransferProgress { received, total } => {
                assert!(started, "progress only after the start");
                assert_eq!(total, 4 * 1024 * 1024);
                if let Some((last, _)) = seen.last() {
                    assert!(received >= *last, "{seen:?}");
                }
                seen.push((received, total));
            }
            ContentEvent::ImageReceived { .. } => break,
            // The page change of the PDF may still be queued.
            _ => {}
        }
    }
    send.await.unwrap();
    assert!(!seen.is_empty(), "at least one progress event");
    assert_eq!(seen.last().unwrap().0, 4 * 1024 * 1024, "{seen:?}");
    // At most one event per percent and four per second.
    assert!(seen.len() <= 101, "{}", seen.len());

    p.leave().await;
    d.core.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn join_errors_carry_the_protocol_code() {
    let server = start_server().await;
    let d = Display::start(&server, JoinMode::Locked, None).await;
    let o = PresenterOptions::new(&server, d.code());
    let err = PresenterSession::join(o).await.err().expect("locked room");
    match err {
        cc_transport::TransportError::Server { code, .. } => assert_eq!(code, "room_locked"),
        other => panic!("{other:?}"),
    }
    d.core.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_video_presentation_stops_and_a_file_follows_in_the_same_session() {
    let server = start_server().await;
    let mut d = Display::start(&server, JoinMode::Open, None).await;
    let mut p = join(&server, &d, "Ada").await;

    // The built-in test stream stands in for the screen: video and control channel together.
    let ctl = p
        .present(Presentation::Media {
            screen: None,
            external: None,
            duration: None,
        })
        .await
        .unwrap();
    ctl.wait_ready(Duration::from_secs(20)).await.unwrap();
    let code = d
        .event(|e| match e {
            ReceiverEvent::ConnectionCode(c) => Some(c.clone()),
            _ => None,
        })
        .await;
    d.event(|e| matches!(e, ReceiverEvent::VideoTrack { .. }).then_some(()))
        .await;
    let mine = presenter_event(&mut p, |e| match e {
        PresenterEvent::ConnectionCode(c) => Some(c.clone()),
        _ => None,
    })
    .await;
    assert_eq!(code, mine);

    p.stop_presenting();
    presenter_event(&mut p, |e| {
        matches!(e, PresenterEvent::PresentingStopped).then_some(())
    })
    .await;
    d.event(|e| matches!(e, ReceiverEvent::PresentationStopped).then_some(()))
        .await;

    // A second presentation in the same session: a file, on a fresh connection.
    let ctl = p.present(Presentation::Content).await.unwrap();
    ctl.wait_ready(Duration::from_secs(20)).await.unwrap();
    ctl.send_image("a.png", ImageMime::Png, &tiny_png())
        .await
        .unwrap();
    loop {
        match d.content().await {
            ContentEvent::ImageReceived { .. } => break,
            ContentEvent::ContentStopped => {}
            other => panic!("{other:?}"),
        }
    }
    assert!(
        !d.events
            .iter()
            .any(|e| matches!(e, ReceiverEvent::PeerLeft)),
        "{:?}",
        d.events
    );
    p.leave().await;
    d.core.stop().await;
}
