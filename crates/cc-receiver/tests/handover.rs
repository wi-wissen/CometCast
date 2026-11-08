// SPDX-License-Identifier: GPL-3.0-or-later
//! The iOS hand-over token (native-apps.md 20.5, decision N32): native receiver, real
//! `PresenterSession`s and a real cc-server on 127.0.0.1.
//!
//! The host approves a person once. The presenter hands a single-use token to the display on
//! the content control channel, leaves, and the next join with the token and the same name is
//! let in without a dialog. A replay, a token of another room, another name and an expired
//! token all end in an ordinary request for the host.

use std::time::Duration;

use cc_docs::Isolation;
use cc_proto::JoinMode;
use cc_receiver::{ContentOptions, ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_sender::{PresenterOptions, PresenterSession};
use cc_server::{ServerConfig, run};
use cc_transport::SessionConfig;
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

async fn display(server: &str, mode: JoinMode, ttl: Duration) -> ReceiverCore {
    let mut o = ReceiverOptions::new(server);
    o.session = local_session();
    o.join_mode = mode;
    o.handover_ttl = ttl;
    o.content = Some(ContentOptions {
        isolation: Isolation::InProcess,
        ..ContentOptions::default()
    });
    let mut core = ReceiverCore::start(o).await.expect("receiver start");
    // The content stream must be read, or the sender is slowed down; nothing arrives here.
    let mut content = core.take_content_events().unwrap();
    tokio::spawn(async move { while content.next().await.is_some() {} });
    core
}

async fn event<T>(d: &mut ReceiverCore, mut f: impl FnMut(&ReceiverEvent) -> Option<T>) -> T {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let e = d.next_event().await.expect("receiver event stream ended");
            if let ReceiverEvent::Error(m) = &e {
                panic!("receiver error: {m}");
            }
            if let Some(t) = f(&e) {
                return t;
            }
        }
    })
    .await
    .expect("timeout waiting for a receiver event")
}

fn opts(server: &str, d: &ReceiverCore, name: &str, handover: Option<&str>) -> PresenterOptions {
    let mut o = PresenterOptions::new(server, d.room_code().clone());
    o.session = local_session();
    o.name = Some(name.into());
    o.handover = handover.map(str::to_owned);
    o
}

/// The first join of a person: the host approves once. Returns the joined session.
async fn first_join(server: &str, d: &mut ReceiverCore, name: &str) -> PresenterSession {
    let o = opts(server, d, name, None);
    let joining = tokio::spawn(PresenterSession::join(o));
    let id = event(d, |e| match e {
        ReceiverEvent::JoinRequest { id, .. } => Some(*id),
        _ => None,
    })
    .await;
    d.decide_join(id, true);
    let p = joining.await.unwrap().expect("joined after approval");
    event(d, |e| {
        matches!(
            e,
            ReceiverEvent::PeerJoined {
                handover: false,
                ..
            }
        )
        .then_some(())
    })
    .await;
    p
}

