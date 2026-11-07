// SPDX-License-Identifier: GPL-3.0-or-later
//! Native clients against a real `cc-server` in open mode (ALTCHA always required), over TLS,
//! and against an internal-mode server that hands out TURN credentials.
//! Everything runs in this process on 127.0.0.1; media goes over real UDP.

use std::time::Duration;

use cc_proto::{ClientMsg, Role, RoomCode, ServerMsg};
use cc_receiver::{ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_sender::{SenderCore, SenderEvent, SenderOptions};
use cc_server::{Mode, Secret, ServerConfig, TurnConfig, run};
use cc_transport::{
    ServerAddr, SessionConfig, SignalingClient, TlsTrust, TransportError, altcha, ice, net,
};
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

/// Wait until the receiver has seen `wanted` RTP packets.
async fn wait_for_packets(receiver: &mut ReceiverCore, sender: &mut SenderCore, wanted: u64) {
    tokio::time::timeout(Duration::from_secs(25), async {
        loop {
            tokio::select! {
                ev = receiver.next_event() => match ev {
                    Some(ReceiverEvent::Stats(s)) if s.packets >= wanted => return,
                    Some(ReceiverEvent::Error(e)) => panic!("receiver error: {e}"),
                    Some(ReceiverEvent::PeerLeft) | None => panic!("receiver ended early"),
                    Some(_) => {}
                },
                ev = sender.next_event() => {
                    if let Some(SenderEvent::Error(e)) = ev {
                        panic!("sender error: {e}");
                    }
                }
            }
        }
    })
    .await
    .expect("timeout: receiver did not see enough RTP packets");
}

#[tokio::test]
async fn open_server_refuses_create_room_without_altcha() {
    let server = start(ServerConfig::for_mode(Mode::Open)).await;
    let mut sig = SignalingClient::connect(&server, Role::Receiver)
        .await
        .unwrap();
    sig.send(ClientMsg::CreateRoom {
        altcha: None,
        join_mode: None,
    })
    .await
    .unwrap();
    match sig.recv().await {
        Some(ServerMsg::Error { code, .. }) => assert_eq!(code, "altcha_required"),
        other => panic!("expected altcha_required, got {other:?}"),
    }
    // A made-up payload is refused as invalid; the connection stays usable.
    sig.send(ClientMsg::CreateRoom {
        altcha: Some("bm90IGFsdGNoYQ==".into()),
        join_mode: None,
    })
    .await
    .unwrap();
    match sig.recv().await {
        Some(ServerMsg::Error { code, .. }) => assert_eq!(code, "altcha_invalid"),
        other => panic!("expected altcha_invalid, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn receiver_solves_altcha_and_video_flows_in_open_mode() {
    let server = start(ServerConfig::for_mode(Mode::Open)).await;

    let mut ropts = ReceiverOptions::new(&server);
    ropts.session = local_session();
    let mut receiver = ReceiverCore::start(ropts)
        .await
        .expect("receiver start against an open server");
    let code = receiver.room_code().clone();

    let mut sopts = SenderOptions::new(&server, code);
    sopts.session = local_session();
    sopts.duration = Some(Duration::from_secs(3));
    let mut sender = SenderCore::start(sopts).await.expect("sender start");

    wait_for_packets(&mut receiver, &mut sender, 20).await;
    sender.stop().await;
    receiver.stop().await;
}

#[tokio::test]
async fn a_puzzle_can_be_used_only_once() {
    let server = start(ServerConfig::for_mode(Mode::Open)).await;
    let addr = ServerAddr::parse(&server).unwrap();
    let payload = altcha::fetch_and_solve(&addr, &TlsTrust::system())
        .await
        .unwrap();

    let mut sig = SignalingClient::connect(&server, Role::Receiver)
        .await
        .unwrap();
    sig.send(ClientMsg::CreateRoom {
        altcha: Some(payload.clone()),
        join_mode: None,
    })
    .await
    .unwrap();
    assert!(matches!(
        sig.recv().await,
        Some(ServerMsg::RoomCreated { .. })
    ));
    // A second connection replays the same solution.
    let mut sig2 = SignalingClient::connect(&server, Role::Receiver)
        .await
        .unwrap();
    sig2.send(ClientMsg::CreateRoom {
        altcha: Some(payload),
        join_mode: None,
    })
    .await
    .unwrap();
    match sig2.recv().await {
        Some(ServerMsg::Error { code, .. }) => assert_eq!(code, "altcha_invalid"),
        other => panic!("expected altcha_invalid, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn wss_and_https_with_a_self_signed_certificate() {
    let cert_path =
        std::env::temp_dir().join(format!("cometcast-test-cert-{}.pem", std::process::id()));
    let _ = std::fs::remove_file(&cert_path);
    let mut config = ServerConfig::for_mode(Mode::Open);
    config.tls.self_signed = true;
    config.tls.self_signed_cert_out = Some(cert_path.clone());
    let server = start(config).await;
    for _ in 0..100 {
        if cert_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let pem = std::fs::read(&cert_path).expect("server wrote its certificate");
    let _ = std::fs::remove_file(&cert_path);
    let url = format!("wss://{server}");

    // Without the certificate the handshake must fail (the certificate is not in any store).
    let err = SignalingClient::connect(&url, Role::Receiver)
        .await
        .err()
        .expect("untrusted certificate must be refused");
    assert!(matches!(err, TransportError::Connect(_)), "{err}");

    let mut trust = TlsTrust::system();
    trust.add_pem(&pem).unwrap();

    // HTTPS endpoints.
    let addr = ServerAddr::parse(&url).unwrap();
    assert!(addr.tls);
    let health = net::http_get(&addr, "/healthz", &trust).await.unwrap();
    assert_eq!((health.status, health.body.as_slice()), (200, &b"ok"[..]));
    let ice_answer = net::http_get(&addr, "/ice-servers", &trust).await.unwrap();
    assert_eq!(ice_answer.status, 404, "open mode has no TURN");

    // Signalling over WSS including ALTCHA over HTTPS, then a sender joins.
    let mut ropts = ReceiverOptions::new(&url);
    ropts.trust = trust.clone();
    ropts.session = local_session();
    let mut receiver = ReceiverCore::start(ropts).await.expect("receiver over wss");
    let code: RoomCode = receiver.room_code().clone();

    let mut sopts = SenderOptions::new(&url, code);
    sopts.trust = trust;
    sopts.session = local_session();
    sopts.duration = Some(Duration::from_secs(3));
    let mut sender = SenderCore::start(sopts).await.expect("sender over wss");
    wait_for_packets(&mut receiver, &mut sender, 20).await;
    sender.stop().await;
    receiver.stop().await;
}

#[tokio::test]
async fn ice_servers_come_from_an_internal_server_with_turn() {
    let mut config = ServerConfig::for_mode(Mode::Internal);
    config.turn = Some(TurnConfig {
        urls: vec!["turn:relay.example.org:3478?transport=udp".into()],
        stun_urls: vec!["stun:relay.example.org:3478".into()],
        secret: Secret("test-turn-secret-0123456789".into()),
        ttl: Duration::from_secs(3600),
    });
    let server = start(config).await;
    let addr = ServerAddr::parse(&server).unwrap();
    let list = ice::fetch_ice_servers(&addr, &TlsTrust::system())
        .await
        .unwrap();
    assert_eq!(list.len(), 2, "{list:?}");
    assert_eq!(list[0].urls, ["stun:relay.example.org:3478"]);
    assert!(list[0].username.is_none());
    let turn = &list[1];
    assert_eq!(turn.urls, ["turn:relay.example.org:3478?transport=udp"]);
    // username is "<expiry>:<id>", credential is the base64 HMAC-SHA1 (28 characters).
    let user = turn.username.as_deref().expect("username");
    assert!(
        user.split_once(':')
            .is_some_and(|(t, _)| t.parse::<u64>().is_ok())
    );
    let expected = cc_server::credential_for("test-turn-secret-0123456789", user);
    assert_eq!(turn.credential.as_deref(), Some(expected.as_str()));

    // The cores fetch the list by themselves and report what they use.
    let mut ropts = ReceiverOptions::new(&server);
    ropts.session = local_session();
    let receiver = ReceiverCore::start(ropts).await.unwrap();
    // (Every request gets a new random credential id, so only the shape can be compared.)
    assert_eq!(receiver.ice_servers().len(), 2);
    assert_eq!(receiver.ice_servers()[1].urls, turn.urls);
    assert!(receiver.ice_servers()[1].credential.is_some());
    let mut sopts = SenderOptions::new(&server, receiver.room_code().clone());
    sopts.session = local_session();
    let sender = SenderCore::start(sopts).await.unwrap();
    assert_eq!(sender.ice_servers().len(), 2);
    sender.stop().await;
    receiver.stop().await;

    // A server without TURN answers 404, which is "no list", not an error.
    let plain = start(ServerConfig::for_mode(Mode::Internal)).await;
    let none = ice::fetch_ice_servers(&ServerAddr::parse(&plain).unwrap(), &TlsTrust::system())
        .await
        .unwrap();
    assert!(none.is_empty());
}
