// SPDX-License-Identifier: GPL-3.0-or-later
//! Native clients and join approval against a real `cc-server` (all on 127.0.0.1).

use std::time::Duration;

use cc_proto::JoinMode;
use cc_receiver::{ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_sender::{JoinProgress, SenderCore, SenderOptions};
use cc_server::{ServerConfig, run};
use cc_transport::SessionConfig;
use tokio::net::TcpListener;

async fn start(config: ServerConfig) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(run(listener, config, std::future::pending()));
    addr.to_string()
}

fn local_session() -> SessionConfig {
    SessionConfig {
        udp_bind: vec!["127.0.0.1:0".into()],
        ..SessionConfig::default()
    }
}

async fn receiver(server: &str, mode: JoinMode) -> ReceiverCore {
    let mut o = ReceiverOptions::new(server);
    o.session = local_session();
    o.join_mode = mode;
    ReceiverCore::start(o).await.unwrap()
}

async fn next_matching<T>(
    r: &mut ReceiverCore,
    mut f: impl FnMut(ReceiverEvent) -> Option<T>,
) -> T {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let ev = r.next_event().await.expect("receiver ended");
            if let Some(v) = f(ev) {
                return v;
            }
        }
    })
    .await
    .expect("timeout")
}

#[tokio::test]
async fn confirm_allow_and_deny_with_native_cores() {
    let server = start(ServerConfig::default()).await;
    let mut rx = receiver(&server, JoinMode::Confirm).await;
    assert_eq!(rx.join_mode(), JoinMode::Confirm);
    let code = rx.room_code().clone();

    // First sender is denied.
    let (ptx, mut prx) = tokio::sync::mpsc::unbounded_channel();
    let mut o = SenderOptions::new(&server, code.clone());
    o.session = local_session();
    o.name = Some("Ana".into());
    o.join_progress = Some(ptx);
    let denied = tokio::spawn(SenderCore::start(o));
    let (id, name, check) = next_matching(&mut rx, |e| match e {
        ReceiverEvent::JoinRequest {
            id, name, check, ..
        } => Some((id, name, check)),
        _ => None,
    })
    .await;
    assert_eq!(name, "Ana");
    let JoinProgress::Pending {
        timeout_secs,
        check: sender_check,
        room_name,
    } = prx.recv().await.unwrap();
    assert_eq!(timeout_secs, 60);
    assert_eq!(check, sender_check);
    assert_eq!(room_name, None);
    rx.decide_join(id, false);
    let err = denied.await.unwrap().err().expect("must be denied");
    assert!(err.to_string().contains("join_denied"), "{err}");

    // Second sender is allowed and streams.
    let mut o = SenderOptions::new(&server, code);
    o.session = local_session();
    let allowed = tokio::spawn(SenderCore::start(o));
    let id = next_matching(&mut rx, |e| match e {
        ReceiverEvent::JoinRequest { id, name, .. } => {
            assert_eq!(name, "Guest 2");
            Some(id)
        }
        _ => None,
    })
    .await;
    rx.decide_join(id, true);
    let sender = allowed.await.unwrap().expect("allowed");
    next_matching(&mut rx, |e| match e {
        ReceiverEvent::PeerJoined { name, .. } => Some(name),
        _ => None,
    })
    .await;
    // Switching modes is confirmed by the server.
    rx.set_join_mode(JoinMode::Locked);
    let m = next_matching(&mut rx, |e| match e {
        ReceiverEvent::JoinModeChanged(m) => Some(m),
        _ => None,
    })
    .await;
    assert_eq!(m, JoinMode::Locked);
    drop(sender);
    rx.stop().await;
}

#[tokio::test]
async fn locked_room_refuses_native_sender_and_kick_ends_the_session() {
    let server = start(ServerConfig::default()).await;
    let mut rx = receiver(&server, JoinMode::Locked).await;
    let code = rx.room_code().clone();
    let mut o = SenderOptions::new(&server, code.clone());
    o.session = local_session();
    let err = SenderCore::start(o).await.err().expect("locked");
    assert!(err.to_string().contains("room_locked"), "{err}");

    rx.set_join_mode(JoinMode::Open);
    next_matching(&mut rx, |e| match e {
        ReceiverEvent::JoinModeChanged(JoinMode::Open) => Some(()),
        _ => None,
    })
    .await;
    let mut o = SenderOptions::new(&server, code);
    o.session = local_session();
    let mut sender = SenderCore::start(o).await.unwrap();
    next_matching(&mut rx, |e| match e {
        ReceiverEvent::PeerJoined { .. } => Some(()),
        _ => None,
    })
    .await;
    rx.remove_peer(false);
    let ev = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            match sender.next_event().await {
                Some(cc_sender::SenderEvent::RemovedByHost) => return true,
                Some(_) => {}
                None => return false,
            }
        }
    })
    .await
    .expect("timeout");
    assert!(ev, "sender must learn that it was removed");
    rx.stop().await;
}

