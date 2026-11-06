// SPDX-License-Identifier: GPL-3.0-or-later
//! One WebSocket connection: handshake (`hello`), room handling and relaying.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use std::net::IpAddr;
use std::time::Instant as StdInstant;

use axum::Extension;
use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use cc_proto::{ClientMsg, PROTO_VERSION, Role, RoomCode, ServerMsg, error_codes};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::Instant;

use crate::AppState;
use crate::limits::{ConnBudget, ConnSlot, Deny, limit_key};
use crate::net::ClientIp;
use crate::puzzle::PuzzleError;
use crate::rooms::{CreateError, Gone, JoinError, Outgoing, Tx};

/// Source of unique connection numbers.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) async fn ws_handler(
    State(state): State<AppState>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    ws: WebSocketUpgrade,
) -> Response {
    // Refuse before upgrading: banned, too many connections per minute, server full.
    let slot = match state.guard.admit_connection(ip, StdInstant::now()) {
        Ok(slot) => slot,
        Err(deny) => return crate::deny_response(deny),
    };
    let max = state.config.max_message_bytes;
    // Messages or frames bigger than `max` make the connection fail and close.
    ws.max_message_size(max)
        .max_frame_size(max)
        .on_upgrade(move |socket| handle_socket(socket, state, ip, slot))
}

/// Per-connection state.
struct Conn {
    id: u64,
    /// Client address (after proxy handling); limits and bans are counted under it.
    ip: IpAddr,
    tx: Tx,
    role: Option<Role>,
    room: Option<RoomCode>,
    /// Messages and bytes this connection may still send.
    budget: ConnBudget,
    /// Bad messages (garbage, wrong role) seen on this connection.
    bad: u32,
}

enum Step {
    Continue,
    Close,
}

impl Conn {
    fn send(&self, msg: ServerMsg) {
        self.tx.send(Outgoing::Msg(msg));
    }

    fn error(&self, code: &str, message: &str) {
        self.send(ServerMsg::Error {
            code: code.into(),
            message: message.into(),
        });
    }

    /// Send an error and end the connection.
    fn fail(&self, code: &str, message: &str) -> Step {
        self.error(code, message);
        self.tx.send(Outgoing::Close("error"));
        Step::Close
    }
}

