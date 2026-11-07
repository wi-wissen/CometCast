// SPDX-License-Identifier: GPL-3.0-or-later
//! `[stun]`: a public STUN list at `GET /ice-servers` in both modes, next to (internal) or
//! without (open) TURN credentials. Real server, real sockets.
#![allow(clippy::field_reassign_with_default)] // tests tweak one setting of a default config

use std::net::SocketAddr;
use std::time::Duration;

use cc_server::{Mode, Secret, ServerConfig, TurnConfig, credential_for, load_config, run};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const PAGE: &str = "https://cometcast.de";
const STUN: &str = "stun:relay.example.org:3478";

async fn start(config: ServerConfig) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(run(listener, config, std::future::pending()));
    addr
}

struct Http {
    status: u16,
    head: String,
    body: String,
}

impl Http {
    fn header(&self, name: &str) -> Option<String> {
        self.head.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim() == name).then(|| v.trim().to_string())
        })
    }
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| panic!("{e}: {:?}", self.body))
    }
}

async fn request(addr: SocketAddr, method: &str, path: &str, headers: &[(&str, &str)]) -> Http {
    let mut tcp = TcpStream::connect(addr).await.unwrap();
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    tcp.write_all(req.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    tcp.read_to_end(&mut raw).await.ok();
    let text = String::from_utf8_lossy(&raw).to_string();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    Http {
        status: head
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        head: head.to_ascii_lowercase(),
        body: body.to_string(),
    }
}

async fn get(addr: SocketAddr, headers: &[(&str, &str)]) -> Http {
    request(addr, "GET", "/ice-servers", headers).await
}

fn open_with_stun(urls: &[&str]) -> ServerConfig {
    let mut c = ServerConfig::for_mode(Mode::Open);
    c.stun_urls = urls.iter().map(|s| s.to_string()).collect();
    c
}

fn turn(stun_urls: &[&str]) -> TurnConfig {
    TurnConfig {
        urls: vec!["turn:relay.example.org:3478?transport=udp".into()],
        stun_urls: stun_urls.iter().map(|s| s.to_string()).collect(),
        secret: Secret("test-secret-0123456789".into()),
        ttl: Duration::from_secs(3600),
    }
}

fn overrides(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// ---------- open mode ----------

#[tokio::test]
async fn open_mode_hands_out_stun_without_credentials() {
    let addr = start(open_with_stun(&[STUN, "stun:relay.example.org:3479"])).await;
    // No ALTCHA payload, no Origin: a plain GET works.
    let r = get(addr, &[]).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.header("cache-control").as_deref(), Some("no-store"));
    assert_eq!(
        r.json(),
        json!([{"urls": [STUN, "stun:relay.example.org:3479"]}])
    );
    // Exactly the plain STUN shape on the wire: no username, credential or ttl key at all.
    for key in ["username", "credential", "ttl"] {
        assert!(!r.body.contains(key), "{key} in {}", r.body);
    }
}

#[tokio::test]
async fn open_mode_without_stun_is_still_404() {
    let addr = start(ServerConfig::for_mode(Mode::Open)).await;
    assert_eq!(get(addr, &[]).await.status, 404);
}

#[tokio::test]
async fn open_mode_still_refuses_turn() {
    let mut c = open_with_stun(&[STUN]);
    c.turn = Some(turn(&[]));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let err = run(listener, c, std::future::pending()).await.unwrap_err();
    assert!(err.to_string().contains("internal mode"), "{err}");
    // Same through the file/env loader.
    let err = load_config(
        None,
        &overrides(&[
            ("mode", "open"),
            ("stun.urls", STUN),
            ("turn.urls", "turn:x:3478"),
            ("turn.secret", "abcdefghijklmnopqrstuvwxyz"),
        ]),
    )
    .unwrap_err();
    assert!(err.to_string().contains("internal mode"), "{err}");
}

#[tokio::test]
async fn open_mode_keeps_the_per_address_http_limit() {
    let mut c = open_with_stun(&[STUN]);
    c.limits.ice_per_min = 2;
    let addr = start(c).await;
    assert_eq!(get(addr, &[]).await.status, 200);
    assert_eq!(get(addr, &[]).await.status, 200);
    let r = get(addr, &[]).await;
    assert_eq!(r.status, 429);
    assert!(r.head.contains("retry-after:"));
}

#[tokio::test]
async fn ice_servers_do_not_share_the_challenge_bucket() {
    let mut c = open_with_stun(&[STUN]);
    c.limits.http_per_min = 1;
    c.limits.ice_per_min = 2;
    let addr = start(c).await;
    // Use up the /altcha/challenge budget of this address ...
    let _ = request(addr, "GET", "/altcha/challenge", &[]).await;
    assert_eq!(
        request(addr, "GET", "/altcha/challenge", &[]).await.status,
        429
    );
    // ... the ICE list is still served, from its own bucket, until that one is used up.
    assert_eq!(get(addr, &[]).await.status, 200);
    assert_eq!(get(addr, &[]).await.status, 200);
    assert_eq!(get(addr, &[]).await.status, 429);
}

#[tokio::test]
async fn open_mode_obeys_the_deny_list() {
    let mut c = open_with_stun(&[STUN]);
    c.network.deny = vec!["127.0.0.0/8".parse().unwrap()];
    let addr = start(c).await;
    assert_eq!(get(addr, &[]).await.status, 403);
}

// ---------- CORS ----------

#[tokio::test]
async fn stun_list_has_cors_for_allowed_origins_only() {
    let mut c = open_with_stun(&[STUN]);
    c.allowed_origins = vec![PAGE.into()];
    let addr = start(c).await;
    let r = get(addr, &[("Origin", PAGE)]).await;
    assert_eq!(r.status, 200);
    assert_eq!(
        r.header("access-control-allow-origin").as_deref(),
        Some(PAGE)
    );
    assert_eq!(r.header("vary").as_deref(), Some("origin"));
    assert_eq!(r.header("access-control-allow-credentials"), None);
    assert_eq!(r.json(), json!([{"urls": [STUN]}]));
    // Another page gets no CORS header (the browser then hides the answer).
    let r = get(addr, &[("Origin", "https://evil.example")]).await;
    assert_eq!(r.header("access-control-allow-origin"), None);
    // Preflight for the listed page.
    let r = request(
        addr,
        "OPTIONS",
        "/ice-servers",
        &[("Origin", PAGE), ("Access-Control-Request-Method", "GET")],
    )
    .await;
    assert_eq!(r.status, 204);
    assert_eq!(
        r.header("access-control-allow-origin").as_deref(),
        Some(PAGE)
    );
    // The 429 stays readable for the allowed page too.
    let mut c = open_with_stun(&[STUN]);
    c.allowed_origins = vec![PAGE.into()];
    c.limits.ice_per_min = 1;
    let addr = start(c).await;
    assert_eq!(get(addr, &[("Origin", PAGE)]).await.status, 200);
    let r = get(addr, &[("Origin", PAGE)]).await;
    assert_eq!(r.status, 429);
    assert_eq!(
        r.header("access-control-allow-origin").as_deref(),
        Some(PAGE)
    );
}

// ---------- internal mode ----------

#[tokio::test]
async fn internal_mode_stun_only() {
    let mut c = ServerConfig::default();
    c.stun_urls = vec![STUN.into()];
    let addr = start(c).await;
    let r = get(addr, &[]).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.json(), json!([{"urls": [STUN]}]));
}

