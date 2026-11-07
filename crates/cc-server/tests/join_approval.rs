// SPDX-License-Identifier: GPL-3.0-or-later
//! Join approval (join modes `open`, `confirm`, `locked`): a real server, real WebSockets.

use std::time::Duration;

use cc_server::{ServerConfig, run};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Ws = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

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

/// Assert that nothing arrives within a short time.
async fn silent(ws: &mut Ws) {
    if let Ok(m) = tokio::time::timeout(Duration::from_millis(300), ws.next()).await {
        panic!("expected silence, got {m:?}");
    }
}

async fn hello(ws: &mut Ws, role: &str) {
    send(ws, json!({"type":"hello","proto_version":2,"role":role})).await;
    assert_eq!(recv(ws).await, json!({"type":"hello","proto_version":2}));
}

fn code_of(v: &Value) -> &str {
    assert_eq!(v["type"], "error", "expected error, got {v}");
    v["code"].as_str().unwrap()
}

/// Receiver with a room in `mode` (`None` = field omitted). Returns (receiver, code).
async fn room(addr: std::net::SocketAddr, mode: Option<&str>) -> (Ws, String) {
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    let mut m = json!({"type":"create_room"});
    if let Some(mode) = mode {
        m["join_mode"] = json!(mode);
    }
    send(&mut r, m).await;
    let created = recv(&mut r).await;
    assert_eq!(created["type"], "room_created", "{created}");
    (r, created["code"].as_str().unwrap().to_string())
}

async fn sender(addr: std::net::SocketAddr) -> Ws {
    let mut s = connect(addr).await;
    hello(&mut s, "sender").await;
    s
}

async fn join(s: &mut Ws, code: &str, name: Option<&str>) {
    let mut m = json!({"type":"join_room","code":code});
    if let Some(n) = name {
        m["name"] = json!(n);
    }
    send(s, m).await;
}

/// Sender joins in `confirm` mode and waits; returns the request id the owner saw.
async fn request(r: &mut Ws, s: &mut Ws, code: &str, name: &str) -> u64 {
    join(s, code, Some(name)).await;
    assert_eq!(recv(s).await["type"], "join_pending");
    let req = recv(r).await;
    assert_eq!(req["type"], "join_request");
    req["id"].as_u64().unwrap()
}

#[tokio::test]
async fn default_mode_is_open_and_room_created_reports_it() {
    let addr = start(ServerConfig::default()).await;
    let (mut r, code) = room(addr, None).await;
    let mut s = sender(addr).await;
    join(&mut s, &code, None).await;
    assert_eq!(recv(&mut s).await, json!({"type":"joined"}));
    assert_eq!(
        recv(&mut r).await,
        json!({"type":"peer_joined","name":"Guest 1"})
    );
    send(&mut s, json!({"type":"offer","sdp":"v=0"})).await;
    assert_eq!(recv(&mut r).await, json!({"type":"offer","sdp":"v=0"}));
}

#[tokio::test]
async fn room_created_names_the_mode() {
    let addr = start(ServerConfig::default()).await;
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    send(&mut r, json!({"type":"create_room","join_mode":"confirm"})).await;
    let created = recv(&mut r).await;
    assert_eq!(created["join_mode"], "confirm");
}

