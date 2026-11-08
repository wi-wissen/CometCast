// SPDX-License-Identifier: GPL-3.0-or-later
//! Integration tests for server hardening (milestone M8): limits, bans, address
//! lists, ALTCHA, TURN credentials and TLS. Real server, real sockets.
#![allow(clippy::field_reassign_with_default)] // tests tweak one setting of a default config

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use cc_server::{
    AltchaMode, Mode, Secret, ServerConfig, TurnConfig, credential_for, load_config, run,
};
use futures_util::{SinkExt, StreamExt};
use ipnet::IpNet;
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream, connect_async};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

// ---------- helpers ----------

async fn start(config: ServerConfig) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(run(listener, config, std::future::pending()));
    addr
}

/// Config with the quick-to-solve puzzle used by the tests.
fn small_puzzle(mut c: ServerConfig) -> ServerConfig {
    c.altcha.cost = 10;
    c.altcha.base_counter = 20;
    c
}

fn nets(list: &[&str]) -> Vec<IpNet> {
    list.iter().map(|s| s.parse().unwrap()).collect()
}

async fn connect(addr: SocketAddr) -> Ws {
    connect_async(format!("ws://{addr}/ws")).await.unwrap().0
}

/// Connect with extra request headers. Returns the HTTP status when refused.
async fn connect_with(addr: SocketAddr, headers: &[(&'static str, &str)]) -> Result<Ws, u16> {
    let mut req = format!("ws://{addr}/ws").into_client_request().unwrap();
    for (k, v) in headers {
        req.headers_mut().insert(*k, v.parse().unwrap());
    }
    match connect_async(req).await {
        Ok((ws, _)) => Ok(ws),
        Err(tungstenite::Error::Http(resp)) => Err(resp.status().as_u16()),
        Err(e) => panic!("unexpected connect error: {e}"),
    }
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::text(v.to_string())).await.unwrap();
}

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

async fn expect_closed<S>(ws: &mut WebSocketStream<S>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
            Err(_) => panic!("connection not closed in time"),
            Ok(None) | Ok(Some(Err(_))) | Ok(Some(Ok(Message::Close(_)))) => return,
            Ok(Some(Ok(_))) => {}
        }
    }
}

async fn hello(ws: &mut Ws, role: &str) {
    send(ws, json!({"type":"hello","proto_version":2,"role":role})).await;
    assert_eq!(recv(ws).await, json!({"type":"hello","proto_version":2}));
}

fn error_code(v: &Value) -> &str {
    assert_eq!(v["type"], "error", "expected error, got {v}");
    v["code"].as_str().unwrap()
}

struct Http {
    status: u16,
    headers: String,
    body: String,
}

/// Minimal HTTP/1.1 GET over any stream (plain or TLS), reading until close.
async fn http_over<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    path: &str,
    headers: &[(&str, &str)],
) -> Http {
    let mut req = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.ok();
    let text = String::from_utf8_lossy(&raw).to_string();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    Http {
        status,
        headers: head.to_ascii_lowercase(),
        body: body.to_string(),
    }
}

async fn http_get(addr: SocketAddr, path: &str, headers: &[(&str, &str)]) -> Http {
    let tcp = TcpStream::connect(addr).await.unwrap();
    http_over(tcp, path, headers).await
}

/// Fetch a puzzle and solve it like a client would. Returns the base64 payload.
async fn solved_payload(addr: SocketAddr) -> String {
    let r = http_get(addr, "/altcha/challenge", &[]).await;
    assert_eq!(r.status, 200, "challenge: {}", r.headers);
    let challenge: altcha::Challenge = serde_json::from_str(&r.body).unwrap();
    let solution = altcha::solve_challenge(altcha::SolveChallengeOptions::new(&challenge))
        .unwrap()
        .unwrap();
    BASE64.encode(
        serde_json::to_vec(&altcha::Payload {
            challenge,
            solution,
        })
        .unwrap(),
    )
}

// ---------- rate limits, global caps, bans ----------

#[tokio::test]
async fn connection_rate_limit_per_ip() {
    let mut c = ServerConfig::default();
    c.limits.connections_per_min = 3;
    let addr = start(c).await;
    let mut keep = Vec::new();
    for _ in 0..3 {
        keep.push(connect(addr).await);
    }
    assert_eq!(connect_with(addr, &[]).await.err(), Some(429));
}

#[tokio::test]
async fn global_connection_cap() {
    let mut c = ServerConfig::default();
    c.limits.max_connections = 1;
    let addr = start(c).await;
    let first = connect(addr).await;
    assert_eq!(connect_with(addr, &[]).await.err(), Some(503));
    drop(first);
    // The place is free again once the first connection is gone.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(connect_with(addr, &[]).await.is_ok());
}

#[tokio::test]
async fn create_room_rate_limit_and_room_cap() {
    let mut c = ServerConfig::default();
    c.limits.create_room_per_min = 2;
    c.limits.max_rooms = 2;
    let addr = start(c).await;

    // Two rooms from two connections.
    let mut a = connect(addr).await;
    hello(&mut a, "receiver").await;
    send(&mut a, json!({"type":"create_room"})).await;
    assert_eq!(recv(&mut a).await["type"], "room_created");
    let mut b = connect(addr).await;
    hello(&mut b, "receiver").await;
    send(&mut b, json!({"type":"create_room"})).await;
    assert_eq!(recv(&mut b).await["type"], "room_created");

    // Third attempt: this address used its 2 per minute.
    let mut c3 = connect(addr).await;
    hello(&mut c3, "receiver").await;
    send(&mut c3, json!({"type":"create_room"})).await;
    assert_eq!(error_code(&recv(&mut c3).await), "rate_limited");
    // The connection stays usable (not closed).
    send(&mut c3, json!({"type":"nonsense"})).await;
    assert_eq!(error_code(&recv(&mut c3).await), "bad_message");
}