async fn handle_socket(socket: WebSocket, state: AppState, ip: IpAddr, _slot: ConnSlot) {
    // `_slot` is kept until this function ends: it counts the open connection.
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx, kill) = Tx::new(state.config.outgoing_queue_len);

    // Writer task: the only place that writes to the socket. Everything else
    // (including other connections relaying to us) pushes into the channel.
    let mut writer = tokio::spawn(async move {
        while let Some(out) = rx.recv().await {
            match out {
                Outgoing::Msg(m) => {
                    let Ok(text) = serde_json::to_string(&m) else {
                        continue;
                    };
                    if sink.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Outgoing::Ping => {
                    if sink.send(Message::Ping(Vec::new().into())).await.is_err() {
                        break;
                    }
                }
                Outgoing::Close(reason) => {
                    let frame = CloseFrame {
                        code: 1000,
                        reason: reason.into(),
                    };
                    let _ = sink.send(Message::Close(Some(frame))).await;
                    break;
                }
            }
        }
    });

    let mut conn = Conn {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        ip,
        tx,
        role: None,
        room: None,
        budget: ConnBudget::new(state.guard.cfg(), StdInstant::now()),
        bad: 0,
    };
    let mut writer_done = false;
    // Killed = our queue overflowed (client too slow); then we just drop the connection.
    let mut killed = false;
    // Deadlines are fixed at connect time, so pings cannot keep a silent client alive.
    let connected = Instant::now();
    let hello_deadline = connected + state.config.hello_timeout;
    let no_room_deadline = connected + state.config.no_room_timeout;

    // Liveness: a timer ticks about once a second. It sends pings, closes connections
    // that went silent, and closes rooms that are too old or have been quiet too long.
    let limits = state.guard.cfg();
    // The tick must be much shorter than the ping interval and the pong timeout.
    let tick = (limits.ping_interval.min(limits.pong_timeout) / 4)
        .clamp(Duration::from_millis(50), Duration::from_secs(1));
    let mut ticker = tokio::time::interval(tick);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_seen = connected;
    let mut last_ping = connected;

    loop {
        let waiting_for_hello = conn.role.is_none();
        let deadline = if waiting_for_hello {
            Some(hello_deadline)
        } else if conn.room.is_none() {
            Some(no_room_deadline)
        } else {
            None
        };
        let next = async {
            match deadline {
                Some(d) => tokio::time::timeout_at(d, stream.next())
                    .await
                    .map_err(|_| ()),
                None => Ok(stream.next().await),
            }
        };
        let received = tokio::select! {
            r = next => r,
            _ = &mut writer => { writer_done = true; break; }
            _ = kill.notified() => { killed = true; break; }
            _ = ticker.tick() => {
                if let Some(reason) = tick_check(&conn, &state, last_seen, &mut last_ping) {
                    conn.tx.send(Outgoing::Close(reason));
                    break;
                }
                continue;
            }
        };
        let msg = match received {
            Err(()) => {
                if waiting_for_hello {
                    conn.fail("bad_hello", "no hello received in time");
                } else {
                    conn.tx.send(Outgoing::Close("no_room_timeout"));
                }
                break;
            }
            // Stream ended or a protocol error (e.g. message too big): just stop.
            Ok(None) | Ok(Some(Err(_))) => break,
            Ok(Some(Ok(m))) => m,
        };
        // Any frame (data, ping, pong) shows that the client is alive.
        last_seen = Instant::now();
        // One connection may not send faster than its budget, whatever it sends.
        let data_len = match &msg {
            Message::Text(t) => Some(t.len()),
            Message::Binary(b) => Some(b.len()),
            _ => None,
        };
        if let Some(len) = data_len
            && !conn.budget.allow(len, StdInstant::now())
        {
            let banned = state.guard.record_failure(conn.ip, StdInstant::now());
            if banned.is_some() {
                conn.fail(error_codes::BANNED, "sending too fast; banned for a while");
            } else {
                conn.fail(error_codes::RATE_LIMITED, "sending too fast");
            }
            break;
        }
        let step = match msg {
            Message::Text(text) => handle_text(&mut conn, &state, text.as_str()),
            Message::Binary(_) => {
                if conn.role.is_none() {
                    conn.fail("bad_hello", "first message must be a hello text message")
                } else {
                    strike(
                        &mut conn,
                        &state,
                        "bad_message",
                        "binary messages are not supported",
                    )
                }
            }
            Message::Close(_) => Step::Close,
            // Ping/pong are answered by the library.
            Message::Ping(_) | Message::Pong(_) => Step::Continue,
        };
        if let Step::Close = step {
            break;
        }
    }

    // Cleanup: tell the other side and free the room.
    if let Some(code) = conn.room.take() {
        let peer = state
            .rooms
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .leave(&code, conn.id);
        if let Some(peer) = peer {
            peer.send(Outgoing::Msg(ServerMsg::PeerLeft));
        }
    }
    if killed {
        writer.abort();
    } else if !writer_done {
        // Let queued messages (like a final error) go out, but do not wait forever.
        conn.tx.send(Outgoing::Close("bye"));
        let _ = tokio::time::timeout(Duration::from_secs(2), &mut writer).await;
        writer.abort();
    }
}

/// The checks of the timer. Returns a close reason when the connection must end.
fn tick_check(
    conn: &Conn,
    state: &AppState,
    last_seen: Instant,
    last_ping: &mut Instant,
) -> Option<&'static str> {
    let l = state.guard.cfg();
    let now = Instant::now();
    if now.duration_since(last_seen) > l.ping_interval + l.pong_timeout {
        return Some("ping_timeout");
    }
    if now.duration_since(*last_ping) >= l.ping_interval {
        *last_ping = now;
        conn.tx.send(Outgoing::Ping);
    }
    let code = conn.room.as_ref()?;
    let rooms = lock(state);
    let (created, last_activity) = rooms.times(code)?;
    if now.duration_since(created) > l.max_room_lifetime {
        return Some("room_lifetime");
    }
    // Quiet for too long (nothing relayed, nobody joined). Both sides check this, so
    // whichever tick comes first, the room ends.
    if now.duration_since(last_activity) > l.paired_idle {
        return Some("idle_timeout");
    }
    None
}

/// Answer a bad message and count it: against the address (ban) and against this
/// connection (closed after `max_bad_messages`).
fn strike(conn: &mut Conn, state: &AppState, code: &str, message: &str) -> Step {
    conn.error(code, message);
    conn.bad += 1;
    if state
        .guard
        .record_failure(conn.ip, StdInstant::now())
        .is_some()
    {
        conn.error(
            error_codes::BANNED,
            "too many bad requests; banned for a while",
        );
        conn.tx.send(Outgoing::Close("banned"));
        return Step::Close;
    }
    if conn.bad >= state.guard.cfg().max_bad_messages {
        return conn.fail(error_codes::RATE_LIMITED, "too many bad messages");
    }
    Step::Continue
}

