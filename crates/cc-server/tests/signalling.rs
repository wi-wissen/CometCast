// SPDX-License-Identifier: GPL-3.0-or-later
//! Integration tests: a real server on an ephemeral port, real WebSocket clients.

use std::time::Duration;

use cc_server::{ServerConfig, run};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Ws = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// Start a server on port 0 (free port chosen by the OS). Returns its address.
async fn start(config: ServerConfig) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(run(listener, config, std::future::pending()));
    addr
}

async fn connect(addr: std::net::SocketAddr) -> Ws {
    connect_async(format!("ws://{addr}/ws")).await.unwrap().0
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::text(v.to_string())).await.unwrap();
}

/// Next JSON text message; panics on timeout or non-text.
async fn recv(ws: &mut Ws) -> Value {
    let m = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("timeout waiting for message")
        .expect("stream ended")
        .expect("ws error");
    match m {
        Message::Text(t) => serde_json::from_str(t.as_str()).unwrap(),
        other => panic!("unexpected frame: {other:?}"),
    }
}

/// Wait until the connection is closed (close frame, error or end of stream).
async fn expect_closed(ws: &mut Ws) {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
            Err(_) => panic!("connection not closed in time"),
            Ok(None) | Ok(Some(Err(_))) | Ok(Some(Ok(Message::Close(_)))) => return,
            Ok(Some(Ok(_))) => {}
        }
    }
}

async fn hello(ws: &mut Ws, role: &str) {
    send(ws, json!({"type":"hello","proto_version":1,"role":role})).await;
    assert_eq!(recv(ws).await, json!({"type":"hello","proto_version":1}));
}

