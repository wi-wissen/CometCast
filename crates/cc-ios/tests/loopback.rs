// SPDX-License-Identifier: GPL-3.0-or-later
//! The engine behind the C ABI end to end, with a real cc-server in this process and a real
//! native receiver (cc-receiver) over loopback UDP. The test plays the part of the Swift
//! extension: it pushes H.264 access units that look like VideoToolbox output (Annex B, SPS/PPS
//! in front of the key frames, small synthetic NAL payloads; nothing is decoded) and reads the
//! status reports. No OpenH264 and no iOS involved.

// The C ABI tests call the unsafe functions directly.
#![allow(unsafe_code)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cc_ios::engine::{self, Config, PushCode, Sink, State, Status};
use cc_ios::{ErrorCode, ffi};
use cc_receiver::{ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_server::{ServerConfig, run};
use cc_transport::{SessionConfig, VideoTap};
use tokio::net::TcpListener;

// The engine is a process-wide singleton: the tests of this file run one at a time.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

const SPS: &[u8] = &[0x67, 0x42, 0xc0, 0x1f, 0xda, 0x05, 0xa0, 0x80];
const PPS: &[u8] = &[0x68, 0xce, 0x3c, 0x80];

fn annex_b(nals: &[&[u8]]) -> Vec<u8> {
    let mut v = Vec::new();
    for n in nals {
        v.extend_from_slice(&[0, 0, 0, 1]);
        v.extend_from_slice(n);
    }
    v
}

fn key_frame() -> Vec<u8> {
    let mut idr = vec![0x65, 0x88, 0x84];
    idr.extend((0..600u32).map(|i| (i % 251) as u8 + 1));
    annex_b(&[SPS, PPS, &idr])
}

fn delta_frame(n: u8) -> Vec<u8> {
    let mut p = vec![0x41, 0x9a, n];
    p.extend((0..200u32).map(|i| (i % 241) as u8 + 1));
    annex_b(&[&p])
}

fn collecting_sink() -> (Sink, Arc<Mutex<Vec<Status>>>) {
    let log = Arc::new(Mutex::new(Vec::<Status>::new()));
    let l = Arc::clone(&log);
    let sink: Sink = Arc::new(move |s: &Status| l.lock().unwrap().push(s.clone()));
    (sink, log)
}

fn last_state(log: &Arc<Mutex<Vec<Status>>>) -> Option<(State, ErrorCode)> {
    log.lock().unwrap().last().map(|s| (s.state, s.error))
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

fn config(server: &str, code: &str) -> Config {
    Config {
        room_code: code.to_owned(),
        relay_url: server.to_owned(),
        log_dir: None,
        udp_bind: Some("127.0.0.1:0".into()),
    }
}

#[test]
fn frames_pushed_through_the_engine_reach_a_receiver() {
    let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (server, mut receiver, mut tap_rx) = rt.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(run(
            listener,
            ServerConfig::default(),
            std::future::pending(),
        ));
        let (tap, rx) = VideoTap::channel_default();
        let mut o = ReceiverOptions::new(&addr);
        o.session = SessionConfig {
            udp_bind: vec!["127.0.0.1:0".into()],
            video_tap: Some(tap),
            ..SessionConfig::default()
        };
        (addr, ReceiverCore::start(o).await.unwrap(), rx)
    });
    let code = receiver.room_code().to_string();
    // Keep the receiver's events flowing.
    rt.spawn(async move {
        while let Some(ev) = receiver.next_event().await {
            if let ReceiverEvent::Error(e) = ev {
                panic!("receiver error: {e}");
            }
        }
    });

    // Nothing runs yet: pushes are refused.
    assert_eq!(
        engine::push_frame(&key_frame(), true, 0),
        PushCode::NoSender
    );

    let (sink, log) = collecting_sink();
    engine::start(config(&server, &code), sink.clone()).expect("start");
    // A second start while one runs is refused.
    assert_eq!(
        engine::start(config(&server, &code), sink),
        Err(ErrorCode::AlreadyRunning)
    );

    // The host's loop: ask for key frames when requested, push about 15 units per second.
    assert!(engine::set_config(&annex_b(&[SPS, PPS])));
    let mut n = 0u8;
    let mut pts = 0i64;
    let mut outcomes = Vec::new();
    let mut packets_total = 0usize;
    let got = wait_for(Duration::from_secs(25), || {
        n = n.wrapping_add(1);
        pts += 66_000;
        let key = engine::keyframe_requested() || n == 1;
        let unit = if key { key_frame() } else { delta_frame(n) };
        outcomes.push(engine::push_frame(&unit, key, pts));
        while tap_rx.try_recv().is_ok() {
            packets_total += 1;
        }
        packets_total >= 6 && last_state(&log).is_some_and(|(s, _)| s == State::Streaming)
    });
    let states: Vec<State> = log.lock().unwrap().iter().map(|s| s.state).collect();
    assert!(
        got,
        "no media at the receiver; states={states:?} outcomes={:?}",
        &outcomes[outcomes.len().saturating_sub(10)..]
    );
    assert_eq!(states[0], State::Connecting);
    assert!(states.contains(&State::Waiting), "{states:?}");
    assert!(outcomes.contains(&PushCode::Queued), "{outcomes:?}");
    let last = log.lock().unwrap().last().cloned().unwrap();
    assert!(last.pushed >= 1, "{last:?}");

    engine::stop();
    assert_eq!(
        engine::push_frame(&key_frame(), true, 1),
        PushCode::NoSender
    );
    // Stopping twice is harmless and a new broadcast can start again.
    engine::stop();
    drop(rt);
}