#[tokio::test]
async fn block_unblock_and_end_room_with_native_cores() {
    let server = start(ServerConfig::default()).await;
    let mut rx = receiver(&server, JoinMode::Confirm).await;
    let code = rx.room_code().clone();
    let mut o = SenderOptions::new(&server, code.clone());
    o.session = local_session();
    o.name = Some("Pest".into());
    let first = tokio::spawn(SenderCore::start(o));
    let id = next_matching(&mut rx, |e| match e {
        ReceiverEvent::JoinRequest { id, .. } => Some(id),
        _ => None,
    })
    .await;
    rx.deny_and_block_join(id);
    let err = first.await.unwrap().err().expect("denied");
    assert!(err.to_string().contains("join_denied"), "{err}");
    let block_id = next_matching(&mut rx, |e| match e {
        ReceiverEvent::Blocked { id, name } => {
            assert_eq!(name, "Pest");
            Some(id)
        }
        _ => None,
    })
    .await;
    // Blocked: denied at once, the host sees no request.
    let mut o = SenderOptions::new(&server, code.clone());
    o.session = local_session();
    let err = SenderCore::start(o).await.err().expect("blocked");
    assert!(err.to_string().contains("join_denied"), "{err}");
    rx.unblock(block_id);
    next_matching(&mut rx, |e| match e {
        ReceiverEvent::Unblocked { id } if id == block_id => Some(()),
        _ => None,
    })
    .await;
    rx.end_room();
    next_matching(&mut rx, |e| match e {
        ReceiverEvent::RoomEnded => Some(()),
        _ => None,
    })
    .await;
    rx.stop().await;
}

#[tokio::test]
async fn room_name_reaches_native_senders_through_pending_and_joined() {
    let server = start(ServerConfig::default()).await;
    let mut o = ReceiverOptions::new(&server);
    o.session = local_session();
    o.join_mode = JoinMode::Confirm;
    o.room_name = Some("  Meeting\u{202E} room   Elbe ".into());
    let mut rx = ReceiverCore::start(o).await.unwrap();
    let code = rx.room_code().clone();

    let (ptx, mut prx) = tokio::sync::mpsc::unbounded_channel();
    let mut o = SenderOptions::new(&server, code);
    o.session = local_session();
    o.join_progress = Some(ptx);
    let joined = tokio::spawn(SenderCore::start(o));
    let id = next_matching(&mut rx, |e| match e {
        ReceiverEvent::JoinRequest { id, .. } => Some(id),
        _ => None,
    })
    .await;
    let JoinProgress::Pending { room_name, .. } = prx.recv().await.unwrap();
    assert_eq!(room_name.as_deref(), Some("Meeting room Elbe"));
    rx.decide_join(id, true);
    let sender = joined.await.unwrap().expect("allowed");
    assert_eq!(sender.room_name(), Some("Meeting room Elbe"));
    drop(sender);
    rx.stop().await;
}

/// A host command the server refuses without closing the connection (here a decision on a
/// request that is no longer waiting, as when the joiner cancels at the same moment) is a
/// notice: the room, its code and the connected presenter stay.
#[tokio::test]
async fn no_such_request_keeps_the_room() {
    let server = start(ServerConfig::default()).await;
    let mut rx = receiver(&server, JoinMode::Confirm).await;
    let code = rx.room_code().clone();

    // A presenter is in the room.
    let mut o = SenderOptions::new(&server, code.clone());
    o.session = local_session();
    let allowed = tokio::spawn(SenderCore::start(o));
    let id = next_matching(&mut rx, |e| match e {
        ReceiverEvent::JoinRequest { id, .. } => Some(id),
        _ => None,
    })
    .await;
    rx.decide_join(id, true);
    let sender = allowed.await.unwrap().expect("allowed");
    next_matching(&mut rx, |e| match e {
        ReceiverEvent::PeerJoined { .. } => Some(()),
        _ => None,
    })
    .await;

    // Deciding on the same request again: it is not waiting any more.
    rx.decide_join(id, false);
    let notice = next_matching(&mut rx, |e| match e {
        ReceiverEvent::ServerNotice { code, .. } => Some(code),
        ReceiverEvent::Error(e) => panic!("the room ended: {e}"),
        ReceiverEvent::PeerLeft => panic!("the presenter was dropped"),
        _ => None,
    })
    .await;
    assert_eq!(notice, "no_such_request");

    // The room still answers commands, with the same code, and the presenter is still in it.
    rx.set_join_mode(JoinMode::Locked);
    let m = next_matching(&mut rx, |e| match e {
        ReceiverEvent::JoinModeChanged(m) => Some(m),
        ReceiverEvent::Error(e) => panic!("the room ended: {e}"),
        ReceiverEvent::PeerLeft => panic!("the presenter was dropped"),
        _ => None,
    })
    .await;
    assert_eq!(m, JoinMode::Locked);
    assert_eq!(rx.room_code(), &code);
    drop(sender);
    rx.stop().await;
}