#[tokio::test]
async fn confirm_allow_flow_relays_only_after_approval() {
    let addr = start(ServerConfig::default()).await;
    let (mut r, code) = room(addr, Some("confirm")).await;
    let mut s = sender(addr).await;
    join(&mut s, &code, Some("Ana")).await;
    assert_eq!(
        recv(&mut s).await,
        json!({"type":"join_pending","timeout_secs":60})
    );
    let req = recv(&mut r).await;
    assert_eq!(req["type"], "join_request");
    assert_eq!(req["name"], "Ana");
    assert_eq!(
        req.as_object().unwrap().len(),
        3,
        "only type, id, name: {req}"
    );

    // Nothing from a waiting joiner reaches the owner.
    for m in [
        json!({"type":"offer","sdp":"v=0"}),
        json!({"type":"answer","sdp":"v=0"}),
        json!({"type":"ice_candidate","candidate":"c"}),
        json!({"type":"control","cmd":"stop"}),
    ] {
        send(&mut s, m).await;
        assert_eq!(code_of(&recv(&mut s).await), "join_pending");
    }
    silent(&mut r).await;

    send(
        &mut r,
        json!({"type":"join_decision","id":req["id"],"allow":true}),
    )
    .await;
    assert_eq!(recv(&mut s).await, json!({"type":"joined"}));
    assert_eq!(
        recv(&mut r).await,
        json!({"type":"peer_joined","name":"Ana"})
    );
    send(&mut s, json!({"type":"offer","sdp":"v=0 o"})).await;
    assert_eq!(recv(&mut r).await, json!({"type":"offer","sdp":"v=0 o"}));
    send(&mut r, json!({"type":"answer","sdp":"v=0 a"})).await;
    assert_eq!(recv(&mut s).await, json!({"type":"answer","sdp":"v=0 a"}));
}

#[tokio::test]
async fn confirm_deny_flow_and_retry() {
    let addr = start(ServerConfig::default()).await;
    let (mut r, code) = room(addr, Some("confirm")).await;
    let mut s = sender(addr).await;
    let id = request(&mut r, &mut s, &code, "Ben").await;
    send(
        &mut r,
        json!({"type":"join_decision","id":id,"allow":false}),
    )
    .await;
    assert_eq!(code_of(&recv(&mut s).await), "join_denied");
    silent(&mut r).await;
    // The same connection may ask again.
    let id2 = request(&mut r, &mut s, &code, "Ben").await;
    assert_ne!(id, id2);
    // Deciding twice on one request is an error for the owner.
    send(&mut r, json!({"type":"join_decision","id":id,"allow":true})).await;
    assert_eq!(code_of(&recv(&mut r).await), "no_such_request");
}

#[tokio::test]
async fn deny_all_and_only_one_sender_per_room() {
    let addr = start(ServerConfig::default()).await;
    let (mut r, code) = room(addr, Some("confirm")).await;
    let mut a = sender(addr).await;
    let mut b = sender(addr).await;
    let mut c = sender(addr).await;
    let ida = request(&mut r, &mut a, &code, "A").await;
    let _idb = request(&mut r, &mut b, &code, "B").await;
    let _idc = request(&mut r, &mut c, &code, "C").await;
    // Allowing A makes the room full for B and C.
    send(
        &mut r,
        json!({"type":"join_decision","id":ida,"allow":true}),
    )
    .await;
    assert_eq!(recv(&mut a).await["type"], "joined");
    assert_eq!(recv(&mut r).await["type"], "peer_joined");
    assert_eq!(code_of(&recv(&mut b).await), "room_full");
    assert_eq!(code_of(&recv(&mut c).await), "room_full");
    for _ in 0..2 {
        let m = recv(&mut r).await;
        assert_eq!(m["type"], "join_request_cancelled");
        assert_eq!(m["reason"], "room_full");
    }
    // A join into a room that has a sender is refused at once, also in confirm mode.
    send(&mut r, json!({"type":"remove_peer"})).await;
    assert_eq!(recv(&mut r).await["type"], "peer_left");
    assert_eq!(code_of(&recv(&mut a).await), "removed_by_host");

    // Deny all.
    let mut d = sender(addr).await;
    let mut e = sender(addr).await;
    request(&mut r, &mut d, &code, "D").await;
    request(&mut r, &mut e, &code, "E").await;
    send(&mut r, json!({"type":"join_decision","allow":false})).await;
    assert_eq!(code_of(&recv(&mut d).await), "join_denied");
    assert_eq!(code_of(&recv(&mut e).await), "join_denied");
    // allow without an id is invalid.
    send(&mut r, json!({"type":"join_decision","allow":true})).await;
    assert_eq!(code_of(&recv(&mut r).await), "no_such_request");
}

