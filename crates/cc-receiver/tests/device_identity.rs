// SPDX-License-Identifier: GPL-3.0-or-later
//! Presenter device identity on the display (docs/design/presenter-identity.md): proofs are
//! verified, the memory of names and devices gives the note, against a real `cc-server`.

use std::time::Duration;

use cc_proto::{ClientMsg, JoinDevice, JoinMode, Role, RoomCode};
use cc_receiver::{DeviceMemory, ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_server::{ServerConfig, run};
use cc_transport::device::{DeviceKey, device_id_of_secret, sign_join, unix_ms};
use cc_transport::{SessionConfig, SignalingClient};
use tokio::net::TcpListener;

async fn start() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(run(
        listener,
        ServerConfig::default(),
        std::future::pending(),
    ));
    addr.to_string()
}

async fn display(server: &str, mode: JoinMode, memory: DeviceMemory) -> ReceiverCore {
    let mut o = ReceiverOptions::new(server);
    o.session = SessionConfig {
        udp_bind: vec!["127.0.0.1:0".into()],
        ..SessionConfig::default()
    };
    o.join_mode = mode;
    o.device_memory = memory;
    ReceiverCore::start(o).await.unwrap()
}

const SECRETS: [[u8; 32]; 3] = [[1; 32], [2; 32], [3; 32]];

fn id(n: usize) -> String {
    device_id_of_secret(&SECRETS[n])
}

fn proof(n: usize, code: &RoomCode) -> JoinDevice {
    DeviceKey::from_secret(&SECRETS[n])
        .unwrap()
        .proof_now(code.as_str())
}

async fn join(
    server: &str,
    code: &RoomCode,
    name: Option<&str>,
    device: Option<JoinDevice>,
) -> SignalingClient {
    let sig = SignalingClient::connect(server, Role::Sender)
        .await
        .unwrap();
    sig.send(ClientMsg::JoinRoom {
        code: code.clone(),
        name: name.map(str::to_owned),
        handover: None,
        device,
    })
    .await
    .unwrap();
    sig
}

async fn next_matching<T>(
    r: &mut ReceiverCore,
    mut f: impl FnMut(ReceiverEvent) -> Option<T>,
) -> T {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let ev = r.next_event().await.expect("receiver ended");
            if let Some(v) = f(ev) {
                return v;
            }
        }
    })
    .await
    .expect("timeout")
}

/// A join request: (request id, device, note).
async fn request(d: &mut ReceiverCore) -> (u64, Option<String>, bool) {
    next_matching(d, |e| match e {
        ReceiverEvent::JoinRequest {
            id,
            device,
            other_device,
            ..
        } => Some((id, device, other_device)),
        _ => None,
    })
    .await
}

async fn peer_joined(d: &mut ReceiverCore) -> (Option<String>, bool) {
    next_matching(d, |e| match e {
        ReceiverEvent::PeerJoined {
            device,
            other_device,
            ..
        } => Some((device, other_device)),
        _ => None,
    })
    .await
}

async fn left(d: &mut ReceiverCore) {
    next_matching(d, |e| matches!(e, ReceiverEvent::PeerLeft).then_some(())).await;
}

#[tokio::test]
async fn the_note_appears_for_a_known_name_with_another_device_only() {
    let server = start().await;
    let mut d = display(&server, JoinMode::Confirm, DeviceMemory::new()).await;
    let code = d.room_code().clone();

    // A name never seen: no note. The host allows; the pair is remembered.
    let s = join(&server, &code, Some("Julian's iPad"), Some(proof(0, &code))).await;
    let (req, device, note) = request(&mut d).await;
    assert_eq!((device, note), (Some(id(0)), false));
    d.decide_join(req, true);
    let stored = next_matching(&mut d, |e| match e {
        ReceiverEvent::DeviceMemoryChanged(j) => Some(j),
        _ => None,
    })
    .await;
    assert!(
        stored.contains(&id(0)) && stored.contains("Julian's iPad"),
        "{stored}"
    );
    assert_eq!(peer_joined(&mut d).await, (Some(id(0)), false));
    drop(s);
    left(&mut d).await;

    // The same name from another device: the note. Not allowed, so not remembered.
    let s = join(&server, &code, Some("julian's ipad"), Some(proof(1, &code))).await;
    let (req, device, note) = request(&mut d).await;
    assert_eq!((device, note), (Some(id(1)), true));
    d.decide_join(req, false);
    drop(s);

    // Again from the first device: no note (stored with this id).
    let s = join(&server, &code, Some("Julian's iPad"), Some(proof(0, &code))).await;
    let (req, device, note) = request(&mut d).await;
    assert_eq!((device, note), (Some(id(0)), false));
    d.decide_join(req, false);
    drop(s);

    // The denied device was not remembered, and it is still "another" device.
    let s = join(&server, &code, Some("Julian's iPad"), Some(proof(1, &code))).await;
    let (_, device, note) = request(&mut d).await;
    assert_eq!((device, note), (Some(id(1)), true));
    drop(s);
    d.stop().await;
}

