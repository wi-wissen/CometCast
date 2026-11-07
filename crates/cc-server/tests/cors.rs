// SPDX-License-Identifier: GPL-3.0-or-later
//! `allowed_origins`: CORS on the HTTP endpoints and the `Origin` check on `/ws`.
//! Real server, real sockets.

use std::net::SocketAddr;
use std::time::Duration;

use cc_server::{Mode, ServerConfig, load_config, run};
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{self, Message};

const PAGE: &str = "https://cometcast.de";

async fn start(config: ServerConfig) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(run(listener, config, std::future::pending()));
    addr
}

fn with_origins(list: &[&str]) -> ServerConfig {
    ServerConfig {
        allowed_origins: list.iter().map(|s| s.to_string()).collect(),
        ..ServerConfig::default()
    }
}

struct Http {
    status: u16,
    headers: String,
}

impl Http {
    /// Value of a header (names are compared in lowercase), `None` when absent.
    fn header(&self, name: &str) -> Option<String> {
        self.headers.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim() == name).then(|| v.trim().to_string())
        })
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
    let text = String::from_utf8_lossy(&raw).to_ascii_lowercase();
    let head = text
        .split_once("\r\n\r\n")
        .map_or(text.as_str(), |(h, _)| h);
    Http {
        status: head
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        headers: head.to_string(),
    }
}

async fn ws_connect(addr: SocketAddr, origin: Option<&str>) -> Result<(), u16> {
    let mut req = format!("ws://{addr}/ws").into_client_request().unwrap();
    if let Some(o) = origin {
        req.headers_mut().insert("origin", o.parse().unwrap());
    }
    match connect_async(req).await {
        Ok((mut ws, _)) => {
            ws.send(Message::text(
                r#"{"type":"hello","proto_version":2,"role":"receiver"}"#,
            ))
            .await
            .unwrap();
            let m = tokio::time::timeout(Duration::from_secs(5), ws.next())
                .await
                .unwrap();
            assert!(
                matches!(m, Some(Ok(Message::Text(_)))),
                "hello answer: {m:?}"
            );
            Ok(())
        }
        Err(tungstenite::Error::Http(resp)) => Err(resp.status().as_u16()),
        Err(e) => panic!("unexpected connect error: {e}"),
    }
}

#[tokio::test]
async fn default_sends_no_cors_headers_and_accepts_any_ws_origin() {
    let addr = start(ServerConfig::default()).await;
    let r = request(addr, "GET", "/altcha/challenge", &[("Origin", PAGE)]).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.header("access-control-allow-origin"), None);
    assert_eq!(r.header("vary"), None);
    let r = request(
        addr,
        "OPTIONS",
        "/altcha/challenge",
        &[("Origin", PAGE), ("Access-Control-Request-Method", "GET")],
    )
    .await;
    assert_eq!(r.status, 405, "no preflight handling without a list");
    assert_eq!(
        ws_connect(addr, Some("https://anything.example")).await,
        Ok(())
    );
}

#[tokio::test]
async fn listed_origin_gets_cors_headers_on_the_three_endpoints() {
    let addr = start(with_origins(&[PAGE])).await;
    for path in ["/altcha/challenge", "/healthz"] {
        let r = request(addr, "GET", path, &[("Origin", PAGE)]).await;
        assert_eq!(r.status, 200, "{path}");
        assert_eq!(
            r.header("access-control-allow-origin").as_deref(),
            Some(PAGE),
            "{path}"
        );
        assert_eq!(r.header("vary").as_deref(), Some("origin"), "{path}");
        assert_eq!(r.header("access-control-allow-credentials"), None, "{path}");
    }
    // /ice-servers: 404 without TURN, and the 404 is still readable for the page.
    let r = request(addr, "GET", "/ice-servers", &[("Origin", PAGE)]).await;
    assert_eq!(r.status, 404);
    assert_eq!(
        r.header("access-control-allow-origin").as_deref(),
        Some(PAGE)
    );
    // Default port is the same origin.
    let r = request(
        addr,
        "GET",
        "/healthz",
        &[("Origin", "https://cometcast.de:443")],
    )
    .await;
    assert_eq!(
        r.header("access-control-allow-origin").as_deref(),
        Some("https://cometcast.de:443")
    );
}

#[tokio::test]
async fn unlisted_origin_gets_no_allow_header() {
    let addr = start(with_origins(&[PAGE])).await;
    for o in [
        "https://evil.example",
        "http://cometcast.de",
        "https://cometcast.de:8443",
        "null",
    ] {
        let r = request(addr, "GET", "/altcha/challenge", &[("Origin", o)]).await;
        assert_eq!(r.header("access-control-allow-origin"), None, "{o}");
        assert_eq!(r.header("vary").as_deref(), Some("origin"), "{o}");
    }
    // No Origin header (curl, native client): works, Vary still set for caches.
    let r = request(addr, "GET", "/altcha/challenge", &[]).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.header("access-control-allow-origin"), None);
    assert_eq!(r.header("vary").as_deref(), Some("origin"));
}