#[tokio::test]
async fn full_room_is_refused_without_asking() {
    let addr = start(ServerConfig::default()).await;
    let (mut r, code) = room(addr, Some("confirm")).await;
    let mut a = sender(addr).await;
    let id = request(&mut r, &mut a, &code, "A").await;
    send(&mut r, json!({"type":"join_decision","id":id,"allow":true})).await;
    assert_eq!(recv(&mut a).await["type"], "joined");
    assert_eq!(recv(&mut r).await["type"], "peer_joined");
    let mut b = sender(addr).await;
    join(&mut b, &code, None).await;
    assert_eq!(code_of(&recv(&mut b).await), "room_full");
    silent(&mut r).await;
}

#[tokio::test]
async fn locked_refuses_and_modes_change_at_any_time() {
    let addr = start(ServerConfig::default()).await;
    let (mut r, code) = room(addr, Some("locked")).await;
    let mut s = sender(addr).await;
    join(&mut s, &code, None).await;
    assert_eq!(code_of(&recv(&mut s).await), "room_locked");
    silent(&mut r).await;

    send(&mut r, json!({"type":"set_join_mode","join_mode":"open"})).await;
    assert_eq!(
        recv(&mut r).await,
        json!({"type":"join_mode_changed","join_mode":"open"})
    );
    join(&mut s, &code, Some("Cy")).await;
    assert_eq!(recv(&mut s).await["type"], "joined");
    assert_eq!(recv(&mut r).await["name"], "Cy");

    // Locking after the presenter is in does not remove the presenter, but blocks others.
    send(&mut r, json!({"type":"set_join_mode","join_mode":"locked"})).await;
    assert_eq!(recv(&mut r).await["join_mode"], "locked");
    send(&mut s, json!({"type":"offer","sdp":"v=0"})).await;
    assert_eq!(recv(&mut r).await["type"], "offer");
    let mut t = sender(addr).await;
    join(&mut t, &code, None).await;
    // Locked is checked before "full": a locked room answers the same to everybody.
    let c = code_of(&recv(&mut t).await).to_string();
    assert!(c == "room_locked" || c == "room_full", "{c}");
}

#[tokio::test]
async fn locking_drops_waiting_requests() {
    let addr = start(ServerConfig::default()).await;
    let (mut r, code) = room(addr, Some("confirm")).await;
    let mut s = sender(addr).await;
    let id = request(&mut r, &mut s, &code, "Di").await;
    send(&mut r, json!({"type":"set_join_mode","join_mode":"locked"})).await;
    assert_eq!(code_of(&recv(&mut s).await), "room_locked");
    let c = recv(&mut r).await;
    assert_eq!(c["type"], "join_request_cancelled");
    assert_eq!(c["id"], id);
    assert_eq!(c["reason"], "locked");
    assert_eq!(recv(&mut r).await["join_mode"], "locked");
    // The dropped joiner is not a member and relays nothing.
    send(&mut s, json!({"type":"offer","sdp":"v=0"})).await;
    assert_eq!(code_of(&recv(&mut s).await), "not_in_room");
}

#[tokio::test]
async fn joiners_cannot_spoof_the_owner() {
    let addr = start(ServerConfig::default()).await;
    let (mut r, code) = room(addr, Some("confirm")).await;
    let mut s = sender(addr).await;
    let id = request(&mut r, &mut s, &code, "Eve").await;
    // A waiting joiner tries to approve itself, change the mode, or kick.
    for m in [
        json!({"type":"join_decision","id":id,"allow":true}),
        json!({"type":"set_join_mode","join_mode":"open"}),
        json!({"type":"remove_peer"}),
    ] {
        send(&mut s, m).await;
        assert_eq!(code_of(&recv(&mut s).await), "wrong_role");
    }
    silent(&mut r).await;
    // A different receiver connection (not the owner, no room) cannot decide either.
    let mut other = connect(addr).await;
    hello(&mut other, "receiver").await;
    for m in [
        json!({"type":"join_decision","id":id,"allow":true}),
        json!({"type":"set_join_mode","join_mode":"open"}),
        json!({"type":"remove_peer"}),
    ] {
        send(&mut other, m).await;
        assert_eq!(code_of(&recv(&mut other).await), "not_in_room");
    }
    // A second owner with its own room cannot touch the first room's requests: ids are
    // room-local and the decision only applies to the caller's own room.
    let (mut r2, _code2) = room(addr, Some("confirm")).await;
    send(
        &mut r2,
        json!({"type":"join_decision","id":id,"allow":true}),
    )
    .await;
    assert_eq!(code_of(&recv(&mut r2).await), "no_such_request");
    // The real owner still decides, and the joiner was never let in.
    send(
        &mut r,
        json!({"type":"join_decision","id":id,"allow":false}),
    )
    .await;
    assert_eq!(code_of(&recv(&mut s).await), "join_denied");
}