#[tokio::test]
async fn global_room_cap_is_enforced() {
    let mut c = ServerConfig::default();
    c.limits.max_rooms = 1;
    let addr = start(c).await;
    let mut a = connect(addr).await;
    hello(&mut a, "receiver").await;
    send(&mut a, json!({"type":"create_room"})).await;
    assert_eq!(recv(&mut a).await["type"], "room_created");
    let mut b = connect(addr).await;
    hello(&mut b, "receiver").await;
    send(&mut b, json!({"type":"create_room"})).await;
    let e = recv(&mut b).await;
    assert_eq!(error_code(&e), "rate_limited");
    assert!(e["message"].as_str().unwrap().contains("rooms"));
}

#[tokio::test]
async fn failed_joins_lead_to_a_ban() {
    let mut c = ServerConfig::default();
    c.limits.ban_after_failed_joins = 3;
    c.limits.join_attempts_per_min = 1000;
    c.limits.ban_durations = vec![Duration::from_secs(60)];
    let addr = start(c).await;

    let mut s = connect(addr).await;
    hello(&mut s, "sender").await;
    for _ in 0..2 {
        send(&mut s, json!({"type":"join_room","code":"AAAAAA"})).await;
        assert_eq!(error_code(&recv(&mut s).await), "room_not_found");
    }
    // Third failure: banned, connection closed.
    send(&mut s, json!({"type":"join_room","code":"BADCODE"})).await;
    assert_eq!(error_code(&recv(&mut s).await), "banned");
    expect_closed(&mut s).await;
    // The banned address cannot connect at all for now.
    assert_eq!(connect_with(addr, &[]).await.err(), Some(403));
}

#[tokio::test]
async fn good_joins_are_not_punished() {
    let mut c = ServerConfig::default();
    c.limits.ban_after_failed_joins = 2;
    let addr = start(c).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    send(&mut r, json!({"type":"create_room"})).await;
    let code = recv(&mut r).await["code"].as_str().unwrap().to_string();
    for _ in 0..4 {
        let mut s = connect(addr).await;
        hello(&mut s, "sender").await;
        send(&mut s, json!({"type":"join_room","code":code})).await;
        assert_eq!(recv(&mut s).await, json!({"type":"joined"}));
        assert_eq!(recv(&mut r).await["type"], "peer_joined");
        drop(s);
        assert_eq!(recv(&mut r).await, json!({"type":"peer_left"}));
    }
}

#[tokio::test]
async fn failed_join_rate_limit_counts_harder() {
    let mut c = ServerConfig::default();
    c.limits.join_attempts_per_min = 6;
    c.limits.failed_join_weight = 3;
    c.limits.ban_after_failed_joins = 100;
    let addr = start(c).await;
    let mut s = connect(addr).await;
    hello(&mut s, "sender").await;
    // Two failures use up 6 attempts.
    for _ in 0..2 {
        send(&mut s, json!({"type":"join_room","code":"AAAAAA"})).await;
        assert_eq!(error_code(&recv(&mut s).await), "room_not_found");
    }
    send(&mut s, json!({"type":"join_room","code":"AAAAAA"})).await;
    assert_eq!(error_code(&recv(&mut s).await), "rate_limited");
}

// ---------- client address, allow and deny lists ----------

