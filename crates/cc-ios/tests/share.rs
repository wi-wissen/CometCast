// SPDX-License-Identifier: GPL-3.0-or-later
//! The share extension's engine (native-apps.md 21.3) end to end: a real cc-server in this
//! process, a real native receiver (cc-receiver) over loopback UDP, and the engine and the C ABI
//! playing the part of the Swift sheet: join on its own connection (in an Approval-on room the
//! host is asked again), show a link and an image, stop.

// The C ABI tests call the unsafe functions directly.
#![allow(unsafe_code)]

use std::ffi::CString;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cc_ios::share::{self, Item, ShareConfig};
use cc_ios::{ErrorCode, ffi};
use cc_receiver::{ContentEvent, ContentOptions, ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_server::{ServerConfig, run};
use cc_transport::SessionConfig;
use tokio::net::TcpListener;

// The engine is a process-wide singleton: the tests of this file run one at a time.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

#[derive(Default)]
struct Seen {
    requests: Vec<String>,
    links: Vec<String>,
    images: Vec<String>,
    left: bool,
}

struct World {
    _rt: tokio::runtime::Runtime,
    server: String,
    code: String,
    seen: Arc<Mutex<Seen>>,
}

fn world(mode: cc_proto::JoinMode) -> World {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let s = Arc::clone(&seen);
    let (server, code) = rt.block_on(async move {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(run(
            listener,
            ServerConfig::default(),
            std::future::pending(),
        ));
        let mut o = ReceiverOptions::new(&addr);
        o.session = SessionConfig {
            udp_bind: vec!["127.0.0.1:0".into()],
            ..SessionConfig::default()
        };
        o.join_mode = mode;
        o.content = Some(ContentOptions::default());
        let mut r = ReceiverCore::start(o).await.unwrap();
        let mut content = r.take_content_events().unwrap();
        let code = r.room_code().to_string();
        let s2 = Arc::clone(&s);
        tokio::spawn(async move {
            while let Some(e) = content.next().await {
                match e {
                    ContentEvent::LinkRequested { url } => s2.lock().unwrap().links.push(url),
                    ContentEvent::ImageReceived { name, .. } => {
                        s2.lock().unwrap().images.push(name)
                    }
                    _ => {}
                }
            }
        });
        let r = Arc::new(tokio::sync::Mutex::new(r));
        let s3 = Arc::clone(&s);
        tokio::spawn(async move {
            loop {
                let e = r.lock().await.next_event().await;
                match e {
                    Some(ReceiverEvent::JoinRequest { id, name, .. }) => {
                        s3.lock().unwrap().requests.push(name);
                        r.lock().await.decide_join(id, true);
                    }
                    Some(ReceiverEvent::PeerLeft) => s3.lock().unwrap().left = true,
                    Some(_) => {}
                    None => break,
                }
            }
        });
        (addr, code)
    });
    World {
        _rt: rt,
        server,
        code,
        seen,
    }
}

fn wait_for(limit: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + limit;
    while Instant::now() < end {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    false
}

/// Poll through the C ABI until an event of this name arrives; the events before it are kept.
fn event(seen: &mut Vec<serde_json::Value>, name: &str, limit: Duration) -> serde_json::Value {
    let mut found = None;
    let ok = wait_for(limit, || {
        let mut buf = vec![0u8; 4096];
        // SAFETY: `buf` is writable for its length.
        let n = unsafe { ffi::cc_ios_share_poll(buf.as_mut_ptr(), buf.len()) };
        if n > 0 {
            let text = std::str::from_utf8(&buf[..n as usize]).unwrap();
            let v: serde_json::Value = serde_json::from_str(text).unwrap();
            if v["event"] == name {
                found = Some(v);
                return true;
            }
            seen.push(v);
        }
        false
    });
    assert!(ok, "no {name} event; saw {seen:?}");
    found.unwrap()
}

fn start(w: &World, name: &str) {
    let cfg = serde_json::json!({"code": w.code, "server": w.server, "name": name});
    let c = CString::new(cfg.to_string()).unwrap();
    // SAFETY: a NUL-terminated string.
    let r = unsafe { ffi::cc_ios_share_start(c.as_ptr()) };
    assert_eq!(r, 0, "start failed");
}

fn present_json(json: &serde_json::Value) -> i32 {
    let c = CString::new(json.to_string()).unwrap();
    // SAFETY: a NUL-terminated string.
    unsafe { ffi::cc_ios_share_present(c.as_ptr()) }
}

#[test]
fn a_link_and_an_image_are_shown_and_stop_leaves() {
    let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
    let w = world(cc_proto::JoinMode::Open);
    // Nothing runs: refused.
    assert_eq!(
        present_json(&serde_json::json!({"link": "https://example.org"})),
        0
    );
    start(&w, "Ana");
    let mut seen = Vec::new();
    let joined = event(&mut seen, "joined", Duration::from_secs(25));
    assert!(joined.get("room_name").is_some());
    // Open: nobody was asked.
    assert!(w.seen.lock().unwrap().requests.is_empty());

    assert_eq!(
        present_json(&serde_json::json!({"link": "https://example.org/a"})),
        1
    );
    let prepared = event(&mut seen, "content_prepared", Duration::from_secs(25));
    assert_eq!(prepared["kind"], "link");
    assert_eq!(prepared["host"], "example.org");
    event(&mut seen, "content_sent", Duration::from_secs(25));
    assert!(wait_for(Duration::from_secs(10), || {
        w.seen.lock().unwrap().links == ["https://example.org/a"]
    }));

    // One item per session (the sheet shows one item): a second one while the first is shown
    // is refused inline, the session stays.
    assert_eq!(
        present_json(&serde_json::json!({"link": "https://example.org/b"})),
        1
    );
    assert_eq!(
        event(&mut seen, "error", Duration::from_secs(25))["code"],
        "unknown"
    );
    // An image needs its own session: stop (the display sees the sender leave) and start again.
    ffi::cc_ios_share_stop();
    assert!(wait_for(Duration::from_secs(10), || w
        .seen
        .lock()
        .unwrap()
        .left));
    w.seen.lock().unwrap().left = false;
    start(&w, "Ana");
    event(&mut seen, "joined", Duration::from_secs(25));
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("pic.png");
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    bytes.extend_from_slice(&[7; 64]);
    std::fs::write(&png, bytes).unwrap();
    assert_eq!(
        present_json(&serde_json::json!({"file": png.to_string_lossy()})),
        1
    );
    assert_eq!(
        event(&mut seen, "content_prepared", Duration::from_secs(25))["kind"],
        "image"
    );
    event(&mut seen, "content_sent", Duration::from_secs(25));
    assert!(wait_for(Duration::from_secs(10), || {
        w.seen.lock().unwrap().images == ["pic.png"]
    }));

    // A refused item is an event and the session stays.
    assert_eq!(
        present_json(&serde_json::json!({"link": "ftp://example.org"})),
        1
    );
    assert_eq!(
        event(&mut seen, "content_rejected", Duration::from_secs(10))["reason"],
        "bad_link"
    );

    // Stop and close: the display sees the sender leave.
    ffi::cc_ios_share_stop();
    assert!(wait_for(Duration::from_secs(10), || w
        .seen
        .lock()
        .unwrap()
        .left));
    // After stop nothing is accepted and nothing is queued.
    assert_eq!(
        present_json(&serde_json::json!({"link": "https://example.org"})),
        0
    );
    let mut buf = [0u8; 64];
    // SAFETY: `buf` is writable for its length.
    assert_eq!(
        unsafe { ffi::cc_ios_share_poll(buf.as_mut_ptr(), buf.len()) },
        0
    );
    drop(w);
}

#[test]
fn an_approval_room_asks_the_host_again_and_shows_the_check_number() {
    let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
    let w = world(cc_proto::JoinMode::Confirm);
    start(&w, "Ben");
    let mut seen = Vec::new();
    let joined = event(&mut seen, "joined", Duration::from_secs(25));
    assert!(joined.is_object());
    // The sheet was told to wait (check number and countdown) before it was let in.
    let waiting = seen.iter().find(|v| v["event"] == "waiting");
    assert!(
        waiting.is_some_and(|v| v["check"].as_u64().is_some_and(|c| (10..=99).contains(&c))),
        "{seen:?}"
    );
    assert_eq!(w.seen.lock().unwrap().requests, ["Ben"]);
    share::stop();
    drop(w);
}

#[test]
fn a_bad_code_or_relay_is_refused_at_once_and_a_second_start_too() {
    let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
    let c = CString::new(r#"{"code":"bad","server":"127.0.0.1:1"}"#).unwrap();
    // SAFETY: a NUL-terminated string.
    assert_eq!(
        unsafe { ffi::cc_ios_share_start(c.as_ptr()) },
        ErrorCode::InvalidRoomCode as i32
    );
    let c = CString::new(r#"{"code":"K7QM2W","server":""}"#).unwrap();
    // SAFETY: a NUL-terminated string.
    assert_eq!(
        unsafe { ffi::cc_ios_share_start(c.as_ptr()) },
        ErrorCode::InvalidRelay as i32
    );
    let c = CString::new("not json").unwrap();
    // SAFETY: a NUL-terminated string.
    assert_eq!(
        unsafe { ffi::cc_ios_share_start(c.as_ptr()) },
        ErrorCode::InvalidRoomCode as i32
    );
    let ok = CString::new(r#"{"code":"K7QM2W","server":"127.0.0.1:1"}"#).unwrap();
    // SAFETY: a NUL-terminated string.
    assert_eq!(unsafe { ffi::cc_ios_share_start(ok.as_ptr()) }, 0);
    // SAFETY: a NUL-terminated string.
    assert_eq!(
        unsafe { ffi::cc_ios_share_start(ok.as_ptr()) },
        ErrorCode::AlreadyRunning as i32
    );
    // The relay is unreachable: an error event follows.
    let mut seen = Vec::new();
    let e = event(&mut seen, "error", Duration::from_secs(25));
    assert_eq!(e["code"], "connection_lost");
    share::stop();
    let _ = Item::Link(String::new());
    let _ = ShareConfig::default();
}

#[test]
fn a_small_poll_buffer_keeps_the_event() {
    let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
    let ok = CString::new(r#"{"code":"K7QM2W","server":"127.0.0.1:1"}"#).unwrap();
    // SAFETY: a NUL-terminated string.
    assert_eq!(unsafe { ffi::cc_ios_share_start(ok.as_ptr()) }, 0);
    assert!(wait_for(Duration::from_secs(25), || {
        let mut tiny = [0u8; 4];
        // SAFETY: `tiny` is writable for its length.
        unsafe { ffi::cc_ios_share_poll(tiny.as_mut_ptr(), tiny.len()) < 0 }
    }));
    // Still there for a big enough buffer.
    let mut buf = vec![0u8; 4096];
    // SAFETY: `buf` is writable for its length.
    let n = unsafe { ffi::cc_ios_share_poll(buf.as_mut_ptr(), buf.len()) };
    assert!(n > 0);
    assert_eq!(buf[n as usize], 0);
    share::stop();
}
