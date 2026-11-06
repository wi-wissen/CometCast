// SPDX-License-Identifier: GPL-3.0-or-later
//! End-to-end tests of the sender state machine on loopback against a FAKE Cast receiver.
//!
//! THE FAKE RECEIVER IN THIS FILE IS A TEST DOUBLE ONLY. It speaks just the message subset the
//! tests need, with certificates generated at test time (an invented root, an invented
//! "device" certificate on a throw-away test key, a self-signed TLS certificate). It is not a
//! usable Cast receiver: real senders (which verify against Google's roots) would reject it,
//! it does not decode or show anything, and it does not advertise itself with mDNS.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cc_cast::channel::{
    CONNECTION_NS, HEARTBEAT_NS, Heartbeat, RECEIVER_NS, WEBRTC_NS, json_message,
};
use cc_cast::device_auth::{AUTH_NAMESPACE, DeviceAuth, TrustAnchors};
use cc_cast::error::CastError;
use cc_cast::framing::{read_message, write_message};
use cc_cast::media::{StreamEnd, VideoUnit, run_stream};
use cc_cast::offer::VideoOffer;
use cc_cast::proto::{AuthResponse, CastMessage, DeviceAuthMessage, HashAlgorithm};
use cc_cast::rtp::{self, ALL_PACKETS_LOST, Nack, RtcpEvent};
use cc_cast::session::{MIRRORING_APP_ID, SessionConfig, negotiate};
use cc_cast::streamer::VideoStreamer;
use prost::Message;
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde_json::{Value, json};
use time::{Duration as TimeDuration, OffsetDateTime};
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::mpsc;
use tokio_rustls::TlsAcceptor;

/// PKCS#8 DER of the fake receiver's "device key": a 2048-bit RSA key generated once per test
/// run (Cast device auth signs with RSA; ring cannot generate RSA keys). Nothing is stored.
fn device_key_der() -> &'static [u8] {
    use rsa::pkcs8::EncodePrivateKey;
    static KEY: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    KEY.get_or_init(|| {
        let k = rsa::RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 2048).unwrap();
        k.to_pkcs8_der().unwrap().as_bytes().to_vec()
    })
}

const RECEIVER_SSRC: u32 = 77_777;

/// AES key and IV mask taken from the OFFER.
type KeySlot = Arc<Mutex<Option<([u8; 16], [u8; 16])>>>;

// ---------------------------------------------------------------------------------------
// The fake receiver
// ---------------------------------------------------------------------------------------

#[derive(Clone, Default)]
struct Behavior {
    /// Answer LAUNCH with LAUNCH_ERROR and this reason.
    launch_error: Option<&'static str>,
    /// Answer the OFFER with an error.
    offer_error: bool,
    /// Sign the wrong data in the device-auth response.
    bad_signature: bool,
    /// Echo a different nonce.
    bad_nonce: bool,
}

#[derive(Default)]
struct Observed {
    pings: usize,
    launched_app: Option<String>,
    offer: Option<Value>,
    stopped: bool,
    sender_reports: usize,
    rtp_packets: usize,
    udp_peer: Option<SocketAddr>,
    connect_destinations: Vec<String>,
}

struct Pki {
    root_der: Vec<u8>,
    device_der: Vec<u8>,
    tls_der: Vec<u8>,
    tls_key_pkcs8: Vec<u8>,
}