#[tokio::test]
async fn deny_list_and_forwarded_for_from_trusted_proxy() {
    let mut c = ServerConfig::default();
    c.network.trusted_proxies = nets(&["127.0.0.0/8"]);
    c.network.deny = nets(&["203.0.113.5/32"]);
    let addr = start(c).await;

    // The proxy (127.0.0.1) says the client is 203.0.113.5: refused.
    let r = http_get(
        addr,
        "/altcha/challenge",
        &[("X-Forwarded-For", "203.0.113.5")],
    )
    .await;
    assert_eq!(r.status, 403);
    // A spoofed value on the left is ignored; the right-most untrusted address counts.
    let r = http_get(
        addr,
        "/altcha/challenge",
        &[("X-Forwarded-For", "198.51.100.1, 203.0.113.5")],
    )
    .await;
    assert_eq!(r.status, 403);
    let r = http_get(
        addr,
        "/altcha/challenge",
        &[("X-Forwarded-For", "203.0.113.5, 198.51.100.1")],
    )
    .await;
    assert_eq!(r.status, 200);
    // Same for WebSocket upgrades.
    assert_eq!(
        connect_with(addr, &[("x-forwarded-for", "203.0.113.5")])
            .await
            .err(),
        Some(403)
    );
    assert!(
        connect_with(addr, &[("x-forwarded-for", "198.51.100.1")])
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn forwarded_for_is_ignored_without_trusted_proxy() {
    let mut c = ServerConfig::default();
    c.network.deny = nets(&["203.0.113.5/32"]);
    let addr = start(c).await;
    // The peer is 127.0.0.1 and not trusted, so the header means nothing.
    let r = http_get(
        addr,
        "/altcha/challenge",
        &[("X-Forwarded-For", "203.0.113.5")],
    )
    .await;
    assert_eq!(r.status, 200);
}

#[tokio::test]
async fn allow_list_blocks_everybody_else_but_loopback_health_check() {
    let mut c = ServerConfig::default();
    c.network.allow = nets(&["10.0.0.0/8"]);
    let addr = start(c).await;
    assert_eq!(http_get(addr, "/altcha/challenge", &[]).await.status, 403);
    assert_eq!(connect_with(addr, &[]).await.err(), Some(403));
    // Health checks from the same computer still work.
    let h = http_get(addr, "/healthz", &[]).await;
    assert_eq!((h.status, h.body.as_str()), (200, "ok"));

    // With a trusted proxy that forwards an allowed address, access works.
    let mut c = ServerConfig::default();
    c.network.allow = nets(&["10.0.0.0/8"]);
    c.network.trusted_proxies = nets(&["127.0.0.1/32"]);
    let addr = start(c).await;
    let r = http_get(
        addr,
        "/altcha/challenge",
        &[("X-Forwarded-For", "10.1.2.3")],
    )
    .await;
    assert_eq!(r.status, 200);
}

#[tokio::test]
async fn rate_limit_uses_the_forwarded_client_not_the_proxy() {
    let mut c = ServerConfig::default();
    c.network.trusted_proxies = nets(&["127.0.0.0/8"]);
    c.limits.connections_per_min = 1;
    let addr = start(c).await;
    let _a = connect_with(addr, &[("x-forwarded-for", "198.51.100.1")])
        .await
        .unwrap();
    // Second connection of the same client: limited.
    assert_eq!(
        connect_with(addr, &[("x-forwarded-for", "198.51.100.1")])
            .await
            .err(),
        Some(429)
    );
    // A different client behind the same proxy is fine.
    assert!(
        connect_with(addr, &[("x-forwarded-for", "198.51.100.2")])
            .await
            .is_ok()
    );
}

// ---------- ALTCHA ----------

fn open_config() -> ServerConfig {
    small_puzzle(ServerConfig::for_mode(Mode::Open))
}

#[tokio::test]
async fn open_mode_requires_a_solved_puzzle_once() {
    let addr = start(open_config()).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;

    // No payload.
    send(&mut r, json!({"type":"create_room"})).await;
    assert_eq!(error_code(&recv(&mut r).await), "altcha_required");
    // Garbage payload.
    send(&mut r, json!({"type":"create_room","altcha":"AAAA"})).await;
    assert_eq!(error_code(&recv(&mut r).await), "altcha_invalid");

    // A real solution works ...
    let payload = solved_payload(addr).await;
    send(&mut r, json!({"type":"create_room","altcha":payload})).await;
    let created = recv(&mut r).await;
    assert_eq!(created["type"], "room_created");

    // ... but only once: the same payload from another receiver is a replay.
    let mut r2 = connect(addr).await;
    hello(&mut r2, "receiver").await;
    send(&mut r2, json!({"type":"create_room","altcha":payload})).await;
    assert_eq!(error_code(&recv(&mut r2).await), "altcha_invalid");

    // The sender side is not affected by ALTCHA when joining (limits protect it).
    let mut s = connect(addr).await;
    hello(&mut s, "sender").await;
    send(
        &mut s,
        json!({"type":"join_room","code":created["code"].as_str().unwrap()}),
    )
    .await;
    assert_eq!(recv(&mut s).await, json!({"type":"joined"}));
}

#[tokio::test]
async fn altcha_required_answer_costs_no_create_token() {
    let mut config = open_config();
    config.limits.create_room_per_min = 1;
    let addr = start(config).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    // The documented two steps: ask without a payload, then retry with one.
    send(&mut r, json!({"type":"create_room"})).await;
    assert_eq!(error_code(&recv(&mut r).await), "altcha_required");
    let payload = solved_payload(addr).await;
    send(&mut r, json!({"type":"create_room","altcha":payload})).await;
    assert_eq!(recv(&mut r).await["type"], "room_created");
}

#[tokio::test]
async fn open_mode_rejects_puzzle_from_another_server() {
    let other = start(open_config()).await;
    let addr = start(open_config()).await;
    let foreign = solved_payload(other).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    send(&mut r, json!({"type":"create_room","altcha":foreign})).await;
    assert_eq!(error_code(&recv(&mut r).await), "altcha_invalid");
}

#[tokio::test]
async fn internal_mode_altcha_is_optional_but_checked_when_sent() {
    let addr = start(small_puzzle(ServerConfig::default())).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    // Old clients send no field and still work.
    send(&mut r, json!({"type":"create_room"})).await;
    assert_eq!(recv(&mut r).await["type"], "room_created");

    let mut r2 = connect(addr).await;
    hello(&mut r2, "receiver").await;
    send(&mut r2, json!({"type":"create_room","altcha":"AAAA"})).await;
    assert_eq!(error_code(&recv(&mut r2).await), "altcha_invalid");
    let payload = solved_payload(addr).await;
    send(&mut r2, json!({"type":"create_room","altcha":payload})).await;
    assert_eq!(recv(&mut r2).await["type"], "room_created");
}

#[tokio::test]
async fn internal_mode_can_require_altcha() {
    let mut c = small_puzzle(ServerConfig::default());
    c.altcha.mode = AltchaMode::Always;
    let addr = start(c).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    send(&mut r, json!({"type":"create_room"})).await;
    assert_eq!(error_code(&recv(&mut r).await), "altcha_required");
}

#[tokio::test]
async fn challenge_endpoint_is_rate_limited_and_not_cached() {
    let mut c = small_puzzle(ServerConfig::default());
    c.limits.http_per_min = 2;
    let addr = start(c).await;
    let ok = http_get(addr, "/altcha/challenge", &[]).await;
    assert_eq!(ok.status, 200);
    assert!(ok.headers.contains("cache-control: no-store"));
    let v: Value = serde_json::from_str(&ok.body).unwrap();
    assert!(v["parameters"]["nonce"].is_string());
    assert_eq!(http_get(addr, "/altcha/challenge", &[]).await.status, 200);
    let limited = http_get(addr, "/altcha/challenge", &[]).await;
    assert_eq!(limited.status, 429);
    assert!(limited.headers.contains("retry-after:"));
}

// ---------- TURN ----------

fn turn_config() -> TurnConfig {
    TurnConfig {
        urls: vec!["turn:relay.example.org:3478?transport=udp".into()],
        stun_urls: vec!["stun:relay.example.org:3478".into()],
        secret: Secret("test-secret-0123456789".into()),
        ttl: Duration::from_secs(3600),
    }
}

#[tokio::test]
async fn ice_servers_endpoint_issues_verifiable_credentials() {
    let mut c = ServerConfig::default();
    c.turn = Some(turn_config());
    let addr = start(c).await;
    let r = http_get(addr, "/ice-servers", &[]).await;
    assert_eq!(r.status, 200);
    let list: Vec<Value> = serde_json::from_str(&r.body).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0], json!({"urls":["stun:relay.example.org:3478"]}));
    let turn = &list[1];
    let username = turn["username"].as_str().unwrap();
    let (expiry, id) = username.split_once(':').unwrap();
    let expiry: u64 = expiry.parse().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(
        expiry > now + 3600 && expiry <= now + 7201,
        "expiry {expiry}"
    );
    assert!(!id.is_empty());
    assert_eq!(
        turn["credential"].as_str().unwrap(),
        credential_for("test-secret-0123456789", username)
    );
    let ttl = turn["ttl"].as_u64().unwrap();
    assert!((3600..=7200).contains(&ttl), "ttl {ttl}");
    // Repeated requests of one client get the SAME user (coturn's user-quota counts them
    // together). Only a request that lands on a bucket boundary may differ.
    let r2 = http_get(addr, "/ice-servers", &[]).await;
    let list2: Vec<Value> = serde_json::from_str(&r2.body).unwrap();
    if list2[1]["username"]
        .as_str()
        .unwrap()
        .starts_with(&format!("{expiry}:"))
    {
        assert_eq!(list2[1]["username"], turn["username"]);
    }
}