#[tokio::test]
async fn names_are_sanitised_before_the_owner_sees_them() {
    let addr = start(ServerConfig::default()).await;
    let (mut r, code) = room(addr, Some("confirm")).await;
    let long = "x".repeat(500);
    let cases: Vec<(Value, String)> = vec![
        (json!("A\u{202E}b\u{0007}c\u{200B}d\n e"), "Abcd e".into()),
        (json!(long), "x".repeat(40)),
        (json!("\u{202E}\u{2066}"), "Guest 3".into()),
        (json!(""), "Guest 4".into()),
        (
            json!("<img src=x onerror=1>"),
            "<img src=x onerror=1>".into(),
        ),
    ];
    for (i, (name, want)) in cases.into_iter().enumerate() {
        let mut s = sender(addr).await;
        send(&mut s, json!({"type":"join_room","code":code,"name":name})).await;
        assert_eq!(recv(&mut s).await["type"], "join_pending");
        let req = recv(&mut r).await;
        let got = req["name"].as_str().unwrap();
        // "Guest N" counts joiners of the room (including the earlier ones).
        let want = if want.starts_with("Guest") {
            format!("Guest {}", i + 1)
        } else {
            want
        };
        assert_eq!(got, want);
        send(&mut r, json!({"type":"join_decision","allow":false})).await;
        assert_eq!(code_of(&recv(&mut s).await), "join_denied");
    }
    // A name that is not a string is a bad message, not a crash.
    let mut s = sender(addr).await;
    send(&mut s, json!({"type":"join_room","code":code,"name":5})).await;
    assert_eq!(code_of(&recv(&mut s).await), "bad_message");
}

#[tokio::test]
async fn pending_limits_per_room_and_per_address() {
    let mut cfg = ServerConfig::default();
    cfg.limits.max_pending_per_room = 2;
    cfg.limits.max_pending_per_ip = 3;
    let addr = start(cfg).await;
    let (mut r, code) = room(addr, Some("confirm")).await;
    let mut a = sender(addr).await;
    let mut b = sender(addr).await;
    request(&mut r, &mut a, &code, "A").await;
    request(&mut r, &mut b, &code, "B").await;
    let mut c = sender(addr).await;
    join(&mut c, &code, Some("C")).await;
    assert_eq!(code_of(&recv(&mut c).await), "rate_limited");
    silent(&mut r).await;
    // Per address (all clients here share 127.0.0.1): room 2 has room, but the address is at 3.
    let (mut r2, code2) = room(addr, Some("confirm")).await;
    let mut d = sender(addr).await;
    request(&mut r2, &mut d, &code2, "D").await;
    let mut e = sender(addr).await;
    join(&mut e, &code2, Some("E")).await;
    assert_eq!(code_of(&recv(&mut e).await), "rate_limited");
    // Freeing a place helps.
    send(&mut r, json!({"type":"join_decision","allow":false})).await;
    assert_eq!(code_of(&recv(&mut a).await), "join_denied");
    join(&mut e, &code2, Some("E")).await;
    assert_eq!(recv(&mut e).await["type"], "join_pending");
}