#[test]
fn an_unknown_room_is_reported_as_room_not_found() {
    let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(run(
            listener,
            ServerConfig::default(),
            std::future::pending(),
        ));
        addr
    });
    let (sink, log) = collecting_sink();
    // A well-formed code of a room that does not exist.
    let code = "ABCDEF";
    if let Err(e) = engine::start(config(&server, code), sink) {
        // The alphabet of the room codes is decided by cc-proto; if this literal is not
        // valid there, the test must say so instead of failing mysteriously.
        panic!("test room code rejected by the engine: {e:?}");
    }
    assert!(
        wait_for(Duration::from_secs(15), || {
            last_state(&log).is_some_and(|(s, _)| s == State::Failed)
        }),
        "{:?}",
        log.lock().unwrap().last()
    );
    assert_eq!(
        last_state(&log),
        Some((State::Failed, ErrorCode::RoomNotFound))
    );
    // The failed broadcast refuses pushes, and a new start replaces it without `stop`.
    assert_eq!(
        engine::push_frame(&key_frame(), true, 1),
        PushCode::NoSender
    );
    engine::stop();
}

#[test]
fn an_unreachable_relay_is_reported() {
    let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
    let (sink, log) = collecting_sink();
    // Nothing listens on port 1 of the loopback interface.
    engine::start(config("127.0.0.1:1", "ABCDEF"), sink).expect("start returns at once");
    assert!(wait_for(Duration::from_secs(15), || {
        last_state(&log).is_some_and(|(s, _)| s == State::Failed)
    }));
    assert_eq!(
        last_state(&log),
        Some((State::Failed, ErrorCode::ServerUnreachable))
    );
    engine::stop();
}

#[test]
fn the_c_functions_work_without_a_running_sender() {
    let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
    use std::ffi::CString;
    let bad = CString::new("no").unwrap();
    let relay = CString::new("127.0.0.1:1").unwrap();
    // SAFETY: valid NUL-terminated strings, no callback.
    let r = unsafe {
        ffi::cc_ios_start(
            bad.as_ptr(),
            relay.as_ptr(),
            std::ptr::null(),
            None,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(r, ErrorCode::InvalidRoomCode as i32);
    // SAFETY: a null pointer is explicitly allowed (returns "invalid").
    let r = unsafe { ffi::cc_ios_push_frame(std::ptr::null(), 0, true, 0) };
    assert_eq!(r, PushCode::Invalid as i32);
    let unit = key_frame();
    // SAFETY: `unit` is a valid buffer of its length.
    let r = unsafe { ffi::cc_ios_push_frame(unit.as_ptr(), unit.len(), true, 0) };
    assert_eq!(r, PushCode::NoSender as i32);
    assert!(!ffi::cc_ios_keyframe_requested());
    ffi::cc_ios_stop();
}