#[tokio::test]
async fn internal_mode_turn_and_stun() {
    let mut c = ServerConfig::default();
    c.stun_urls = vec![STUN.into()];
    // The same URL in [turn] stun_urls is not listed twice; another one is added.
    c.turn = Some(turn(&[STUN, "stun:turn.example.org:3478"]));
    let addr = start(c).await;
    let r = get(addr, &[]).await;
    assert_eq!(r.status, 200);
    let list = r.json();
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(
        list[0],
        json!({"urls": [STUN, "stun:turn.example.org:3478"]})
    );
    let t = &list[1];
    assert_eq!(
        t["urls"],
        json!(["turn:relay.example.org:3478?transport=udp"])
    );
    let username = t["username"].as_str().unwrap();
    assert_eq!(
        t["credential"].as_str().unwrap(),
        credential_for("test-secret-0123456789", username)
    );
    assert!(t["ttl"].as_u64().unwrap() >= 3600);
}

#[tokio::test]
async fn internal_mode_turn_only_is_unchanged() {
    let mut c = ServerConfig::default();
    c.turn = Some(turn(&[]));
    let addr = start(c).await;
    let list = get(addr, &[]).await.json();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert!(list[0]["username"].is_string());
}

#[tokio::test]
async fn neither_stun_nor_turn_is_404() {
    let addr = start(ServerConfig::default()).await;
    assert_eq!(get(addr, &[]).await.status, 404);
}