#[tokio::test]
async fn preflight_allowed_and_refused() {
    let addr = start(with_origins(&[PAGE])).await;
    let r = request(
        addr,
        "OPTIONS",
        "/ice-servers",
        &[
            ("Origin", PAGE),
            ("Access-Control-Request-Method", "GET"),
            ("Access-Control-Request-Headers", "cache-control"),
        ],
    )
    .await;
    assert_eq!(r.status, 204);
    assert_eq!(
        r.header("access-control-allow-origin").as_deref(),
        Some(PAGE)
    );
    assert_eq!(
        r.header("access-control-allow-methods").as_deref(),
        Some("get, head, options")
    );
    assert_eq!(
        r.header("access-control-allow-headers").as_deref(),
        Some("cache-control")
    );
    assert_eq!(r.header("access-control-max-age").as_deref(), Some("600"));
    assert_eq!(r.header("vary").as_deref(), Some("origin"));
    assert_eq!(r.header("access-control-allow-credentials"), None);

    let r = request(
        addr,
        "OPTIONS",
        "/ice-servers",
        &[
            ("Origin", "https://evil.example"),
            ("Access-Control-Request-Method", "GET"),
        ],
    )
    .await;
    assert_eq!(r.status, 403);
    assert_eq!(r.header("access-control-allow-origin"), None);
    assert_eq!(r.header("vary").as_deref(), Some("origin"));
}

#[tokio::test]
async fn retry_after_is_exposed_to_allowed_origins_only() {
    let mut c = with_origins(&[PAGE]);
    c.limits.http_per_min = 1;
    let addr = start(c).await;
    let _ = request(addr, "GET", "/altcha/challenge", &[("Origin", PAGE)]).await;
    let r = request(addr, "GET", "/altcha/challenge", &[("Origin", PAGE)]).await;
    assert_eq!(r.status, 429);
    assert!(r.header("retry-after").is_some());
    assert_eq!(
        r.header("access-control-expose-headers").as_deref(),
        Some("retry-after"),
        "the page must be able to read Retry-After of a refusal"
    );
    assert_eq!(
        r.header("access-control-allow-origin").as_deref(),
        Some(PAGE)
    );
    let r = request(
        addr,
        "GET",
        "/altcha/challenge",
        &[("Origin", "https://evil.example")],
    )
    .await;
    assert_eq!(r.header("access-control-expose-headers"), None);
}

fn pna_headers(origin: &'static str) -> [(&'static str, &'static str); 4] {
    [
        ("Origin", origin),
        ("Access-Control-Request-Method", "GET"),
        ("Access-Control-Request-Private-Network", "true"),
        ("Access-Control-Request-Headers", "cache-control"),
    ]
}

#[tokio::test]
async fn private_network_access_only_for_allowed_origin_and_private_relay() {
    // Internal mode: answered for the listed origin, not for another one, not without the request header.
    let addr = start(with_origins(&[PAGE])).await;
    let r = request(addr, "OPTIONS", "/ice-servers", &pna_headers(PAGE)).await;
    assert_eq!(r.status, 204);
    assert_eq!(
        r.header("access-control-allow-private-network").as_deref(),
        Some("true")
    );
    let r = request(
        addr,
        "OPTIONS",
        "/ice-servers",
        &pna_headers("https://evil.example"),
    )
    .await;
    assert_eq!(r.status, 403);
    assert_eq!(r.header("access-control-allow-private-network"), None);
    let r = request(
        addr,
        "OPTIONS",
        "/ice-servers",
        &[("Origin", PAGE), ("Access-Control-Request-Method", "GET")],
    )
    .await;
    assert_eq!(r.header("access-control-allow-private-network"), None);

    // Open mode on a public listen address: never. On a loopback/private one: yes.
    let mut open = ServerConfig::for_mode(Mode::Open);
    open.allowed_origins = vec![PAGE.into()];
    open.listen_addr = "203.0.113.5:8443".parse().unwrap();
    let addr = start(open.clone()).await;
    let r = request(addr, "OPTIONS", "/ice-servers", &pna_headers(PAGE)).await;
    assert_eq!(r.status, 204);
    assert_eq!(r.header("access-control-allow-private-network"), None);
    open.listen_addr = "192.168.1.5:8443".parse().unwrap();
    let addr = start(open).await;
    let r = request(addr, "OPTIONS", "/ice-servers", &pna_headers(PAGE)).await;
    assert_eq!(
        r.header("access-control-allow-private-network").as_deref(),
        Some("true")
    );
}