#[tokio::test]
async fn pending_times_out() {
    let mut cfg = ServerConfig::default();
    cfg.join_approval_timeout = Duration::from_millis(400);
    let addr = start(cfg).await;
    let (mut r, code) = room(addr, Some("confirm")).await;
    let mut s = sender(addr).await;
    let id = request(&mut r, &mut s, &code, "Slow").await;
    assert_eq!(code_of(&recv(&mut s).await), "approval_timeout");
    let c = recv(&mut r).await;
    assert_eq!(c["type"], "join_request_cancelled");
    assert_eq!(c["id"], id);
    assert_eq!(c["reason"], "timeout");
    // Too late to allow now.
    send(&mut r, json!({"type":"join_decision","id":id,"allow":true})).await;
    assert_eq!(code_of(&recv(&mut r).await), "no_such_request");
    // The connection survives and can ask again.
    join(&mut s, &code, Some("Slow")).await;
    assert_eq!(recv(&mut s).await["type"], "join_pending");
}

#[tokio::test]
async fn repeated_denials_lead_to_a_cooldown() {
    let mut cfg = ServerConfig::default();
    cfg.limits.ban_after_failed_joins = 4;
    cfg.limits.denied_join_weight = 2;
    let addr = start(cfg).await;
    let (mut r, code) = room(addr, Some("confirm")).await;
    let mut s = sender(addr).await;
    request(&mut r, &mut s, &code, "Pest").await;
    send(&mut r, json!({"type":"join_decision","allow":false})).await;
    assert_eq!(code_of(&recv(&mut s).await), "join_denied");
    request(&mut r, &mut s, &code, "Pest").await;
    send(&mut r, json!({"type":"join_decision","allow":false})).await;
    assert_eq!(code_of(&recv(&mut s).await), "join_denied");
    assert_eq!(code_of(&recv(&mut s).await), "banned");
    // The address is banned for new connections (the owner's open connection stays).
    let refused = connect_async(format!("ws://{addr}/ws")).await;
    assert!(refused.is_err(), "banned address must be refused");
    send(&mut r, json!({"type":"set_join_mode","join_mode":"open"})).await;
    assert_eq!(recv(&mut r).await["type"], "join_mode_changed");
}

#[tokio::test]
async fn timeouts_count_toward_the_cooldown_too() {
    let mut cfg = ServerConfig::default();
    cfg.join_approval_timeout = Duration::from_millis(300);
    cfg.limits.ban_after_failed_joins = 2;
    cfg.limits.denied_join_weight = 2;
    let addr = start(cfg).await;
    let (mut r, code) = room(addr, Some("confirm")).await;
    let mut s = sender(addr).await;
    request(&mut r, &mut s, &code, "Idle").await;
    assert_eq!(code_of(&recv(&mut s).await), "approval_timeout");
    assert_eq!(code_of(&recv(&mut s).await), "banned");
}

#[tokio::test]
async fn cancel_and_disconnect_tell_the_owner() {
    let addr = start(ServerConfig::default()).await;
    let (mut r, code) = room(addr, Some("confirm")).await;
    let mut s = sender(addr).await;
    let id = request(&mut r, &mut s, &code, "Cai").await;
    send(&mut s, json!({"type":"cancel_join"})).await;
    let c = recv(&mut r).await;
    assert_eq!(
        (c["type"].as_str(), c["id"].as_u64()),
        (Some("join_request_cancelled"), Some(id))
    );
    assert_eq!(c["reason"], "cancelled");
    // Disconnecting while waiting.
    let mut t = sender(addr).await;
    let id2 = request(&mut r, &mut t, &code, "Dan").await;
    drop(t);
    let c = recv(&mut r).await;
    assert_eq!(c["id"], id2);
    assert_eq!(c["reason"], "left");
    // Owner leaves while somebody waits.
    let mut u = sender(addr).await;
    request(&mut r, &mut u, &code, "Eli").await;
    drop(r);
    assert_eq!(code_of(&recv(&mut u).await), "room_not_found");
}