#[tokio::test]
async fn no_note_without_a_valid_proof_and_never_for_generated_names() {
    let server = start().await;
    let mut memory = DeviceMemory::new();
    memory.remember("Julian's iPad", &id(0));
    let mut d = display(&server, JoinMode::Confirm, memory).await;
    let code = d.room_code().clone();
    let other_code = RoomCode::parse("K7QM2W").unwrap();
    let old = unix_ms() - 121_000;
    let bad_sig = {
        let mut p = proof(1, &code);
        p.sig = proof(2, &code).sig;
        p
    };
    let cases: Vec<(&str, Option<JoinDevice>)> = vec![
        ("no proof", None),
        ("proof for another room", Some(proof(1, &other_code))),
        (
            "proof too old",
            Some(sign_join(&SECRETS[1], code.as_str(), old)),
        ),
        ("proof of another key's signature", Some(bad_sig)),
    ];
    for (what, device) in cases {
        let s = join(&server, &code, Some("Julian's iPad"), device).await;
        let (req, device, note) = request(&mut d).await;
        assert_eq!((device, note), (None, false), "{what}");
        d.decide_join(req, false);
        drop(s);
    }
    // A joiner without a name is "Guest N": valid proof, no note, nothing remembered.
    let s = join(&server, &code, None, Some(proof(1, &code))).await;
    let (req, device, note) = request(&mut d).await;
    assert_eq!((device, note), (Some(id(1)), false));
    d.decide_join(req, true);
    peer_joined(&mut d).await;
    drop(s);
    left(&mut d).await;
    let s = join(&server, &code, None, Some(proof(2, &code))).await;
    let (_, device, note) = request(&mut d).await;
    assert_eq!((device, note), (Some(id(2)), false));
    drop(s);
    d.stop().await;
}

#[tokio::test]
async fn open_mode_reports_the_note_on_the_join_and_remembers_verified_joins() {
    let server = start().await;
    let mut d = display(&server, JoinMode::Open, DeviceMemory::new()).await;
    let code = d.room_code().clone();

    let s = join(&server, &code, Some("Ana"), Some(proof(0, &code))).await;
    assert_eq!(peer_joined(&mut d).await, (Some(id(0)), false));
    drop(s);
    left(&mut d).await;

    let s = join(&server, &code, Some("ANA"), Some(proof(1, &code))).await;
    assert_eq!(peer_joined(&mut d).await, (Some(id(1)), true));
    drop(s);
    left(&mut d).await;

    // No proof: joins as always, no device, no note.
    let s = join(&server, &code, Some("Ana"), None).await;
    assert_eq!(peer_joined(&mut d).await, (None, false));
    drop(s);
    d.stop().await;
}

#[tokio::test]
async fn the_memory_comes_from_the_options_and_goes_back_as_text() {
    let server = start().await;
    let mut memory = DeviceMemory::new();
    memory.remember("Ana", &id(0));
    let restored = DeviceMemory::from_json(&memory.to_json());
    let mut d = display(&server, JoinMode::Open, restored).await;
    let code = d.room_code().clone();
    let s = join(&server, &code, Some("Ana"), Some(proof(1, &code))).await;
    assert_eq!(peer_joined(&mut d).await, (Some(id(1)), true));
    drop(s);
    d.stop().await;
}