#[tokio::test]
async fn cors_does_not_cover_static_files_or_ws_upgrade_path() {
    let dir = std::env::temp_dir().join(format!("cc-cors-web-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("index.html"), "x").unwrap();
    let mut c = with_origins(&[PAGE]);
    c.web_dir = Some(dir);
    let addr = start(c).await;
    let r = request(addr, "GET", "/index.html", &[("Origin", PAGE)]).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.header("access-control-allow-origin"), None);
}

#[tokio::test]
async fn ws_origin_check_when_configured() {
    let addr = start(with_origins(&[PAGE, "http://localhost:5173"])).await;
    assert_eq!(ws_connect(addr, Some(PAGE)).await, Ok(()));
    assert_eq!(
        ws_connect(addr, Some("http://localhost:5173")).await,
        Ok(())
    );
    assert_eq!(
        ws_connect(addr, Some("https://evil.example")).await,
        Err(403)
    );
    assert_eq!(ws_connect(addr, Some("null")).await, Err(403));
    assert_eq!(
        ws_connect(addr, Some("https://cometcast.de.evil.example")).await,
        Err(403)
    );
    // Not a browser: no Origin header.
    assert_eq!(ws_connect(addr, None).await, Ok(()));
}

#[tokio::test]
async fn wildcard_accepts_any_origin_in_internal_mode_and_is_refused_in_open_mode() {
    let addr = start(with_origins(&["*"])).await;
    let r = request(
        addr,
        "GET",
        "/healthz",
        &[("Origin", "https://anything.example")],
    )
    .await;
    assert_eq!(
        r.header("access-control-allow-origin").as_deref(),
        Some("*")
    );
    assert_eq!(
        ws_connect(addr, Some("https://anything.example")).await,
        Ok(())
    );

    let mut open = ServerConfig::for_mode(Mode::Open);
    open.allowed_origins = vec!["*".into()];
    let e = open.validate().unwrap_err().to_string();
    assert!(e.contains("open mode"), "{e}");
}

#[test]
fn config_sources_and_validation() {
    // File text, comma separated env/CLI form, and bad entries.
    let c = load_config(
        None,
        &[(
            "allowed_origins".into(),
            "https://a.example, http://localhost:5173".into(),
        )],
    )
    .unwrap();
    assert_eq!(
        c.allowed_origins,
        vec!["https://a.example", "http://localhost:5173"]
    );
    let c = load_config(
        None,
        &[("allowed_origins".into(), r#"["https://a.example"]"#.into())],
    )
    .unwrap();
    assert_eq!(c.allowed_origins, vec!["https://a.example"]);
    assert!(ServerConfig::default().allowed_origins.is_empty());
    for bad in [
        "https://a.example/",
        "a.example",
        "https://*.a.example",
        "ftp://a.example",
    ] {
        let e = load_config(None, &[("allowed_origins".into(), bad.into())])
            .unwrap_err()
            .to_string();
        assert!(e.contains("allowed_origins"), "{bad}: {e}");
    }
    let e = load_config(
        None,
        &[
            ("mode".into(), "open".into()),
            ("allowed_origins".into(), "*".into()),
        ],
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("open mode"), "{e}");
    assert!(
        load_config(
            None,
            &[
                ("mode".into(), "open".into()),
                ("allowed_origins".into(), "https://cometcast.de".into())
            ]
        )
        .is_ok()
    );
}

#[test]
fn empty_allowed_origins_is_a_start_error_in_every_mode() {
    for mode in ["internal", "open"] {
        for bad in ["", ",", " , ,"] {
            // env / CLI / --set path (string override)
            let e = load_config(
                None,
                &[
                    ("mode".into(), mode.into()),
                    ("allowed_origins".into(), bad.into()),
                ],
            )
            .unwrap_err()
            .to_string();
            assert!(
                e.contains("allowed_origins") && e.contains("no origin"),
                "{mode} {bad:?}: {e}"
            );
            // TOML string in a config file names the file
            let path = std::env::temp_dir().join(format!(
                "cc-empty-origins-{}-{mode}-{}.toml",
                std::process::id(),
                bad.len()
            ));
            std::fs::write(
                &path,
                format!("mode = \"{mode}\"\nallowed_origins = \"{bad}\"\n"),
            )
            .unwrap();
            let e = load_config(Some(&path), &[]).unwrap_err().to_string();
            assert!(
                e.contains(path.to_str().unwrap()) && e.contains("no origin"),
                "{e}"
            );
            let _ = std::fs::remove_file(&path);
        }
    }
    // Absent, and an empty TOML array (the documented default), still mean "not set".
    assert!(load_config(None, &[]).unwrap().allowed_origins.is_empty());
    assert!(
        load_config(None, &[("allowed_origins".into(), "[]".into())])
            .unwrap()
            .allowed_origins
            .is_empty()
    );
}

#[test]
fn env_short_form() {
    let o = cc_server::overrides_from_env(
        [(
            "COMETCAST_ALLOWED_ORIGINS".to_string(),
            "https://a.example".to_string(),
        )]
        .into_iter(),
    );
    assert_eq!(
        o,
        vec![(
            "allowed_origins".to_string(),
            "https://a.example".to_string()
        )]
    );
}