#[tokio::test]
async fn ice_servers_is_404_without_turn() {
    let addr = start(ServerConfig::default()).await;
    assert_eq!(http_get(addr, "/ice-servers", &[]).await.status, 404);
}

#[tokio::test]
async fn ice_servers_obey_the_deny_list() {
    let mut c = ServerConfig::default();
    c.turn = Some(turn_config());
    c.network.deny = nets(&["127.0.0.0/8"]);
    let addr = start(c).await;
    assert_eq!(http_get(addr, "/ice-servers", &[]).await.status, 403);
}

#[tokio::test]
async fn invalid_settings_stop_the_server_from_starting() {
    // Open mode with TURN is refused when the server starts.
    let mut c = ServerConfig::for_mode(Mode::Open);
    c.turn = Some(turn_config());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let err = run(listener, c, std::future::pending()).await.unwrap_err();
    assert!(err.to_string().contains("internal mode"), "{err}");
}

// ---------- TLS ----------

fn client_tls(cert_pem: &str) -> Arc<rustls::ClientConfig> {
    use rustls::pki_types::CertificateDer;
    use rustls::pki_types::pem::PemObject;
    let mut roots = rustls::RootCertStore::empty();
    for cert in CertificateDer::pem_slice_iter(cert_pem.as_bytes()) {
        roots.add(cert.unwrap()).unwrap();
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    Arc::new(
        rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    )
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cc-tls-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Wait until the TLS server accepts connections.
async fn wait_listening(addr: SocketAddr) {
    for _ in 0..50 {
        if TcpStream::connect(addr).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("server not listening");
}

async fn wss_roundtrip(addr: SocketAddr, tls: Arc<rustls::ClientConfig>) {
    let (mut ws, _) = tokio_tungstenite::connect_async_tls_with_config(
        format!("wss://localhost:{}/ws", addr.port()),
        None,
        false,
        Some(Connector::Rustls(tls)),
    )
    .await
    .expect("WSS connection with the trusted certificate");
    send(
        &mut ws,
        json!({"type":"hello","proto_version":2,"role":"receiver"}),
    )
    .await;
    let m = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let Message::Text(t) = m else {
        panic!("not text")
    };
    assert_eq!(
        serde_json::from_str::<Value>(t.as_str()).unwrap(),
        json!({"type":"hello","proto_version":2})
    );
    send(&mut ws, json!({"type":"create_room"})).await;
    let m = ws.next().await.unwrap().unwrap();
    let Message::Text(t) = m else {
        panic!("not text")
    };
    assert_eq!(
        serde_json::from_str::<Value>(t.as_str()).unwrap()["type"],
        "room_created"
    );
}

#[tokio::test]
async fn self_signed_certificate_serves_wss_and_https() {
    let dir = temp_dir("selfsigned");
    let cert_out = dir.join("cert.pem");
    let mut c = ServerConfig::default();
    c.tls.self_signed = true;
    c.tls.self_signed_cert_out = Some(cert_out.clone());
    let addr = start(c).await;
    wait_listening(addr).await;
    // The certificate file is written before the server accepts connections.
    let pem = std::fs::read_to_string(&cert_out).unwrap();
    assert!(pem.contains("BEGIN CERTIFICATE"));
    assert!(
        !pem.contains("PRIVATE KEY"),
        "the key must never be written"
    );
    let tls = client_tls(&pem);

    wss_roundtrip(addr, tls.clone()).await;

    // Plain HTTPS request with the same trust.
    let tcp = TcpStream::connect(addr).await.unwrap();
    let name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
    let stream = tokio_rustls::TlsConnector::from(tls)
        .connect(name, tcp)
        .await
        .unwrap();
    let h = http_over(stream, "/healthz", &[]).await;
    assert_eq!((h.status, h.body.as_str()), (200, "ok"));

    // A client that does not trust the certificate is refused.
    let untrusting = client_tls(
        &rcgen::generate_simple_self_signed(vec!["x".into()])
            .unwrap()
            .cert
            .pem(),
    );
    let res = tokio_tungstenite::connect_async_tls_with_config(
        format!("wss://localhost:{}/ws", addr.port()),
        None,
        false,
        Some(Connector::Rustls(untrusting)),
    )
    .await;
    assert!(res.is_err());
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn certificate_and_key_files_are_used() {
    let dir = temp_dir("files");
    // The test makes its own certificate at runtime; nothing is committed.
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_file = dir.join("fullchain.pem");
    let key_file = dir.join("privkey.pem");
    std::fs::write(&cert_file, ck.cert.pem()).unwrap();
    std::fs::write(&key_file, ck.signing_key.serialize_pem()).unwrap();

    // Load through the real config path (file values given as overrides).
    let c = load_config(
        None,
        &[
            ("tls.cert_file".into(), cert_file.display().to_string()),
            ("tls.key_file".into(), key_file.display().to_string()),
        ],
    )
    .unwrap();
    assert!(c.tls.enabled());
    let addr = start(c).await;
    wait_listening(addr).await;
    wss_roundtrip(addr, client_tls(&ck.cert.pem())).await;
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn missing_certificate_file_is_a_clear_error() {
    let mut c = ServerConfig::default();
    c.tls.cert_file = Some("/nonexistent/cert.pem".into());
    c.tls.key_file = Some("/nonexistent/key.pem".into());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let err = run(listener, c, std::future::pending()).await.unwrap_err();
    assert!(err.to_string().contains("cert.pem"), "{err}");
}

// ---------- broken X-Forwarded-For from a trusted proxy ----------

/// Send raw request bytes and return the status code.
async fn raw_status(addr: SocketAddr, request: &[u8]) -> u16 {
    let mut tcp = TcpStream::connect(addr).await.unwrap();
    tcp.write_all(request).await.unwrap();
    let mut raw = Vec::new();
    tcp.read_to_end(&mut raw).await.ok();
    String::from_utf8_lossy(&raw)
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn xff_request(header_value: &[u8]) -> Vec<u8> {
    let mut r =
        b"GET /altcha/challenge HTTP/1.1\r\nHost: x\r\nConnection: close\r\nX-Forwarded-For: "
            .to_vec();
    r.extend_from_slice(header_value);
    r.extend_from_slice(b"\r\n\r\n");
    r
}

#[tokio::test]
async fn broken_forwarded_for_never_falls_back_to_the_proxy() {
    let mut c = ServerConfig::default();
    c.network.trusted_proxies = nets(&["127.0.0.0/8"]);
    c.network.deny = nets(&["203.0.113.9/32"]);
    let addr = start(c).await;
    // The two headers that got past the deny list before the fix.
    assert_eq!(
        raw_status(addr, &xff_request(b"\xff, 203.0.113.9")).await,
        400
    );
    assert_eq!(
        raw_status(addr, &xff_request(b"203.0.113.9:5555")).await,
        403
    );
    // Other broken forms: refused, never guessed.
    assert_eq!(raw_status(addr, &xff_request(b"")).await, 400);
    assert_eq!(raw_status(addr, &xff_request(b"unknown")).await, 400);
    assert_eq!(raw_status(addr, &xff_request(b"1.2.3.4, ")).await, 400);
    assert_eq!(raw_status(addr, &xff_request(b"[2001:db8::1")).await, 400);
    // Bracketed IPv6 with port is understood.
    assert_eq!(
        raw_status(addr, &xff_request(b"[2001:db8::1]:443")).await,
        200
    );
    // No header at all: the proxy's own address is the client.
    assert_eq!(
        raw_status(
            addr,
            b"GET /altcha/challenge HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n"
        )
        .await,
        200
    );
}

// ---------- liveness, lifetimes, per-address caps ----------

/// Receiver with a room, plus a joined sender.
async fn paired(addr: SocketAddr) -> (Ws, Ws, String) {
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    send(&mut r, json!({"type":"create_room"})).await;
    let code = recv(&mut r).await["code"].as_str().unwrap().to_string();
    let mut s = connect(addr).await;
    hello(&mut s, "sender").await;
    send(&mut s, json!({"type":"join_room","code":code})).await;
    assert_eq!(recv(&mut s).await, json!({"type":"joined"}));
    assert_eq!(recv(&mut r).await["type"], "peer_joined");
    (r, s, code)
}

#[tokio::test]
async fn silent_connection_is_closed_by_ping_timeout() {
    let mut c = ServerConfig::default();
    c.limits.ping_interval = Duration::from_secs(1);
    c.limits.pong_timeout = Duration::from_secs(1);
    let addr = start(c).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    send(&mut r, json!({"type":"create_room"})).await;
    recv(&mut r).await;
    // The client does not read, so it answers no ping. Wait past interval + timeout.
    tokio::time::sleep(Duration::from_secs(4)).await;
    expect_closed(&mut r).await;
}

#[tokio::test]
async fn answering_connection_stays_open_past_the_ping_timeout() {
    let mut c = ServerConfig::default();
    c.limits.ping_interval = Duration::from_secs(1);
    c.limits.pong_timeout = Duration::from_secs(1);
    let addr = start(c).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    send(&mut r, json!({"type":"create_room"})).await;
    recv(&mut r).await;
    // Keep reading (the library answers pings) for 3 seconds.
    let until = tokio::time::Instant::now() + Duration::from_secs(3);
    while tokio::time::Instant::now() < until {
        let _ = tokio::time::timeout(Duration::from_millis(200), r.next()).await;
    }
    send(&mut r, json!({"type":"nonsense"})).await;
    assert_eq!(error_code(&recv(&mut r).await), "bad_message");
}

/// Read frames for `dur` (the library answers pings meanwhile) and return the text
/// messages seen. A closed connection shows up as a `{"type":"__closed"}` entry.
async fn pump(ws: &mut Ws, dur: Duration) -> Vec<Value> {
    let until = tokio::time::Instant::now() + dur;
    let mut out = Vec::new();
    loop {
        let left = until.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            break;
        }
        match tokio::time::timeout(left, ws.next()).await {
            Err(_) => break,
            Ok(Some(Ok(Message::Text(t)))) => out.push(serde_json::from_str(t.as_str()).unwrap()),
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) | Ok(Some(Err(_))) => {
                out.push(json!({"type":"__closed"}));
                break;
            }
            Ok(Some(Ok(_))) => {}
        }
    }
    out
}

fn types(msgs: &[Value]) -> Vec<&str> {
    msgs.iter().map(|m| m["type"].as_str().unwrap()).collect()
}

/// Tiny timeouts: ping every second, 1 s to answer, 1 s of relay silence is "idle".
fn tiny_liveness() -> ServerConfig {
    let mut c = ServerConfig::default();
    c.limits.ping_interval = Duration::from_secs(1);
    c.limits.pong_timeout = Duration::from_secs(1);
    c.limits.paired_idle = Duration::from_secs(1);
    c
}

#[tokio::test]
async fn quiet_paired_room_stays_open_while_both_sides_answer_pings() {
    let addr = start(tiny_liveness()).await;
    let (mut r, mut s, _) = paired(addr).await;
    // 5 s without any relayed message: far past paired_idle and past interval + timeout.
    let (rm, sm) = tokio::join!(
        pump(&mut r, Duration::from_secs(5)),
        pump(&mut s, Duration::from_secs(5))
    );
    assert!(rm.is_empty(), "receiver saw {:?}", types(&rm));
    assert!(sm.is_empty(), "sender saw {:?}", types(&sm));
    // The room still works.
    send(&mut s, json!({"type":"control","cmd":"stop"})).await;
    let got = pump(&mut r, Duration::from_millis(700)).await;
    assert_eq!(types(&got), ["control"]);
}

#[tokio::test]
async fn paired_side_that_stops_answering_pings_is_closed_and_peer_told() {
    let addr = start(tiny_liveness()).await;
    let (mut r, s, _) = paired(addr).await;
    // The sender never reads, so it never answers a ping. The receiver does.
    let got = pump(&mut r, Duration::from_secs(6)).await;
    assert_eq!(types(&got), ["peer_left"], "receiver should be told once");
    // The receiver stays connected and in a working state.
    send(&mut r, json!({"type":"nonsense"})).await;
    let got = pump(&mut r, Duration::from_millis(500)).await;
    assert_eq!(error_code(&got[0]), "bad_message");
    drop(s);
}

#[tokio::test]
async fn receiver_that_stops_answering_pings_ends_the_room() {
    let addr = start(tiny_liveness()).await;
    let (r, mut s, _) = paired(addr).await;
    let got = pump(&mut s, Duration::from_secs(6)).await;
    assert_eq!(types(&got), ["peer_left"]);
    drop(r);
}

#[tokio::test]
async fn busy_paired_room_is_not_closed_for_idleness() {
    let mut c = ServerConfig::default();
    c.limits.paired_idle = Duration::from_secs(2);
    let addr = start(c).await;
    let (mut r, mut s, _) = paired(addr).await;
    for _ in 0..4 {
        tokio::time::sleep(Duration::from_millis(800)).await;
        send(&mut s, json!({"type":"control","cmd":"stop"})).await;
        assert_eq!(recv(&mut r).await["type"], "control");
    }
}

#[tokio::test]
async fn room_has_a_maximum_lifetime() {
    let mut c = ServerConfig::default();
    c.limits.max_room_lifetime = Duration::from_secs(1);
    let addr = start(c).await;
    let (mut r, mut s, _) = paired(addr).await;
    // Busy or not, the room ends.
    for _ in 0..3 {
        tokio::time::sleep(Duration::from_millis(600)).await;
        let _ = s
            .send(Message::text(
                json!({"type":"control","cmd":"stop"}).to_string(),
            ))
            .await;
    }
    expect_closed(&mut r).await;
    drop(s);
}

#[tokio::test]
async fn rooms_per_address_are_capped_and_freed() {
    let mut c = ServerConfig::default();
    c.limits.max_rooms_per_ip = 1;
    let addr = start(c).await;
    let mut a = connect(addr).await;
    hello(&mut a, "receiver").await;
    send(&mut a, json!({"type":"create_room"})).await;
    assert_eq!(recv(&mut a).await["type"], "room_created");
    let mut b = connect(addr).await;
    hello(&mut b, "receiver").await;
    send(&mut b, json!({"type":"create_room"})).await;
    let e = recv(&mut b).await;
    assert_eq!(error_code(&e), "rate_limited");
    assert!(e["message"].as_str().unwrap().contains("address"));
    // When the first room is gone, the address may make another one.
    drop(a);
    tokio::time::sleep(Duration::from_millis(300)).await;
    send(&mut b, json!({"type":"create_room"})).await;
    assert_eq!(recv(&mut b).await["type"], "room_created");
}

#[tokio::test]
async fn concurrent_connections_per_address_are_capped() {
    let mut c = ServerConfig::default();
    c.limits.max_connections_per_ip = 2;
    let addr = start(c).await;
    let _a = connect(addr).await;
    let _b = connect(addr).await;
    assert_eq!(connect_with(addr, &[]).await.err(), Some(429));
}

// ---------- per-connection budgets and spam ----------

#[tokio::test]
async fn message_flood_closes_the_connection() {
    let mut c = ServerConfig::default();
    c.limits.msg_burst = 5;
    c.limits.msg_rate_per_sec = 1;
    let addr = start(c).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    for _ in 0..8 {
        let _ = r
            .send(Message::text(json!({"type":"nonsense"}).to_string()))
            .await;
    }
    // Some bad_message answers, then rate_limited and a close.
    let mut saw_limit = false;
    for _ in 0..12 {
        match tokio::time::timeout(Duration::from_secs(5), r.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                if t.as_str().contains("rate_limited") {
                    saw_limit = true;
                }
            }
            _ => break,
        }
    }
    assert!(saw_limit);
    expect_closed(&mut r).await;
}

#[tokio::test]
async fn flooding_leads_to_a_ban_when_the_threshold_is_reached() {
    let mut c = ServerConfig::default();
    c.limits.msg_burst = 3;
    c.limits.msg_rate_per_sec = 1;
    c.limits.ban_after_failed_joins = 1;
    let addr = start(c).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    for _ in 0..6 {
        let _ = r
            .send(Message::text(
                json!({"type":"hello","proto_version":2,"role":"receiver"}).to_string(),
            ))
            .await;
    }
    expect_closed(&mut r).await;
    assert_eq!(connect_with(addr, &[]).await.err(), Some(403));
}

#[tokio::test]
async fn byte_budget_stops_big_messages() {
    let mut c = ServerConfig::default();
    c.limits.bytes_burst = c.max_message_bytes as u32; // smallest allowed burst
    c.limits.bytes_per_sec = 1;
    let addr = start(c).await;
    let (mut r, mut s, _) = paired(addr).await;
    let big = "x".repeat(40 * 1024);
    send(&mut s, json!({"type":"offer","sdp":big})).await;
    assert_eq!(recv(&mut r).await["type"], "offer");
    send(&mut s, json!({"type":"offer","sdp":big})).await; // 80 KiB > 64 KiB burst
    assert_eq!(error_code(&recv(&mut s).await), "rate_limited");
    expect_closed(&mut s).await;
}

#[tokio::test]
async fn room_relay_volume_is_capped() {
    let mut c = ServerConfig::default();
    c.limits.max_room_relay_bytes = 1000;
    let addr = start(c).await;
    let (mut r, mut s, _) = paired(addr).await;
    let sdp = "y".repeat(600);
    send(&mut s, json!({"type":"offer","sdp":sdp})).await;
    assert_eq!(recv(&mut r).await["type"], "offer");
    send(&mut s, json!({"type":"offer","sdp":sdp})).await;
    assert_eq!(error_code(&recv(&mut s).await), "rate_limited");
    expect_closed(&mut s).await;
}

#[tokio::test]
async fn bad_message_spam_closes_the_connection() {
    let mut c = ServerConfig::default();
    c.limits.max_bad_messages = 3;
    let addr = start(c).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    for _ in 0..3 {
        send(&mut r, json!({"type":"nonsense"})).await;
        assert_eq!(error_code(&recv(&mut r).await), "bad_message");
    }
    assert_eq!(error_code(&recv(&mut r).await), "rate_limited");
    expect_closed(&mut r).await;
}

#[tokio::test]
async fn wrong_role_spam_counts_toward_a_ban() {
    let mut c = ServerConfig::default();
    c.limits.ban_after_failed_joins = 2;
    let addr = start(c).await;
    let mut s = connect(addr).await;
    hello(&mut s, "sender").await;
    send(&mut s, json!({"type":"create_room"})).await;
    assert_eq!(error_code(&recv(&mut s).await), "wrong_role");
    send(&mut s, json!({"type":"create_room"})).await;
    assert_eq!(error_code(&recv(&mut s).await), "wrong_role");
    assert_eq!(error_code(&recv(&mut s).await), "banned");
    expect_closed(&mut s).await;
    assert_eq!(connect_with(addr, &[]).await.err(), Some(403));
}

// ---------- startup checks ----------

#[tokio::test]
async fn unreadable_key_file_gives_a_helpful_error() {
    let dir = temp_dir("nokey");
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_file = dir.join("fullchain.pem");
    std::fs::write(&cert_file, ck.cert.pem()).unwrap();
    let mut c = ServerConfig::default();
    c.tls.cert_file = Some(cert_file);
    c.tls.key_file = Some(dir.join("missing-privkey.pem"));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let err = run(listener, c, std::future::pending()).await.unwrap_err();
    let text = err.to_string();
    assert!(text.contains("missing-privkey.pem"), "{text}");
    assert!(text.contains("tls.key_file"), "{text}");
    std::fs::remove_dir_all(dir).ok();
}

// ---------- control frames are not free ----------

#[tokio::test]
async fn ping_flood_closes_the_connection() {
    let mut c = ServerConfig::default();
    c.limits.msg_burst = 5;
    c.limits.msg_rate_per_sec = 1;
    let addr = start(c).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    for _ in 0..12 {
        if r.send(Message::Ping(Vec::new().into())).await.is_err() {
            break;
        }
    }
    expect_closed(&mut r).await;
}

#[tokio::test]
async fn pong_flood_counts_toward_a_ban() {
    let mut c = ServerConfig::default();
    c.limits.msg_burst = 3;
    c.limits.msg_rate_per_sec = 1;
    c.limits.ban_after_failed_joins = 1;
    let addr = start(c).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    for _ in 0..10 {
        if r.send(Message::Pong(Vec::new().into())).await.is_err() {
            break;
        }
    }
    expect_closed(&mut r).await;
    assert_eq!(connect_with(addr, &[]).await.err(), Some(403));
}

// ---------- IPv4-mapped networks, health check behind a proxy ----------

#[tokio::test]
async fn mapped_ipv6_networks_match_and_healthz_is_not_a_bypass_for_proxied_clients() {
    let mut c = ServerConfig::default();
    // All three written as IPv4-mapped IPv6; they must act like the IPv4 networks.
    c.network.trusted_proxies = nets(&["::ffff:127.0.0.1/128"]);
    c.network.deny = nets(&["::ffff:203.0.113.0/120"]);
    let addr = start(c).await;
    // A denied client behind the trusted proxy is refused, even on /healthz.
    let denied = [("X-Forwarded-For", "203.0.113.9")];
    assert_eq!(
        http_get(addr, "/altcha/challenge", &denied).await.status,
        403
    );
    assert_eq!(http_get(addr, "/healthz", &denied).await.status, 403);
    // Another client behind the proxy is fine.
    let other = [("X-Forwarded-For", "198.51.100.1")];
    assert_eq!(http_get(addr, "/healthz", &other).await.status, 200);
    // The proxy's own health check (no forwarded header) works.
    assert_eq!(http_get(addr, "/healthz", &[]).await.status, 200);
}

#[tokio::test]
async fn healthz_bypass_needs_no_forwarded_header_even_with_an_allow_list() {
    let mut c = ServerConfig::default();
    c.network.allow = nets(&["10.0.0.0/8"]);
    c.network.trusted_proxies = nets(&["127.0.0.1/32"]);
    let addr = start(c).await;
    assert_eq!(http_get(addr, "/healthz", &[]).await.status, 200);
    let outsider = [("X-Forwarded-For", "198.51.100.1")];
    assert_eq!(http_get(addr, "/healthz", &outsider).await.status, 403);
    let insider = [("X-Forwarded-For", "10.1.2.3")];
    assert_eq!(http_get(addr, "/healthz", &insider).await.status, 200);
}

// ---------- slow HTTP clients ----------

/// Connect, send `bytes`, then nothing; returns how long until the server closed.
async fn time_until_closed(addr: SocketAddr, bytes: &[u8]) -> Duration {
    let mut tcp = TcpStream::connect(addr).await.unwrap();
    tcp.write_all(bytes).await.unwrap();
    let t0 = std::time::Instant::now();
    let mut raw = Vec::new();
    tokio::time::timeout(Duration::from_secs(30), tcp.read_to_end(&mut raw))
        .await
        .expect("connection still open after 30 s")
        .ok();
    t0.elapsed()
}

#[tokio::test]
async fn silent_and_slow_http_clients_are_closed() {
    // The limit is 15 s (UNCERTAIN placeholder in http_deadline.rs); both checks run together.
    let addr = start(ServerConfig::default()).await;
    let (silent, partial) = tokio::join!(
        time_until_closed(addr, b""),
        time_until_closed(addr, b"GET /healthz HTTP/1.1\r\nHost: x\r\n"),
    );
    for d in [silent, partial] {
        assert!(d >= Duration::from_secs(10), "closed too early: {d:?}");
    }
}