fn build_pki() -> Pki {
    let now = OffsetDateTime::now_utc();
    let dn = |cn: &str| {
        let mut d = DistinguishedName::new();
        d.push(DnType::CommonName, cn);
        d
    };
    // An invented root CA. NOT a Cast root.
    let root_key = KeyPair::generate().unwrap();
    let mut root_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    root_params.distinguished_name = dn("Fake Test Root (not a Cast root)");
    root_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    root_params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
    root_params.not_before = now - TimeDuration::days(1);
    root_params.not_after = now + TimeDuration::days(365);
    let root_cert = root_params.self_signed(&root_key).unwrap();
    let issuer = Issuer::new(root_params, root_key);

    // The "device certificate" with the throw-away RSA test key.
    let device_key = KeyPair::try_from(device_key_der()).unwrap();
    let mut dev_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    dev_params.distinguished_name = dn("Fake Test Device");
    dev_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    dev_params.not_before = now - TimeDuration::days(1);
    dev_params.not_after = now + TimeDuration::days(30);
    let dev_cert = dev_params.signed_by(&device_key, &issuer).unwrap();

    // The self-signed TLS certificate of the connection: short lived, like a real device's.
    let tls_key = KeyPair::generate().unwrap();
    let mut tls_params = CertificateParams::new(vec!["localhost".to_owned()]).unwrap();
    tls_params.not_before = now - TimeDuration::hours(1);
    tls_params.not_after = now + TimeDuration::days(1);
    let tls_cert = tls_params.self_signed(&tls_key).unwrap();

    Pki {
        root_der: root_cert.der().to_vec(),
        device_der: dev_cert.der().to_vec(),
        tls_der: tls_cert.der().to_vec(),
        tls_key_pkcs8: tls_key.serialize_der(),
    }
}

struct Fake {
    addr: SocketAddr,
    pki: Arc<Pki>,
    obs: Arc<Mutex<Observed>>,
    frames: mpsc::UnboundedReceiver<(bool, Vec<u8>)>,
    udp_cmd: mpsc::UnboundedSender<Vec<u8>>,
}

impl Fake {
    fn anchors(&self) -> TrustAnchors {
        TrustAnchors(vec![self.pki.root_der.clone()])
    }
}

async fn start_fake(beh: Behavior) -> Fake {
    let pki = Arc::new(build_pki());
    let obs = Arc::new(Mutex::new(Observed::default()));
    let key_slot: KeySlot = Arc::new(Mutex::new(None));

    // UDP side: reassemble, decrypt, hand over the frames.
    let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let udp_port = udp.local_addr().unwrap().port();
    let (frame_tx, frames) = mpsc::unbounded_channel();
    let (udp_cmd, mut udp_cmd_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    {
        let (obs, key_slot) = (Arc::clone(&obs), Arc::clone(&key_slot));
        tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
            let mut parts: Vec<(u16, Vec<u8>)> = Vec::new();
            let mut next_frame_id = 0u32;
            loop {
                tokio::select! {
                    r = udp.recv_from(&mut buf) => {
                        let Ok((n, from)) = r else { return };
                        obs.lock().unwrap().udp_peer = Some(from);
                        let p = &buf[..n];
                        if (200..=207).contains(&p[1]) {
                            obs.lock().unwrap().sender_reports += 1;
                            continue;
                        }
                        let Some(r) = rtp::parse_rtp(p) else { continue };
                        obs.lock().unwrap().rtp_packets += 1;
                        if parts.iter().any(|(id, _)| *id == r.packet_id) {
                            continue; // a retransmission of a packet we already have
                        }
                        parts.push((r.packet_id, r.payload.to_vec()));
                        if parts.len() == usize::from(r.max_packet_id) + 1 {
                            parts.sort_by_key(|(id, _)| *id);
                            let mut data: Vec<u8> = parts.drain(..).flat_map(|(_, d)| d).collect();
                            let (key, iv) = key_slot.lock().unwrap().unwrap();
                            rtp::crypt_frame(&key, &iv, next_frame_id, &mut data);
                            next_frame_id += 1;
                            let _ = frame_tx.send((r.key_frame, data));
                        }
                    }
                    Some(cmd) = udp_cmd_rx.recv() => {
                        let peer = obs.lock().unwrap().udp_peer;
                        if let Some(peer) = peer {
                            let _ = udp.send_to(&cmd, peer).await;
                        }
                    }
                }
            }
        });
    }

    // TLS side.
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let tls_config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(pki.tls_der.clone())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(pki.tls_key_pkcs8.clone())),
        )
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(tls_config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    {
        let (pki, obs) = (Arc::clone(&pki), Arc::clone(&obs));
        tokio::spawn(async move {
            loop {
                let Ok((tcp, _)) = listener.accept().await else {
                    return;
                };
                let Ok(tls) = acceptor.accept(tcp).await else {
                    continue;
                };
                tokio::spawn(serve(
                    tls,
                    Arc::clone(&pki),
                    beh.clone(),
                    Arc::clone(&obs),
                    udp_port,
                    Arc::clone(&key_slot),
                ));
            }
        });
    }
    Fake {
        addr,
        pki,
        obs,
        frames,
        udp_cmd,
    }
}