// ---------- validation ----------

fn stun_err(value: &str) -> String {
    load_config(None, &overrides(&[("stun.urls", value)]))
        .expect_err(value)
        .to_string()
}

#[test]
fn good_stun_urls_load_in_both_modes() {
    for mode in ["open", "internal"] {
        let c = load_config(
            None,
            &overrides(&[
                ("mode", mode),
                (
                    "stun.urls",
                    "stun:relay.example.org:3478, stuns:relay.example.org:5349, stun:192.0.2.7:3478, stun:[2001:db8::1]:3478",
                ),
            ]),
        )
        .unwrap();
        assert_eq!(c.stun_urls.len(), 4, "{mode}");
        assert_eq!(c.stun_urls[0], STUN);
    }
    // Port is optional (RFC 7064 default 3478).
    assert!(load_config(None, &overrides(&[("stun.urls", "stun:example.org")])).is_ok());
}

#[test]
fn bad_stun_urls_are_refused() {
    // Wrong schemes: TURN must not sneak in through [stun], nor web URLs.
    for bad in [
        "turn:relay.example.org:3478",
        "turns:relay.example.org:5349",
        "http://relay.example.org",
        "relay.example.org:3478",
        "STUN:relay.example.org:3478",
    ] {
        assert!(stun_err(bad).contains("stun.urls"), "{bad}");
    }
    // Credentials in the URL.
    let e = stun_err("stun:user:pass@relay.example.org:3478");
    assert!(e.contains("credentials"), "{e}");
    assert!(!e.contains("pass"), "credentials must not be echoed: {e}");
    // Not host:port.
    for bad in [
        "stun:",
        "stun::3478",
        "stun:host:0",
        "stun:host:65536",
        "stun:host:port",
        "stun:host:3478?transport=udp",
        "stun:host/path",
        "stun:ho st:3478",
        "stun:-host:3478",
        "stun:[::1:3478",
        "stun:[nonsense]:3478",
        "stun:[::1]x",
    ] {
        assert!(!stun_err(bad).is_empty(), "{bad}");
    }
    // One bad entry spoils the list.
    assert!(stun_err("stun:ok.example:3478,turn:bad:3478").contains("turn:bad:3478"));
    // The same rules apply to [turn] stun_urls.
    let e = load_config(
        None,
        &overrides(&[
            ("turn.urls", "turn:x:3478"),
            ("turn.stun_urls", "http://x"),
            ("turn.secret", "abcdefghijklmnopqrstuvwxyz"),
        ]),
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("turn.stun_urls"), "{e}");
}

#[test]
fn at_most_four_stun_urls() {
    let four = "stun:a:1,stun:b:1,stun:c:1,stun:d:1";
    assert!(load_config(None, &overrides(&[("stun.urls", four)])).is_ok());
    let e = stun_err(&format!("{four},stun:e:1"));
    assert!(e.contains("at most 4"), "{e}");
}

#[test]
fn server_refuses_to_start_with_a_bad_stun_url() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let c = open_with_stun(&["turn:relay.example.org:3478"]);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let err = run(listener, c, std::future::pending()).await.unwrap_err();
        assert!(err.to_string().contains("stun.urls"), "{err}");
    });
}
