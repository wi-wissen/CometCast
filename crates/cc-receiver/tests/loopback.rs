// SPDX-License-Identifier: GPL-3.0-or-later
//! Native receiver and native sender against a real cc-server running in this process.
//! Media goes over real UDP sockets on 127.0.0.1 (no mocks).

use std::time::Duration;

use cc_receiver::{ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_sender::{SenderCore, SenderEvent, SenderOptions};
use cc_server::{ServerConfig, run};
use cc_transport::{SessionConfig, VideoCodec};
use tokio::net::TcpListener;

/// Start the server on an ephemeral port and return `host:port`.
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sender_to_receiver_over_real_server() {
    let server = start_server().await;

    let mut ropts = ReceiverOptions::new(&server);
    ropts.session = local_session();
    let mut receiver = ReceiverCore::start(ropts).await.expect("receiver start");
    let code = receiver.room_code().clone();

    let mut sopts = SenderOptions::new(&server, code);
    sopts.session = local_session();
    sopts.duration = Some(Duration::from_secs(3));
    let mut sender = SenderCore::start(sopts).await.expect("sender start");

    // Wait until the receiver has seen enough packets (or fail after 20 s).
    const WANTED_PACKETS: u64 = 20;
    let mut saw_track = None;
    let result = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            tokio::select! {
                ev = receiver.next_event() => match ev {
                    Some(ReceiverEvent::VideoTrack { codec, mime }) => saw_track = Some((codec, mime)),
                    Some(ReceiverEvent::Stats(s)) if s.packets >= WANTED_PACKETS => return s,
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

    assert!(result.packets >= WANTED_PACKETS);
    assert!(result.bytes > 0);
    assert!(result.frames > 0, "marker bits should count frames");
    // Every test frame is a VP8 key frame, so key frames must be detected.
    assert!(result.keyframes > 0, "no key frame detected: {result:?}");
    let (codec, mime) = saw_track.expect("VideoTrack event");
    assert_eq!(codec, Some(VideoCodec::Vp8), "mime was {mime}");

    sender.stop().await;
    // The receiver must notice that the sender left.
    let left = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(ev) = receiver.next_event().await {
            if ev == ReceiverEvent::PeerLeft {
                return true;
            }
        }
        false
    })
    .await
    .expect("timeout waiting for PeerLeft");
    assert!(left);
    receiver.stop().await;
}

#[tokio::test]
async fn sender_with_wrong_code_gets_room_not_found() {
    let server = start_server().await;
    let code = cc_proto::RoomCode::parse("ZZZZZZ").unwrap();
    let err = SenderCore::start(SenderOptions::new(&server, code))
        .await
        .err()
        .expect("must fail");
    assert!(err.to_string().contains("room_not_found"), "{err}");
}