fn reply_to(req: &CastMessage, namespace: &str, body: &Value) -> CastMessage {
    json_message(&req.destination_id, &req.source_id, namespace, body)
}

async fn serve<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
    mut s: S,
    pki: Arc<Pki>,
    beh: Behavior,
    obs: Arc<Mutex<Observed>>,
    udp_port: u16,
    key_slot: KeySlot,
) {
    while let Ok(m) = read_message(&mut s).await {
        let text = m.text_payload().unwrap_or_default();
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
        let out: Option<CastMessage> = match m.namespace.as_str() {
            AUTH_NAMESPACE => {
                let d = DeviceAuthMessage::decode(m.payload_binary.as_deref().unwrap_or_default())
                    .unwrap();
                let nonce = d.challenge.unwrap().sender_nonce.unwrap();
                let mut signed = nonce.clone();
                signed.extend_from_slice(if beh.bad_signature {
                    b"something else"
                } else {
                    &pki.tls_der
                });
                let key = ring::signature::RsaKeyPair::from_pkcs8(device_key_der()).unwrap();
                let mut sig = vec![0u8; key.public().modulus_len()];
                key.sign(
                    &ring::signature::RSA_PKCS1_SHA256,
                    &ring::rand::SystemRandom::new(),
                    &signed,
                    &mut sig,
                )
                .unwrap();
                let resp = DeviceAuthMessage {
                    challenge: None,
                    response: Some(AuthResponse {
                        signature: sig,
                        client_auth_certificate: pki.device_der.clone(),
                        intermediate_certificate: vec![],
                        signature_algorithm: None,
                        sender_nonce: Some(if beh.bad_nonce { vec![0; 16] } else { nonce }),
                        hash_algorithm: Some(HashAlgorithm::Sha256 as i32),
                        crl: None,
                    }),
                    error: None,
                };
                Some(CastMessage::binary(
                    &m.destination_id,
                    &m.source_id,
                    AUTH_NAMESPACE,
                    resp.encode_to_vec(),
                ))
            }
            CONNECTION_NS if ty == "CONNECT" => {
                obs.lock()
                    .unwrap()
                    .connect_destinations
                    .push(m.destination_id.clone());
                Some(reply_to(&m, CONNECTION_NS, &json!({"type": "CONNECTED"})))
            }
            HEARTBEAT_NS if ty == "PING" => {
                obs.lock().unwrap().pings += 1;
                Some(reply_to(&m, HEARTBEAT_NS, &json!({"type": "PONG"})))
            }
            RECEIVER_NS => match ty {
                "GET_STATUS" => Some(reply_to(
                    &m,
                    RECEIVER_NS,
                    &json!({"type": "RECEIVER_STATUS", "requestId": v["requestId"], "status": {"volume": {"level": 1.0}}}),
                )),
                "LAUNCH" => {
                    if let Some(reason) = beh.launch_error {
                        Some(reply_to(
                            &m,
                            RECEIVER_NS,
                            &json!({"type": "LAUNCH_ERROR", "requestId": v["requestId"], "reason": reason}),
                        ))
                    } else {
                        obs.lock().unwrap().launched_app = v["appId"].as_str().map(str::to_owned);
                        Some(reply_to(
                            &m,
                            RECEIVER_NS,
                            &json!({"type": "RECEIVER_STATUS", "requestId": v["requestId"], "status": {"applications": [
                                {"appId": v["appId"], "displayName": "Fake Mirroring", "sessionId": "sess-1", "transportId": "transport-1"}]}}),
                        ))
                    }
                }
                "STOP" => {
                    obs.lock().unwrap().stopped = true;
                    None
                }
                _ => None,
            },
            WEBRTC_NS if ty == "OFFER" => {
                obs.lock().unwrap().offer = Some(v.clone());
                let st = &v["offer"]["supportedStreams"][0];
                let hex = |k: &str| -> [u8; 16] {
                    let s = st[k].as_str().unwrap();
                    let mut o = [0u8; 16];
                    for (i, b) in o.iter_mut().enumerate() {
                        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
                    }
                    o
                };
                *key_slot.lock().unwrap() = Some((hex("aesKey"), hex("aesIvMask")));
                if beh.offer_error {
                    Some(reply_to(
                        &m,
                        WEBRTC_NS,
                        &json!({"type": "ANSWER", "seqNum": v["seqNum"], "result": "error",
                                "error": {"code": 42, "description": "fake receiver says no"}}),
                    ))
                } else {
                    Some(reply_to(
                        &m,
                        WEBRTC_NS,
                        &json!({"type": "ANSWER", "seqNum": v["seqNum"], "result": "ok",
                                "answer": {"udpPort": udp_port, "sendIndexes": [0], "ssrcs": [RECEIVER_SSRC]}}),
                    ))
                }
            }
            _ => None,
        };
        if let Some(o) = out {
            if write_message(&mut s, &o).await.is_err() {
                return;
            }
        }
    }
}