#[tokio::test]
async fn kick_removes_the_sender_and_a_new_one_must_be_allowed_again() {
    let addr = start(ServerConfig::default()).await;
    let (mut r, code) = room(addr, Some("confirm")).await;
    let mut s = sender(addr).await;
    let id = request(&mut r, &mut s, &code, "Kim").await;
    send(&mut r, json!({"type":"join_decision","id":id,"allow":true})).await;
    assert_eq!(recv(&mut s).await["type"], "joined");
    assert_eq!(recv(&mut r).await["type"], "peer_joined");
    send(&mut r, json!({"type":"remove_peer"})).await;
    assert_eq!(code_of(&recv(&mut s).await), "removed_by_host");
    assert_eq!(recv(&mut r).await["type"], "peer_left");
    // The removed sender is out: it relays nothing and has to ask again.
    send(&mut s, json!({"type":"offer","sdp":"v=0"})).await;
    assert_eq!(code_of(&recv(&mut s).await), "not_in_room");
    silent(&mut r).await;
    let id2 = request(&mut r, &mut s, &code, "Kim").await;
    assert_ne!(id, id2);
    // Nobody to remove: error, no crash.
    send(&mut r, json!({"type":"join_decision","allow":false})).await;
    assert_eq!(code_of(&recv(&mut s).await), "join_denied");
    send(&mut r, json!({"type":"remove_peer"})).await;
    assert_eq!(code_of(&recv(&mut r).await), "not_in_room");
}

#[tokio::test]
async fn operators_can_forbid_open_rooms() {
    let mut cfg = ServerConfig::default();
    cfg.allow_open_rooms = false;
    cfg.default_join_mode = cc_proto::JoinMode::Confirm;
    let addr = start(cfg).await;
    // Explicit open is refused, omitted mode becomes the default (confirm).
    let mut r = connect(addr).await;
    hello(&mut r, "receiver").await;
    send(&mut r, json!({"type":"create_room","join_mode":"open"})).await;
    assert_eq!(code_of(&recv(&mut r).await), "open_rooms_disabled");
    send(&mut r, json!({"type":"create_room"})).await;
    let created = recv(&mut r).await;
    assert_eq!(created["join_mode"], "confirm");
    // Switching to open later is refused as well.
    send(&mut r, json!({"type":"set_join_mode","join_mode":"open"})).await;
    assert_eq!(code_of(&recv(&mut r).await), "open_rooms_disabled");
    send(&mut r, json!({"type":"set_join_mode","join_mode":"locked"})).await;
    assert_eq!(recv(&mut r).await["join_mode"], "locked");
}

#[tokio::test]
async fn server_default_mode_applies_when_the_client_names_none() {
    let mut cfg = ServerConfig::default();
    cfg.default_join_mode = cc_proto::JoinMode::Confirm;
    let addr = start(cfg).await;
    let (mut r, code) = room(addr, None).await;
    let mut s = sender(addr).await;
    join(&mut s, &code, None).await;
    assert_eq!(recv(&mut s).await["type"], "join_pending");
    assert_eq!(recv(&mut r).await["name"], "Guest 1");
}

#[tokio::test]
async fn bad_config_combinations_are_rejected() {
    let mut cfg = ServerConfig::default();
    cfg.allow_open_rooms = false; // default_join_mode is still open
    assert!(cfg.validate().is_err());
    let mut cfg = ServerConfig::default();
    cfg.default_join_mode = cc_proto::JoinMode::Locked;
    assert!(cfg.validate().is_err());
}

#[tokio::test]
async fn invalid_code_and_valid_code_with_bad_name_are_told_apart() {
    let addr = start(ServerConfig::default()).await;
    let (_r, code) = room(addr, None).await;
    let mut s = sender(addr).await;
    send(&mut s, json!({"type":"join_room","code":"zz"})).await;
    assert_eq!(code_of(&recv(&mut s).await), "room_not_found");
    send(
        &mut s,
        json!({"type":"join_room","code":code,"name":{"a":1}}),
    )
    .await;
    assert_eq!(code_of(&recv(&mut s).await), "bad_message");
}