fn handle_text(conn: &mut Conn, state: &AppState, text: &str) -> Step {
    let len = text.len();
    let parsed = serde_json::from_str::<ClientMsg>(text);

    // Step 1: the first message must be a valid hello.
    let Some(role) = conn.role else {
        return match parsed {
            Ok(ClientMsg::Hello {
                proto_version,
                role,
            }) => {
                if proto_version != PROTO_VERSION {
                    return conn.fail(
                        "proto_mismatch",
                        &format!("server speaks protocol version {PROTO_VERSION}"),
                    );
                }
                conn.role = Some(role);
                conn.send(ServerMsg::Hello {
                    proto_version: PROTO_VERSION,
                });
                Step::Continue
            }
            _ => conn.fail("bad_hello", "first message must be a valid hello"),
        };
    };

    let msg = match parsed {
        Ok(m) => m,
        Err(_) => {
            // A join_room whose code is not even a valid code (wrong length or
            // characters) behaves like an unknown code. Valid messages are unaffected.
            if is_join_with_string_code(text) {
                if role == Role::Sender {
                    // Guessing with malformed codes counts like any failed join.
                    if let Some(step) = join_precheck(conn, state) {
                        return step;
                    }
                    return failed_join(conn, state, "room_not_found", "no room with this code");
                } else {
                    return strike(conn, state, "wrong_role", "only a sender can join a room");
                }
            } else {
                return strike(
                    conn,
                    state,
                    "bad_message",
                    "could not understand this message",
                );
            }
        }
    };

    // If our room ended meanwhile (the receiver left), forget it so this
    // connection can create or join another room.
    if let Some(code) = &conn.room
        && !lock(state).is_member(code, conn.id)
    {
        conn.room = None;
    }

    match msg {
        ClientMsg::Hello { .. } => {
            return strike(conn, state, "bad_message", "hello was already sent");
        }
        ClientMsg::CreateRoom { altcha } => {
            if role != Role::Receiver {
                return strike(
                    conn,
                    state,
                    "wrong_role",
                    "only a receiver can create a room",
                );
            } else if conn.room.is_some() {
                return strike(conn, state, "bad_message", "already in a room");
            } else if let Err(deny) = state.guard.check_create(conn.ip, StdInstant::now()) {
                deny_error(conn, deny);
                return after_deny(deny);
            } else {
                // Proof-of-work first (cheap to check), then the room table.
                match state.puzzle.check(altcha.as_deref()) {
                    Ok(()) => {}
                    Err(PuzzleError::Missing) => {
                        conn.error(
                            error_codes::ALTCHA_REQUIRED,
                            "solve the ALTCHA puzzle from /altcha/challenge first",
                        );
                        return Step::Continue;
                    }
                    Err(PuzzleError::Invalid) => {
                        state.guard.note_event(StdInstant::now());
                        conn.error(
                            error_codes::ALTCHA_INVALID,
                            "the ALTCHA solution is wrong, expired or was already used",
                        );
                        return Step::Continue;
                    }
                }
                let created = lock(state).create(
                    conn.id,
                    conn.tx.clone(),
                    limit_key(conn.ip),
                    state.guard.max_rooms(),
                    state.guard.max_rooms_per_ip(),
                );
                match created {
                    Ok((code, cancel)) => {
                        conn.room = Some(code.clone());
                        tokio::spawn(expire_when_idle(state.clone(), code.clone(), cancel));
                        conn.send(ServerMsg::RoomCreated { code });
                    }
                    Err(CreateError::ServerFull) => {
                        conn.error(error_codes::RATE_LIMITED, "the server has too many rooms")
                    }
                    Err(CreateError::OwnerFull) => conn.error(
                        error_codes::RATE_LIMITED,
                        "you have too many rooms open (limit per address)",
                    ),
                }
            }
        }
        ClientMsg::JoinRoom { code } => {
            if role != Role::Sender {
                return strike(conn, state, "wrong_role", "only a sender can join a room");
            } else if conn.room.is_some() {
                return strike(conn, state, "bad_message", "already in a room");
            } else if let Some(step) = join_precheck(conn, state) {
                return step;
            } else {
                let result = lock(state).join(&code, conn.id, conn.tx.clone());
                match result {
                    Ok(receiver) => {
                        conn.room = Some(code);
                        conn.send(ServerMsg::Joined);
                        receiver.send(Outgoing::Msg(ServerMsg::PeerJoined));
                    }
                    Err(JoinError::NotFound) => {
                        return failed_join(
                            conn,
                            state,
                            "room_not_found",
                            "no room with this code",
                        );
                    }
                    Err(JoinError::Full) => {
                        return failed_join(
                            conn,
                            state,
                            "room_full",
                            "this room already has a sender",
                        );
                    }
                }
            }
        }
        // Everything below is relayed unchanged to the other side.
        ClientMsg::Offer { sdp } => return relay(conn, state, ServerMsg::Offer { sdp }, len),
        ClientMsg::Answer { sdp } => return relay(conn, state, ServerMsg::Answer { sdp }, len),
        ClientMsg::IceCandidate { candidate } => {
            return relay(conn, state, ServerMsg::IceCandidate { candidate }, len);
        }
        ClientMsg::Control { cmd } => return relay(conn, state, ServerMsg::Control { cmd }, len),
    }
    Step::Continue
}