/// Receiver creates a room; sender joins. Returns (receiver, sender, code).
async fn paired(addr: std::net::SocketAddr) -> (Ws, Ws, String) {
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    send(&mut r, json!({"type":"create_room"})).await;
    let created = recv(&mut r).await;
    assert_eq!(created["type"], "room_created");
    let code = created["code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 6);

    let mut s = connect(addr).await;
    hello(&mut s, "sender").await;
    send(&mut s, json!({"type":"join_room","code":code})).await;
    assert_eq!(recv(&mut s).await, json!({"type":"joined"}));
    assert_eq!(recv(&mut r).await, json!({"type":"peer_joined"}));
    (r, s, code)
}

fn error_code(v: &Value) -> &str {
    assert_eq!(v["type"], "error", "expected error, got {v}");
    v["code"].as_str().unwrap()
}

#[tokio::test]
async fn happy_path_relays_everything() {
    let addr = start(ServerConfig::default()).await;
    let (mut r, mut s, _) = paired(addr).await;

    send(&mut s, json!({"type":"offer","sdp":"v=0 offer"})).await;
    assert_eq!(
        recv(&mut r).await,
        json!({"type":"offer","sdp":"v=0 offer"})
    );

    send(&mut r, json!({"type":"answer","sdp":"v=0 answer"})).await;
    assert_eq!(
        recv(&mut s).await,
        json!({"type":"answer","sdp":"v=0 answer"})
    );

    send(&mut s, json!({"type":"ice_candidate","candidate":"c1"})).await;
    assert_eq!(
        recv(&mut r).await,
        json!({"type":"ice_candidate","candidate":"c1"})
    );
    send(&mut r, json!({"type":"ice_candidate","candidate":"c2"})).await;
    assert_eq!(
        recv(&mut s).await,
        json!({"type":"ice_candidate","candidate":"c2"})
    );

    send(
        &mut s,
        json!({"type":"control","cmd":{"go_to_page":{"page":3}}}),
    )
    .await;
    assert_eq!(
        recv(&mut r).await,
        json!({"type":"control","cmd":{"go_to_page":{"page":3}}})
    );
}

#[tokio::test]
async fn unknown_code_is_room_not_found() {
    let addr = start(ServerConfig::default()).await;
    let mut s = connect(addr).await;
    hello(&mut s, "sender").await;
    send(&mut s, json!({"type":"join_room","code":"ABCDEF"})).await;
    assert_eq!(error_code(&recv(&mut s).await), "room_not_found");
}

#[tokio::test]
async fn second_sender_gets_room_full() {
    let addr = start(ServerConfig::default()).await;
    let (_r, _s, code) = paired(addr).await;
    let mut s2 = connect(addr).await;
    hello(&mut s2, "sender").await;
    send(&mut s2, json!({"type":"join_room","code":code})).await;
    assert_eq!(error_code(&recv(&mut s2).await), "room_full");
}

#[tokio::test]
async fn wrong_role_is_rejected() {
    let addr = start(ServerConfig::default()).await;
    let mut s = connect(addr).await;
    hello(&mut s, "sender").await;
    send(&mut s, json!({"type":"create_room"})).await;
    assert_eq!(error_code(&recv(&mut s).await), "wrong_role");

    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    send(&mut r, json!({"type":"join_room","code":"ABCDEF"})).await;
    assert_eq!(error_code(&recv(&mut r).await), "wrong_role");
}

#[tokio::test]
async fn relay_before_pairing_is_not_in_room() {
    let addr = start(ServerConfig::default()).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    // Not in a room at all.
    send(&mut r, json!({"type":"offer","sdp":"x"})).await;
    assert_eq!(error_code(&recv(&mut r).await), "not_in_room");
    // In a room, but no sender yet.
    send(&mut r, json!({"type":"create_room"})).await;
    assert_eq!(recv(&mut r).await["type"], "room_created");
    send(&mut r, json!({"type":"offer","sdp":"x"})).await;
    assert_eq!(error_code(&recv(&mut r).await), "not_in_room");
}

#[tokio::test]
async fn first_message_not_hello_is_bad_hello() {
    let addr = start(ServerConfig::default()).await;
    let mut c = connect(addr).await;
    send(&mut c, json!({"type":"create_room"})).await;
    assert_eq!(error_code(&recv(&mut c).await), "bad_hello");
    expect_closed(&mut c).await;
}

#[tokio::test]
async fn silent_connection_is_closed() {
    let config = ServerConfig {
        hello_timeout: Duration::from_millis(200),
        ..ServerConfig::default()
    };
    let addr = start(config).await;
    let mut c = connect(addr).await;
    assert_eq!(error_code(&recv(&mut c).await), "bad_hello");
    expect_closed(&mut c).await;
}

#[tokio::test]
async fn wrong_proto_version_is_proto_mismatch() {
    let addr = start(ServerConfig::default()).await;
    let mut c = connect(addr).await;
    send(
        &mut c,
        json!({"type":"hello","proto_version":999,"role":"sender"}),
    )
    .await;
    assert_eq!(error_code(&recv(&mut c).await), "proto_mismatch");
    expect_closed(&mut c).await;
}

#[tokio::test]
async fn garbage_after_hello_is_bad_message_and_connection_stays() {
    let addr = start(ServerConfig::default()).await;
    let mut c = connect(addr).await;
    hello(&mut c, "receiver").await;
    c.send(Message::text("not json")).await.unwrap();
    assert_eq!(error_code(&recv(&mut c).await), "bad_message");
    send(&mut c, json!({"type":"create_room"})).await;
    assert_eq!(recv(&mut c).await["type"], "room_created");
}

#[tokio::test]
async fn sender_leaving_tells_receiver_and_room_can_be_rejoined() {
    let addr = start(ServerConfig::default()).await;
    let (mut r, mut s, code) = paired(addr).await;
    s.close(None).await.unwrap();
    assert_eq!(recv(&mut r).await, json!({"type":"peer_left"}));

    let mut s2 = connect(addr).await;
    hello(&mut s2, "sender").await;
    send(&mut s2, json!({"type":"join_room","code":code})).await;
    assert_eq!(recv(&mut s2).await, json!({"type":"joined"}));
    assert_eq!(recv(&mut r).await, json!({"type":"peer_joined"}));
}

#[tokio::test]
async fn receiver_leaving_tells_sender_and_ends_room() {
    let addr = start(ServerConfig::default()).await;
    let (mut r, mut s, code) = paired(addr).await;
    r.close(None).await.unwrap();
    assert_eq!(recv(&mut s).await, json!({"type":"peer_left"}));

    let mut s2 = connect(addr).await;
    hello(&mut s2, "sender").await;
    send(&mut s2, json!({"type":"join_room","code":code})).await;
    assert_eq!(error_code(&recv(&mut s2).await), "room_not_found");
}

#[tokio::test]
async fn idle_room_without_sender_expires() {
    let config = ServerConfig {
        idle_room_timeout: Duration::from_millis(300),
        ..ServerConfig::default()
    };
    let addr = start(config).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    send(&mut r, json!({"type":"create_room"})).await;
    let code = recv(&mut r).await["code"].as_str().unwrap().to_string();
    expect_closed(&mut r).await;

    let mut s = connect(addr).await;
    hello(&mut s, "sender").await;
    send(&mut s, json!({"type":"join_room","code":code})).await;
    assert_eq!(error_code(&recv(&mut s).await), "room_not_found");
}

#[tokio::test]
async fn room_with_sender_does_not_expire() {
    let config = ServerConfig {
        idle_room_timeout: Duration::from_millis(300),
        ..ServerConfig::default()
    };
    let addr = start(config).await;
    let (mut r, mut s, _) = paired(addr).await;
    tokio::time::sleep(Duration::from_millis(900)).await;
    send(&mut s, json!({"type":"offer","sdp":"still here"})).await;
    assert_eq!(recv(&mut r).await["sdp"], "still here");
}

#[tokio::test]
async fn oversize_message_closes_connection() {
    let config = ServerConfig {
        max_message_bytes: 1024,
        ..ServerConfig::default()
    };
    let addr = start(config).await;
    let mut c = connect(addr).await;
    hello(&mut c, "receiver").await;
    let big = "x".repeat(4096);
    send(&mut c, json!({"type":"offer","sdp":big})).await;
    expect_closed(&mut c).await;
}

#[tokio::test]
async fn healthz_and_static_files() {
    let dir = std::env::temp_dir().join(format!("cc-web-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("index.html"), "<h1>hi</h1>").unwrap();
    let config = ServerConfig {
        web_dir: Some(dir.clone()),
        ..ServerConfig::default()
    };
    let addr = start(config).await;

    // A tiny hand-made HTTP/1.0 request, so no HTTP client library is needed.
    async fn get(addr: std::net::SocketAddr, path: &str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
        s.write_all(format!("GET {path} HTTP/1.0\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        out
    }
    let health = get(addr, "/healthz").await;
    assert!(health.starts_with("HTTP/1.0 200") || health.starts_with("HTTP/1.1 200"));
    assert!(health.ends_with("ok"));
    let index = get(addr, "/").await;
    assert!(index.contains("<h1>hi</h1>"));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn pings_do_not_keep_a_silent_client_alive() {
    let config = ServerConfig {
        hello_timeout: Duration::from_millis(300),
        ..ServerConfig::default()
    };
    let addr = start(config).await;
    let mut c = connect(addr).await;
    let start_time = std::time::Instant::now();
    // Ping every 50 ms; the server must still give up after about 300 ms.
    loop {
        assert!(
            start_time.elapsed() < Duration::from_secs(3),
            "never closed"
        );
        if c.send(Message::Ping(vec![1].into())).await.is_err() {
            return;
        }
        match tokio::time::timeout(Duration::from_millis(50), c.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                assert!(t.as_str().contains("bad_hello"));
                break;
            }
            Ok(None) | Ok(Some(Err(_))) => return,
            // A pong comes back at once; keep to one ping per 50 ms (a faster flood is
            // cut off by the message budget, see ping_flood_closes_the_connection).
            _ => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
    expect_closed(&mut c).await;
}

#[tokio::test]
async fn connection_without_room_is_closed() {
    let config = ServerConfig {
        no_room_timeout: Duration::from_millis(300),
        ..ServerConfig::default()
    };
    let addr = start(config).await;
    let mut c = connect(addr).await;
    hello(&mut c, "sender").await;
    expect_closed(&mut c).await;
}

#[tokio::test]
async fn malformed_code_is_room_not_found() {
    let addr = start(ServerConfig::default()).await;
    let mut s = connect(addr).await;
    hello(&mut s, "sender").await;
    send(&mut s, json!({"type":"join_room","code":"bad"})).await;
    assert_eq!(error_code(&recv(&mut s).await), "room_not_found");
    send(&mut s, json!({"type":"join_room","code":"ABCDE0"})).await;
    assert_eq!(error_code(&recv(&mut s).await), "room_not_found");
    // A code that is not a string at all is still a bad message.
    send(&mut s, json!({"type":"join_room","code":5})).await;
    assert_eq!(error_code(&recv(&mut s).await), "bad_message");
}

#[tokio::test]
async fn sender_can_join_another_room_after_receiver_left() {
    let addr = start(ServerConfig::default()).await;
    let (mut r, mut s, _) = paired(addr).await;
    r.close(None).await.unwrap();
    assert_eq!(recv(&mut s).await, json!({"type":"peer_left"}));

    let mut r2 = connect(addr).await;
    hello(&mut r2, "receiver").await;
    send(&mut r2, json!({"type":"create_room"})).await;
    let code = recv(&mut r2).await["code"].as_str().unwrap().to_string();
    send(&mut s, json!({"type":"join_room","code":code})).await;
    assert_eq!(recv(&mut s).await, json!({"type":"joined"}));
    assert_eq!(recv(&mut r2).await, json!({"type":"peer_joined"}));
}

#[tokio::test]
async fn slow_reader_is_disconnected() {
    let mut config = ServerConfig {
        outgoing_queue_len: 4,
        ..ServerConfig::default()
    };
    // This test floods on purpose, so the per-connection and per-room budgets are lifted.
    config.limits.msg_rate_per_sec = 1_000_000;
    config.limits.msg_burst = 1_000_000;
    config.limits.bytes_per_sec = u32::MAX;
    config.limits.bytes_burst = u32::MAX;
    config.limits.max_room_relay_bytes = u64::MAX;
    let addr = start(config).await;
    let (r, s, _) = paired(addr).await;
    // The sender never reads. The receiver floods it with big messages while
    // also reading, because once the sender is gone the server answers with
    // not_in_room errors that the receiver must not leave unread.
    let big = "x".repeat(40 * 1024);
    let (mut w, mut rd) = r.split();
    let mut got_peer_left = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while !got_peer_left && tokio::time::Instant::now() < deadline {
        tokio::select! {
            res = w.send(Message::text(json!({"type":"offer","sdp":big}).to_string())) => {
                if res.is_err() { break; }
            }
            m = rd.next() => match m {
                Some(Ok(Message::Text(t))) if t.as_str().contains("peer_left") => got_peer_left = true,
                Some(Ok(_)) => {}
                _ => break,
            },
        }
    }
    assert!(got_peer_left, "slow sender was not disconnected");
    drop(s);
}