fn video() -> VideoOffer {
    VideoOffer::new(1280, 720, 30, 4_000_000, false)
}

fn cfg(fake: &Fake, auth: DeviceAuth) -> SessionConfig {
    let mut c = SessionConfig::new(fake.addr, auth);
    c.step_timeout = Duration::from_secs(5);
    c
}

async fn within<T>(f: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), f)
        .await
        .expect("timed out")
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn full_negotiation_with_verified_device_auth() {
    let fake = start_fake(Behavior::default()).await;
    let v = video();
    let neg = negotiate(&cfg(&fake, DeviceAuth::Verify(fake.anchors())), &v)
        .await
        .unwrap();

    // Device auth was verified, with the fake device's subject.
    let outcome = neg.auth_outcome.as_ref().unwrap();
    assert!(outcome.verified);
    assert!(
        outcome
            .device_subject
            .as_deref()
            .unwrap()
            .contains("Fake Test Device")
    );
    assert!(neg.warnings.is_empty());
    // App and session.
    assert_eq!(neg.app.app_id, MIRRORING_APP_ID);
    assert_eq!(neg.app.session_id, "sess-1");
    assert_eq!(neg.app.transport_id, "transport-1");
    assert_eq!(neg.stream.receiver_ssrc, RECEIVER_SSRC);
    assert_eq!(neg.stream.sender_ssrc, v.ssrc);
    assert_eq!(neg.remote_udp.ip(), fake.addr.ip());
    // The receiver saw: LAUNCH of the right app, CONNECTs to receiver-0 and the transport id,
    // and an OFFER with H.264 and keys.
    {
        let o = fake.obs.lock().unwrap();
        assert_eq!(o.launched_app.as_deref(), Some("0F5096E8"));
        assert_eq!(o.connect_destinations, vec!["receiver-0", "transport-1"]);
        let offer = o.offer.as_ref().unwrap();
        assert_eq!(offer["offer"]["castMode"], "mirroring");
        let s = &offer["offer"]["supportedStreams"][0];
        assert_eq!(s["codecName"], "h264");
        assert_eq!(s["aesKey"].as_str().unwrap().len(), 32);
        assert_eq!(s["ssrc"], v.ssrc);
    }
    neg.stop().await;
    within(async {
        while !fake.obs.lock().unwrap().stopped {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
}

#[tokio::test]
async fn launch_error_is_reported_with_its_reason() {
    let fake = start_fake(Behavior {
        launch_error: Some("NOT_FOUND"),
        ..Behavior::default()
    })
    .await;
    let r = negotiate(&cfg(&fake, DeviceAuth::Verify(fake.anchors())), &video()).await;
    match r {
        Err(CastError::LaunchError { reason }) => assert_eq!(reason, "NOT_FOUND"),
        other => panic!("expected LaunchError, got {:?}", other.err()),
    }
}

#[tokio::test]
async fn rejected_offer_is_reported() {
    let fake = start_fake(Behavior {
        offer_error: true,
        ..Behavior::default()
    })
    .await;
    let r = negotiate(&cfg(&fake, DeviceAuth::Verify(fake.anchors())), &video()).await;
    match r {
        Err(CastError::OfferRejected(d)) => assert!(d.contains("fake receiver says no")),
        other => panic!("expected OfferRejected, got {:?}", other.err()),
    }
}

#[tokio::test]
async fn device_auth_fails_closed_without_anchors() {
    let fake = start_fake(Behavior::default()).await;
    let r = negotiate(
        &cfg(&fake, DeviceAuth::Verify(TrustAnchors::default())),
        &video(),
    )
    .await;
    let Err(CastError::DeviceAuth(m)) = r else {
        panic!("expected DeviceAuth error")
    };
    assert!(m.contains("trust anchors"), "{m}");
}

#[tokio::test]
async fn device_auth_rejects_foreign_root_bad_signature_and_bad_nonce() {
    // Anchors of a different, unrelated PKI.
    let fake = start_fake(Behavior::default()).await;
    let other = build_pki();
    let r = negotiate(
        &cfg(
            &fake,
            DeviceAuth::Verify(TrustAnchors(vec![other.root_der])),
        ),
        &video(),
    )
    .await;
    let Err(CastError::DeviceAuth(m)) = r else {
        panic!("expected DeviceAuth error")
    };
    assert!(m.contains("trust anchor"), "{m}");

    let fake = start_fake(Behavior {
        bad_signature: true,
        ..Behavior::default()
    })
    .await;
    let r = negotiate(&cfg(&fake, DeviceAuth::Verify(fake.anchors())), &video()).await;
    let Err(CastError::DeviceAuth(m)) = r else {
        panic!("expected DeviceAuth error")
    };
    assert!(m.contains("signature"), "{m}");

    let fake = start_fake(Behavior {
        bad_nonce: true,
        ..Behavior::default()
    })
    .await;
    let r = negotiate(&cfg(&fake, DeviceAuth::Verify(fake.anchors())), &video()).await;
    let Err(CastError::DeviceAuth(m)) = r else {
        panic!("expected DeviceAuth error")
    };
    assert!(m.contains("nonce"), "{m}");
}

/// Regression test for the reviewer's probe: a chain `[forged, legit device leaf]` where the
/// legit leaf is not a CA must not verify, although every signature in it is valid.
#[test]
fn device_auth_rejects_a_chain_signed_by_a_non_ca_device_leaf() {
    use rsa::pkcs8::EncodePrivateKey;
    let now = OffsetDateTime::now_utc();
    let dn = |cn: &str| {
        let mut d = DistinguishedName::new();
        d.push(DnType::CommonName, cn);
        d
    };
    let root_key = KeyPair::generate().unwrap();
    let mut rp = CertificateParams::new(Vec::<String>::new()).unwrap();
    rp.distinguished_name = dn("Probe Root");
    rp.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    rp.key_usages = vec![KeyUsagePurpose::KeyCertSign];
    rp.not_before = now - TimeDuration::days(1);
    rp.not_after = now + TimeDuration::days(300);
    let root = rp.self_signed(&root_key).unwrap();
    let root_issuer = Issuer::new(rp, root_key);

    // The legitimate device leaf (not a CA).
    let leaf_key = KeyPair::try_from(device_key_der()).unwrap();
    let mut lp = CertificateParams::new(Vec::<String>::new()).unwrap();
    lp.distinguished_name = dn("Legit Device Leaf (not a CA)");
    lp.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    lp.not_before = now - TimeDuration::days(1);
    lp.not_after = now + TimeDuration::days(30);
    let leaf = lp.clone().signed_by(&leaf_key, &root_issuer).unwrap();
    let leaf_issuer = Issuer::new(lp, leaf_key);

    // The attacker's certificate, "issued" by that leaf, with the attacker's own RSA key.
    let atk = rsa::RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 2048).unwrap();
    let atk_der = atk.to_pkcs8_der().unwrap();
    let atk_key = KeyPair::try_from(atk_der.as_bytes()).unwrap();
    let mut ap = CertificateParams::new(Vec::<String>::new()).unwrap();
    ap.distinguished_name = dn("Forged Device");
    ap.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    ap.not_before = now - TimeDuration::days(1);
    ap.not_after = now + TimeDuration::days(30);
    let forged = ap.signed_by(&atk_key, &leaf_issuer).unwrap();

    let pki = build_pki();
    let nonce = [5u8; 16];
    let mut signed = nonce.to_vec();
    signed.extend_from_slice(&pki.tls_der);
    let rk = ring::signature::RsaKeyPair::from_pkcs8(atk_der.as_bytes()).unwrap();
    let mut sig = vec![0u8; rk.public().modulus_len()];
    rk.sign(
        &ring::signature::RSA_PKCS1_SHA256,
        &ring::rand::SystemRandom::new(),
        &signed,
        &mut sig,
    )
    .unwrap();
    let body = DeviceAuthMessage {
        challenge: None,
        response: Some(AuthResponse {
            signature: sig,
            client_auth_certificate: forged.der().to_vec(),
            intermediate_certificate: vec![leaf.der().to_vec()],
            signature_algorithm: None,
            sender_nonce: Some(nonce.to_vec()),
            hash_algorithm: Some(HashAlgorithm::Sha256 as i32),
            crl: None,
        }),
        error: None,
    };
    let msg = CastMessage::binary("receiver-0", "sender-0", AUTH_NAMESPACE, body.encode_to_vec());
    let r = cc_cast::device_auth::verify_response(
        &msg,
        &nonce,
        &pki.tls_der,
        &TrustAnchors(vec![root.der().to_vec()]),
        now.unix_timestamp(),
    );
    let Err(CastError::DeviceAuth(m)) = r else {
        panic!("forged chain was accepted: {r:?}")
    };
    assert!(m.contains("not a CA"), "{m}");
}

#[tokio::test]
async fn insecure_skip_works_but_warns_loudly() {
    let fake = start_fake(Behavior {
        bad_signature: true,
        ..Behavior::default()
    })
    .await;
    let neg = negotiate(&cfg(&fake, DeviceAuth::InsecureSkip), &video())
        .await
        .unwrap();
    assert!(neg.auth_outcome.is_none());
    assert_eq!(neg.warnings.len(), 1);
    assert!(neg.warnings[0].contains("DEVICE AUTHENTICATION SKIPPED"));
}

#[tokio::test]
async fn heartbeat_pings_flow_while_connected() {
    let fake = start_fake(Behavior::default()).await;
    let mut c = cfg(&fake, DeviceAuth::InsecureSkip);
    c.heartbeat = Heartbeat {
        interval: Duration::from_millis(100),
        timeout: Duration::from_secs(5),
    };
    let _neg = negotiate(&c, &video()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(450)).await;
    assert!(
        fake.obs.lock().unwrap().pings >= 3,
        "pings: {}",
        fake.obs.lock().unwrap().pings
    );
}

#[tokio::test]
async fn media_frames_arrive_encrypted_and_decrypt_to_the_original() {
    let mut fake = start_fake(Behavior::default()).await;
    let neg = negotiate(&cfg(&fake, DeviceAuth::Verify(fake.anchors())), &video())
        .await
        .unwrap();
    let mut streamer = VideoStreamer::new(neg.remote_udp, neg.stream.clone())
        .await
        .unwrap();

    // A key frame spanning several packets, then two small delta frames.
    let big: Vec<u8> = (0..5000u32).map(|i| (i * 7 % 251) as u8).collect();
    streamer
        .send_frame(&big, true, Duration::ZERO)
        .await
        .unwrap();
    streamer
        .send_frame(b"\x00\x00\x00\x01delta-1", false, Duration::from_millis(33))
        .await
        .unwrap();
    streamer
        .send_frame(b"\x00\x00\x00\x01delta-2", false, Duration::from_millis(66))
        .await
        .unwrap();

    let (k, d) = within(fake.frames.recv()).await.unwrap();
    assert!(k);
    assert_eq!(d, big);
    let (k, d) = within(fake.frames.recv()).await.unwrap();
    assert!(!k);
    assert_eq!(d, b"\x00\x00\x00\x01delta-1");
    let (_, d) = within(fake.frames.recv()).await.unwrap();
    assert_eq!(d, b"\x00\x00\x00\x01delta-2");
    let o = fake.obs.lock().unwrap();
    assert!(
        o.sender_reports >= 1,
        "the sender report must come before the media"
    );
    assert_eq!(o.rtp_packets, 4 + 1 + 1);
}

#[tokio::test]
async fn picture_loss_and_nacks_from_the_receiver_are_served() {
    let mut fake = start_fake(Behavior::default()).await;
    let v = video();
    let neg = negotiate(&cfg(&fake, DeviceAuth::InsecureSkip), &v)
        .await
        .unwrap();
    let mut streamer = VideoStreamer::new(neg.remote_udp, neg.stream.clone())
        .await
        .unwrap();
    let big = vec![0xabu8; 4000];
    streamer
        .send_frame(&big, true, Duration::ZERO)
        .await
        .unwrap();
    within(fake.frames.recv()).await.unwrap();

    // PLI.
    fake.udp_cmd
        .send(rtp::picture_loss_packet(RECEIVER_SSRC, v.ssrc))
        .unwrap();
    let ev = within(streamer.recv_rtcp()).await.unwrap();
    assert_eq!(ev, vec![RtcpEvent::PictureLoss]);
    assert_eq!(streamer.stats().picture_loss, 1);

    // NACK for all packets of frame 0: the sender sends them again.
    let nack = Nack {
        frame_id8: 0,
        packet_id: ALL_PACKETS_LOST,
        bit_vector: 0,
    };
    fake.udp_cmd
        .send(rtp::feedback_packet(RECEIVER_SSRC, v.ssrc, 0, &[nack]))
        .unwrap();
    within(streamer.recv_rtcp()).await.unwrap();
    let st = streamer.stats();
    assert_eq!(st.feedback, 1);
    assert_eq!(st.resent_packets, 3); // 4000 bytes in 1449-byte packets
    within(async {
        while fake.obs.lock().unwrap().rtp_packets < 6 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
}

#[tokio::test]
async fn run_stream_sends_queued_frames_and_ends_when_the_source_ends() {
    let mut fake = start_fake(Behavior::default()).await;
    let mut neg = negotiate(&cfg(&fake, DeviceAuth::InsecureSkip), &video())
        .await
        .unwrap();
    let mut streamer = VideoStreamer::new(neg.remote_udp, neg.stream.clone())
        .await
        .unwrap();
    let (tx, rx) = mpsc::channel(4);
    tx.send(VideoUnit {
        data: vec![1; 100],
        key_frame: true,
        pts: Duration::ZERO,
    })
    .await
    .unwrap();
    tx.send(VideoUnit {
        data: vec![2; 100],
        key_frame: false,
        pts: Duration::from_millis(40),
    })
    .await
    .unwrap();
    drop(tx);
    let key_request = std::sync::atomic::AtomicBool::new(false);
    let end = within(run_stream(&mut neg, &mut streamer, rx, &key_request))
        .await
        .unwrap();
    assert_eq!(end, StreamEnd::SourceEnded);
    assert_eq!(within(fake.frames.recv()).await.unwrap().1, vec![1; 100]);
    assert_eq!(within(fake.frames.recv()).await.unwrap().1, vec![2; 100]);
}

#[tokio::test]
async fn real_h264_encoder_output_survives_the_whole_path_and_decodes() {
    use cc_media::{
        BgraFrame, H264Backend, H264Config, H264Decoder, H264Encoder, annexb_nal_types,
    };
    let mut fake = start_fake(Behavior::default()).await;
    let neg = negotiate(&cfg(&fake, DeviceAuth::InsecureSkip), &video())
        .await
        .unwrap();
    let mut streamer = VideoStreamer::new(neg.remote_udp, neg.stream.clone())
        .await
        .unwrap();

    let (w, h) = (64u32, 64u32);
    let mut bgra = Vec::new();
    for y in 0..h {
        for x in 0..w {
            bgra.extend_from_slice(&[(x * 4) as u8, (y * 4) as u8, 128, 255]);
        }
    }
    let config = H264Config::default();
    let mut enc = H264Encoder::new(&config).unwrap();
    let e = enc
        .encode_bgra(
            BgraFrame {
                width: w,
                height: h,
                bgra: &bgra,
            },
            0,
        )
        .unwrap()
        .expect("first picture must produce output");
    assert!(e.is_keyframe);
    streamer
        .send_frame(&e.data, e.is_keyframe, Duration::ZERO)
        .await
        .unwrap();

    let (key, got) = within(fake.frames.recv()).await.unwrap();
    assert!(key);
    assert_eq!(got, e.data);
    assert!(annexb_nal_types(&got).contains(&5), "IDR slice expected");
    let backend: H264Backend = config.backend;
    let mut dec = H264Decoder::new(&backend).unwrap();
    let pic = dec.decode(&got).unwrap().expect("decodable picture");
    assert_eq!((pic.width, pic.height), (w, h));
}

/// A scripted screen: a few different pictures, then `Ended`.
struct ScriptedScreen {
    left: u8,
}

impl cc_capture::ScreenSource for ScriptedScreen {
    fn list_sources(&self) -> Result<Vec<cc_capture::CaptureSource>, cc_capture::CaptureError> {
        Ok(vec![])
    }
    fn start(&mut self, _: &cc_capture::CaptureSource) -> Result<(), cc_capture::CaptureError> {
        Ok(())
    }
    fn next_frame(&mut self) -> Result<cc_capture::Capture, cc_capture::CaptureError> {
        if self.left == 0 {
            return Ok(cc_capture::Capture::Ended);
        }
        self.left -= 1;
        let shade = 40 * self.left;
        Ok(cc_capture::Capture::Frame(cc_capture::RawFrame {
            width: 64,
            height: 64,
            bgra: [shade, 100, 200, 255].repeat(64 * 64),
        }))
    }
    fn stop(&mut self) {}
}

#[tokio::test]
async fn capture_loop_to_fake_receiver_pipeline() {
    use cc_cast::media::{CaptureLoop, capture_loop};
    use cc_media::{H264Config, H264Encoder};
    use std::sync::atomic::AtomicBool;

    let mut fake = start_fake(Behavior::default()).await;
    let mut neg = negotiate(&cfg(&fake, DeviceAuth::InsecureSkip), &video())
        .await
        .unwrap();
    let mut streamer = VideoStreamer::new(neg.remote_udp, neg.stream.clone())
        .await
        .unwrap();
    let (tx, rx) = mpsc::channel(4);
    let key_request = Arc::new(AtomicBool::new(true));
    let stop = Arc::new(AtomicBool::new(false));
    let cl = CaptureLoop {
        source: Box::new(ScriptedScreen { left: 3 }),
        encoder: H264Encoder::new(&H264Config::default()).unwrap(),
        fps: 30,
    };
    let (k, s) = (Arc::clone(&key_request), Arc::clone(&stop));
    let t = tokio::task::spawn_blocking(move || capture_loop(cl, &tx, &k, &s));
    let end = within(run_stream(&mut neg, &mut streamer, rx, &key_request))
        .await
        .unwrap();
    assert_eq!(end, StreamEnd::SourceEnded);
    t.await.unwrap().unwrap();
    // The first picture is a key frame; every received frame is Annex-B.
    let (key, first) = within(fake.frames.recv()).await.unwrap();
    assert!(key);
    assert_eq!(&first[..4], &[0, 0, 0, 1]);
}