const TOKEN: &str = "q3Zx9_-Lm0AbCdEfGhIjKl";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_handover_token_admits_the_next_join_once_and_without_a_dialog() {
    let server = start_server().await;
    let mut d = display(&server, JoinMode::Confirm, Duration::from_secs(60)).await;
    let p = first_join(&server, &mut d, "Ada").await;

    p.request_handover(TOKEN).await.expect("handover_ok");
    event(&mut d, |e| {
        matches!(e, ReceiverEvent::HandoverStored { name } if name == "Ada").then_some(())
    })
    .await;
    p.leave().await;
    event(&mut d, |e| {
        matches!(e, ReceiverEvent::PeerLeft).then_some(())
    })
    .await;

    // The extension joins with the name and the token: in at once, no request event.
    let ext = tokio::spawn(PresenterSession::join(opts(
        &server,
        &d,
        "Ada",
        Some(TOKEN),
    )));
    let mut saw_request = false;
    event(&mut d, |e| {
        saw_request |= matches!(e, ReceiverEvent::JoinRequest { .. });
        matches!(e, ReceiverEvent::PeerJoined { handover: true, .. }).then_some(())
    })
    .await;
    assert!(!saw_request, "no dialog for a hand-over join");
    let ext = ext.await.unwrap().expect("admitted");
    ext.leave().await;
    event(&mut d, |e| {
        matches!(e, ReceiverEvent::PeerLeft).then_some(())
    })
    .await;

    // Replay: the same token again is an ordinary request.
    let again = tokio::spawn(PresenterSession::join(opts(
        &server,
        &d,
        "Ada",
        Some(TOKEN),
    )));
    let id = event(&mut d, |e| match e {
        ReceiverEvent::JoinRequest { id, .. } => Some(*id),
        _ => None,
    })
    .await;
    d.decide_join(id, false);
    assert!(again.await.unwrap().is_err(), "denied like any join");
    d.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_presenter_the_host_removed_loses_its_token() {
    let server = start_server().await;
    let mut d = display(&server, JoinMode::Confirm, Duration::from_secs(60)).await;
    let p = first_join(&server, &mut d, "Ada").await;
    p.request_handover(TOKEN).await.expect("handover_ok");
    event(&mut d, |e| {
        matches!(e, ReceiverEvent::HandoverStored { name } if name == "Ada").then_some(())
    })
    .await;

    // The host removes Ada (no block): the token is withdrawn with her.
    d.remove_peer(false);
    let mut withdrawn = false;
    event(&mut d, |e| {
        withdrawn |= matches!(e, ReceiverEvent::HandoverExpired);
        matches!(e, ReceiverEvent::PeerLeft).then_some(())
    })
    .await;
    assert!(withdrawn, "the app is told that the hand-over is gone");
    drop(p);

    // Coming back with the token is an ordinary request for the host, not a silent join.
    let back = tokio::spawn(PresenterSession::join(opts(
        &server,
        &d,
        "Ada",
        Some(TOKEN),
    )));
    let id = event(&mut d, |e| match e {
        ReceiverEvent::JoinRequest { id, name, .. } if name == "Ada" => Some(*id),
        ReceiverEvent::PeerJoined { .. } => panic!("let in without the host"),
        _ => None,
    })
    .await;
    d.decide_join(id, false);
    assert!(back.await.unwrap().is_err(), "denied like any join");
    d.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn locking_the_room_withdraws_the_tokens() {
    let server = start_server().await;
    let mut d = display(&server, JoinMode::Confirm, Duration::from_secs(60)).await;
    let p = first_join(&server, &mut d, "Ada").await;
    p.request_handover(TOKEN).await.expect("handover_ok");
    p.leave().await;
    event(&mut d, |e| {
        matches!(e, ReceiverEvent::PeerLeft).then_some(())
    })
    .await;
    d.set_join_mode(JoinMode::Locked);
    event(&mut d, |e| {
        matches!(e, ReceiverEvent::HandoverExpired).then_some(())
    })
    .await;
    // Open again for asking: the token no longer admits anybody by itself.
    d.set_join_mode(JoinMode::Confirm);
    let back = tokio::spawn(PresenterSession::join(opts(
        &server,
        &d,
        "Ada",
        Some(TOKEN),
    )));
    let id = event(&mut d, |e| match e {
        ReceiverEvent::JoinRequest { id, .. } => Some(*id),
        ReceiverEvent::PeerJoined { .. } => panic!("let in without the host"),
        _ => None,
    })
    .await;
    d.decide_join(id, false);
    assert!(back.await.unwrap().is_err());
    d.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_token_works_in_an_open_room_too() {
    let server = start_server().await;
    let mut d = display(&server, JoinMode::Open, Duration::from_secs(60)).await;
    let p = {
        let o = opts(&server, &d, "Ada", None);
        PresenterSession::join(o).await.unwrap()
    };
    event(&mut d, |e| {
        matches!(
            e,
            ReceiverEvent::PeerJoined {
                handover: false,
                ..
            }
        )
        .then_some(())
    })
    .await;
    p.request_handover(TOKEN).await.unwrap();
    p.leave().await;
    event(&mut d, |e| {
        matches!(e, ReceiverEvent::PeerLeft).then_some(())
    })
    .await;
    let _ext = PresenterSession::join(opts(&server, &d, "Ada", Some(TOKEN)))
        .await
        .unwrap();
    event(&mut d, |e| {
        matches!(e, ReceiverEvent::PeerJoined { handover: true, .. }).then_some(())
    })
    .await;
    // A join without the token is an ordinary join in Open (no flag).
    d.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_token_does_not_open_another_room_or_another_person() {
    let server = start_server().await;
    let mut a = display(&server, JoinMode::Confirm, Duration::from_secs(60)).await;
    let mut b = display(&server, JoinMode::Confirm, Duration::from_secs(60)).await;
    let p = first_join(&server, &mut a, "Ada").await;
    p.request_handover(TOKEN).await.unwrap();
    p.leave().await;
    event(&mut a, |e| {
        matches!(e, ReceiverEvent::PeerLeft).then_some(())
    })
    .await;

    // Another display (room) does not know the token: the host is asked.
    let other_room = tokio::spawn(PresenterSession::join(opts(
        &server,
        &b,
        "Ada",
        Some(TOKEN),
    )));
    let id = event(&mut b, |e| match e {
        ReceiverEvent::JoinRequest { id, .. } => Some(*id),
        _ => None,
    })
    .await;
    b.decide_join(id, false);
    assert!(other_room.await.unwrap().is_err());

    // Another name on the right display: also a request, and the token is still good for Ada.
    let other_name = tokio::spawn(PresenterSession::join(opts(
        &server,
        &a,
        "Mallory",
        Some(TOKEN),
    )));
    let id = event(&mut a, |e| match e {
        ReceiverEvent::JoinRequest { id, name, .. } if name == "Mallory" => Some(*id),
        _ => None,
    })
    .await;
    a.decide_join(id, false);
    assert!(other_name.await.unwrap().is_err());
    let ada = PresenterSession::join(opts(&server, &a, "Ada", Some(TOKEN)))
        .await
        .expect("the right person still gets in");
    ada.leave().await;
    a.stop().await;
    b.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_expired_token_is_an_ordinary_request() {
    let server = start_server().await;
    let mut d = display(&server, JoinMode::Confirm, Duration::from_millis(1500)).await;
    let p = first_join(&server, &mut d, "Ada").await;
    p.request_handover(TOKEN).await.unwrap();
    p.leave().await;
    event(&mut d, |e| {
        matches!(e, ReceiverEvent::PeerLeft).then_some(())
    })
    .await;
    // The display tells the app that the hand-over ran out.
    event(&mut d, |e| {
        matches!(e, ReceiverEvent::HandoverExpired).then_some(())
    })
    .await;
    let late = tokio::spawn(PresenterSession::join(opts(
        &server,
        &d,
        "Ada",
        Some(TOKEN),
    )));
    let id = event(&mut d, |e| match e {
        ReceiverEvent::JoinRequest { id, .. } => Some(*id),
        _ => None,
    })
    .await;
    d.decide_join(id, true);
    late.await.unwrap().expect("allowed by the host");
    d.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_display_without_a_token_store_never_answers() {
    // A display that cannot hold tokens (here: content not accepted) does not answer; the
    // presenter's request fails after a short wait and the join falls back to the ordinary way.
    let server = start_server().await;
    let mut o = ReceiverOptions::new(&server);
    o.session = local_session();
    let mut d = ReceiverCore::start(o).await.unwrap();
    let p = PresenterSession::join(opts(&server, &d, "Ada", None))
        .await
        .unwrap();
    let r = p.request_handover(TOKEN).await;
    assert!(r.is_err(), "no answer from a display without content");
    p.leave().await;
    let _ = event(&mut d, |e| {
        matches!(e, ReceiverEvent::PeerLeft).then_some(())
    })
    .await;
    d.stop().await;
}