/// Send the error that belongs to a refusal.
fn deny_error(conn: &Conn, deny: Deny) {
    match deny {
        Deny::RateLimited(after) => conn.error(
            error_codes::RATE_LIMITED,
            &format!("too many requests, try again in {} s", after.as_secs()),
        ),
        Deny::Banned(left) => conn.error(
            error_codes::BANNED,
            &format!("temporarily banned for {} s", left.as_secs()),
        ),
        Deny::Busy => conn.error(error_codes::RATE_LIMITED, "the server is busy"),
    }
}

/// A ban ends the connection, other refusals do not.
fn after_deny(deny: Deny) -> Step {
    match deny {
        Deny::Banned(_) => Step::Close,
        _ => Step::Continue,
    }
}

/// Before a join attempt: banned or too many attempts? Returns the step to take if refused.
fn join_precheck(conn: &Conn, state: &AppState) -> Option<Step> {
    match state.guard.check_join(conn.ip, StdInstant::now()) {
        Ok(()) => None,
        Err(deny) => {
            deny_error(conn, deny);
            if let Deny::Banned(_) = deny {
                conn.tx.send(Outgoing::Close("banned"));
            }
            Some(after_deny(deny))
        }
    }
}

/// A join did not work: count it, maybe ban, and tell the client.
fn failed_join(conn: &Conn, state: &AppState, code: &str, message: &str) -> Step {
    if let Some(len) = state.guard.record_failed_join(conn.ip, StdInstant::now()) {
        conn.error(
            error_codes::BANNED,
            &format!("too many failed joins; banned for {} s", len.as_secs()),
        );
        conn.tx.send(Outgoing::Close("banned"));
        return Step::Close;
    }
    conn.error(code, message);
    Step::Continue
}

/// True for `{"type":"join_room","code":"<any string>"}`.
fn is_join_with_string_code(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text).is_ok_and(|v| {
        v.get("type").and_then(|t| t.as_str()) == Some("join_room")
            && v.get("code").is_some_and(|c| c.is_string())
    })
}

fn lock(state: &AppState) -> std::sync::MutexGuard<'_, crate::rooms::Rooms> {
    // If another thread panicked while holding the lock, keep going with the data.
    state.rooms.lock().unwrap_or_else(|e| e.into_inner())
}

/// Pass a message to the other side. `len` is its size in bytes; a room may only relay
/// `max_room_relay_bytes` in total.
fn relay(conn: &Conn, state: &AppState, msg: ServerMsg, len: usize) -> Step {
    let max = state.guard.cfg().max_room_relay_bytes;
    let peer = conn.room.as_ref().and_then(|code| {
        let mut rooms = lock(state);
        let peer = rooms.peer(code, conn.id)?;
        if rooms.add_relayed(code, len as u64, max) {
            Some(Ok(peer))
        } else {
            Some(Err(()))
        }
    });
    match peer {
        Some(Ok(peer)) => peer.send(Outgoing::Msg(msg)),
        Some(Err(())) => {
            state.guard.note_event(StdInstant::now());
            return conn.fail(error_codes::RATE_LIMITED, "this room relayed too much data");
        }
        None => conn.error("not_in_room", "no peer in the room yet"),
    }
    Step::Continue
}

/// Background task for one room: removes it when no sender joined in time.
async fn expire_when_idle(state: AppState, code: RoomCode, cancel: Arc<Notify>) {
    let idle = state.config.idle_room_timeout;
    let mut wait = idle;
    loop {
        // Stop early if the room is removed for another reason.
        tokio::select! {
            _ = tokio::time::sleep(wait) => {}
            _ = cancel.notified() => return,
        }
        match lock(&state).check_expiry(&code, idle) {
            Ok(next) => wait = next,
            Err(Gone) => return,
        }
    }
}
