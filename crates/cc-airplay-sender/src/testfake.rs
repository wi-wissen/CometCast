// SPDX-License-Identifier: GPL-3.0-or-later
//! TEST FAKE, NOT A REAL RECEIVER. A small AirPlay receiver written for the tests of this
//! crate, speaking the protocol subset the sender uses: legacy PIN pairing and pair-verify,
//! HomeKit-style transient and PIN pairing with the encrypted control channel, and the
//! media requests (`/server-info`, `/play`, `/playback-info`, `/scrub`, `/rate`, `/stop`,
//! `/photo`). It was written from the same public sources as the sender (pyatv), so it can
//! only show that the sender is consistent with that reading, not that real receivers agree.
//!
//! Optionally it fetches the HLS playlist and segments of a played URL, like a receiver
//! would.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use aes::Aes128;
use aes::cipher::{KeyIvInit, StreamCipher};
use aes_gcm::AesGcm;
use aes_gcm::aead::consts::U16;
use aes_gcm::aead::{Aead, KeyInit, Nonce};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use plist::{Dictionary, Value};
use sha1::Sha1;
use sha2::Sha512;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::cryptoutil::{hkdf32, open_labelled, random, seal_labelled, sha512};
use crate::framed::Cipher;
use crate::http::{Conn, Request};
use crate::plistx;
use crate::srp::{Group, KeyKind, Server};
use crate::tlv::{self, Tlv};

type Aes128Gcm16 = AesGcm<Aes128, U16>;
type Aes128Ctr = ctr::Ctr128BE<Aes128>;

/// Which pairing the fake demands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FakeMode {
    /// Legacy PIN pairing; media requests need a verified connection.
    Legacy,
    /// HomeKit-style; media requests need an encrypted connection (transient or verified).
    Hap,
    /// No authentication at all.
    Open,
}

/// What the fake saw.
#[derive(Debug, Default)]
pub(crate) struct FakeState {
    pub(crate) log: Vec<String>,
    pub(crate) played_url: Option<String>,
    pub(crate) photo: Option<Vec<u8>>,
    pub(crate) playing_since: Option<Instant>,
    pub(crate) scrub_to: Option<f64>,
    pub(crate) legacy_client_key: Option<[u8; 32]>,
    pub(crate) hap_client_id: Option<Vec<u8>>,
    pub(crate) hap_client_key: Option<[u8; 32]>,
    /// Segments fetched by the fake HLS client: (name, bytes).
    pub(crate) fetched: Vec<(String, Vec<u8>)>,
}

pub(crate) struct FakeReceiver {
    pub(crate) addr: SocketAddr,
    pub(crate) state: Arc<Mutex<FakeState>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

struct Shared {
    mode: FakeMode,
    pin: String,
    fetch_hls: bool,
    rtsp_only: bool,
    acc_seed: [u8; 32],
    acc_id: Vec<u8>,
    state: Arc<Mutex<FakeState>>,
}

impl FakeReceiver {
    pub(crate) fn start(mode: FakeMode, pin: &str, fetch_hls: bool) -> Self {
        Self::start_with(mode, pin, fetch_hls, false)
    }

    /// `rtsp_only`: like UxPlay, ignore requests that are not RTSP/1.0.
    pub(crate) fn start_with(mode: FakeMode, pin: &str, fetch_hls: bool, rtsp_only: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let addr = listener.local_addr().expect("addr");
        let state = Arc::new(Mutex::new(FakeState::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Shared {
            mode,
            pin: pin.into(),
            fetch_hls,
            rtsp_only,
            acc_seed: random(),
            acc_id: b"FAKE-RECEIVER-ID".to_vec(),
            state: Arc::clone(&state),
        });
        let stop2 = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            while !stop2.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((s, _)) => {
                        let sh = Arc::clone(&shared);
                        std::thread::spawn(move || {
                            let _ = serve(s, &sh);
                        });
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
        });
        Self {
            addr,
            state,
            stop,
            thread: Some(thread),
        }
    }

    pub(crate) fn device(&self) -> crate::discovery::Device {
        crate::discovery::Device::from_address(&self.addr.to_string()).expect("device")
    }

    pub(crate) fn log(&self) -> Vec<String> {
        self.state.lock().expect("lock").log.clone()
    }
}

impl Drop for FakeReceiver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

enum Srp {
    Legacy(Server<Sha1>),
    Hap(Server<Sha512>),
}

#[derive(Default)]
struct Conns {
    srp: Option<Srp>,
    key: Vec<u8>,
    legacy_verified: bool,
    pv_shared: Option<[u8; 32]>,
    pv_client_pub: Option<[u8; 32]>,
    pv_server_pub: Option<[u8; 32]>,
    pv_client_key: Option<[u8; 32]>,
    pv_session: Option<[u8; 32]>,
    hap_encrypted: bool,
    hap_verify_ok: bool,
    transient: bool,
}

fn plist_reply(conn: &mut Conn<TcpStream>, d: Dictionary) -> crate::error::Result<()> {
    let body = plistx::encode(d)?;
    conn.write_response(200, &[("Content-Type", plistx::BPLIST_TYPE.into())], &body)
}

fn serve(stream: TcpStream, sh: &Shared) -> crate::error::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut conn = Conn::new(stream);
    let mut c = Conns::default();
    loop {
        let req = conn.read_request(8 * 1024 * 1024, Duration::from_secs(10))?;
        if sh.rtsp_only && !conn.is_rtsp() {
            return Ok(()); // close without an answer
        }
        sh.state.lock().expect("lock").log.push(format!(
            "{} {}",
            req.method,
            req.path.split('?').next().unwrap_or("")
        ));
        let enable_after = handle(&mut conn, sh, &mut c, &req)?;
        if let Some(cipher) = enable_after {
            conn.framed_mut().enable(cipher);
            c.hap_encrypted = true;
        }
    }
}

fn status(conn: &mut Conn<TcpStream>, code: u16) -> crate::error::Result<()> {
    conn.write_response(code, &[], b"")
}

fn authorised(sh: &Shared, c: &Conns) -> bool {
    match sh.mode {
        FakeMode::Open => true,
        FakeMode::Legacy => c.legacy_verified,
        FakeMode::Hap => c.hap_encrypted,
    }
}

/// Returns a cipher to switch on after the response was written.
fn handle(
    conn: &mut Conn<TcpStream>,
    sh: &Shared,
    c: &mut Conns,
    req: &Request,
) -> crate::error::Result<Option<Cipher>> {
    let path = req.path.split('?').next().unwrap_or("");
    match (req.method.as_str(), path) {
        ("POST", "/pair-pin-start") => {
            status(conn, 200)?;
        }
        ("POST", "/pair-setup-pin") => legacy_setup(conn, sh, c, req)?,
        ("POST", "/pair-verify") if sh.mode == FakeMode::Legacy => legacy_verify(conn, sh, c, req)?,
        ("POST", "/pair-setup") => return hap_setup(conn, sh, c, req),
        ("POST", "/pair-verify") => return hap_verify(conn, sh, c, req),
        ("GET", "/server-info") => {
            let mut d = Dictionary::new();
            d.insert("model".into(), Value::String("FakeTV1,1".into()));
            d.insert("features".into(), Value::Integer(0x1u64.into()));
            plist_reply(conn, d)?;
        }
        (_, _) if !authorised(sh, c) => status(conn, 470)?,
        ("POST", "/play") => {
            let d = plistx::decode_dict(&req.body)?;
            let url = match d.get("Content-Location") {
                Some(Value::String(s)) => s.clone(),
                _ => return status(conn, 400).map(|()| None),
            };
            {
                let mut st = sh.state.lock().expect("lock");
                st.played_url = Some(url.clone());
                st.playing_since = Some(Instant::now());
            }
            if sh.fetch_hls {
                let state = Arc::clone(&sh.state);
                std::thread::spawn(move || fake_hls_client(&url, &state));
            }
            status(conn, 200)?;
        }
        ("GET", "/playback-info") => {
            let st = sh.state.lock().expect("lock");
            let mut d = Dictionary::new();
            if let Some(t) = st.playing_since {
                d.insert("duration".into(), Value::Real(60.0));
                d.insert(
                    "position".into(),
                    Value::Real(st.scrub_to.unwrap_or(0.0) + t.elapsed().as_secs_f64()),
                );
                d.insert("rate".into(), Value::Real(1.0));
                d.insert("readyToPlay".into(), Value::Boolean(true));
            } else {
                d.insert("readyToPlay".into(), Value::Boolean(false));
            }
            drop(st);
            plist_reply(conn, d)?;
        }
        ("POST", "/scrub") => {
            let pos = req
                .path
                .split_once("position=")
                .and_then(|(_, v)| v.parse::<f64>().ok());
            sh.state.lock().expect("lock").scrub_to = pos;
            status(conn, 200)?;
        }
        ("GET", "/scrub") => {
            let st = sh.state.lock().expect("lock");
            let text = format!(
                "duration: 60.000000\nposition: {:.6}\n",
                st.scrub_to.unwrap_or(0.0)
            );
            drop(st);
            conn.write_response(
                200,
                &[("Content-Type", "text/parameters".into())],
                text.as_bytes(),
            )?;
        }
        ("POST", "/rate") => status(conn, 200)?,
        ("POST", "/stop") => {
            sh.state.lock().expect("lock").playing_since = None;
            status(conn, 200)?;
        }
        ("PUT", "/photo") => {
            sh.state.lock().expect("lock").photo = Some(req.body.clone());
            status(conn, 200)?;
        }
        _ => status(conn, 404)?,
    }
    Ok(None)
}

fn legacy_setup(
    conn: &mut Conn<TcpStream>,
    sh: &Shared,
    c: &mut Conns,
    req: &Request,
) -> crate::error::Result<()> {
    let d = plistx::decode_dict(&req.body)?;
    if let Some(Value::String(user)) = d.get("user") {
        let salt: [u8; 16] = random();
        let srv = Server::<Sha1>::new(
            Group::rfc5054_2048(),
            KeyKind::Interleaved,
            user.as_bytes(),
            sh.pin.as_bytes(),
            &salt,
            &random::<32>(),
        );
        let mut out = Dictionary::new();
        out.insert("pk".into(), Value::Data(srv.public_key()));
        out.insert("salt".into(), Value::Data(salt.to_vec()));
        c.srp = Some(Srp::Legacy(srv));
        return plist_reply(conn, out);
    }
    if let (Some(pk), Some(proof)) = (plistx::data(&d, "pk"), plistx::data(&d, "proof")) {
        let Some(Srp::Legacy(srv)) = &c.srp else {
            return status(conn, 400);
        };
        return match srv.verify(pk, proof) {
            Some((m2, key)) => {
                c.key = key;
                let mut out = Dictionary::new();
                out.insert("proof".into(), Value::Data(m2));
                plist_reply(conn, out)
            }
            None => status(conn, 470),
        };
    }
    if let (Some(epk), Some(tag)) = (plistx::data(&d, "epk"), plistx::data(&d, "authTag")) {
        let aes_key = &sha512(&[b"Pair-Setup-AES-Key", &c.key])[..16];
        let mut iv = [0u8; 16];
        iv.copy_from_slice(&sha512(&[b"Pair-Setup-AES-IV", &c.key])[..16]);
        iv[15] = iv[15].wrapping_add(1);
        let gcm = Aes128Gcm16::new_from_slice(aes_key).expect("key");
        let mut sealed = epk.to_vec();
        sealed.extend_from_slice(tag);
        return match gcm.decrypt(Nonce::<Aes128Gcm16>::from_slice(&iv), sealed.as_slice()) {
            Ok(pk) if pk.len() == 32 => {
                let mut k = [0u8; 32];
                k.copy_from_slice(&pk);
                sh.state.lock().expect("lock").legacy_client_key = Some(k);
                plist_reply(conn, Dictionary::new())
            }
            _ => status(conn, 470),
        };
    }
    status(conn, 400)
}

fn legacy_verify(
    conn: &mut Conn<TcpStream>,
    sh: &Shared,
    c: &mut Conns,
    req: &Request,
) -> crate::error::Result<()> {
    if req.body.len() != 68 {
        return status(conn, 400);
    }
    if req.body[0] == 1 {
        let secret = StaticSecret::from(random::<32>());
        let server_pub = PublicKey::from(&secret).to_bytes();
        let mut client_pub = [0u8; 32];
        client_pub.copy_from_slice(&req.body[4..36]);
        let mut client_key = [0u8; 32];
        client_key.copy_from_slice(&req.body[36..68]);
        let shared = secret
            .diffie_hellman(&PublicKey::from(client_pub))
            .to_bytes();
        let key = &sha512(&[b"Pair-Verify-AES-Key", &shared])[..16];
        let iv = &sha512(&[b"Pair-Verify-AES-IV", &shared])[..16];
        let acc = SigningKey::from_bytes(&sh.acc_seed);
        let mut info = server_pub.to_vec();
        info.extend_from_slice(&client_pub);
        let mut sig = acc.sign(&info).to_bytes();
        Aes128Ctr::new_from_slices(key, iv)
            .expect("key")
            .apply_keystream(&mut sig);
        let mut body = server_pub.to_vec();
        body.extend_from_slice(&sig);
        c.pv_shared = Some(shared);
        c.pv_client_pub = Some(client_pub);
        c.pv_server_pub = Some(server_pub);
        c.pv_client_key = Some(client_key);
        return conn.write_response(200, &[], &body);
    }
    let (Some(shared), Some(cp), Some(sp), Some(ck)) = (
        c.pv_shared,
        c.pv_client_pub,
        c.pv_server_pub,
        c.pv_client_key,
    ) else {
        return status(conn, 400);
    };
    let key = &sha512(&[b"Pair-Verify-AES-Key", &shared])[..16];
    let iv = &sha512(&[b"Pair-Verify-AES-IV", &shared])[..16];
    let mut cipher = Aes128Ctr::new_from_slices(key, iv).expect("key");
    let mut skip = [0u8; 64];
    cipher.apply_keystream(&mut skip);
    let mut sig = req.body[4..68].to_vec();
    cipher.apply_keystream(&mut sig);
    let mut info = cp.to_vec();
    info.extend_from_slice(&sp);
    let known = sh.state.lock().expect("lock").legacy_client_key;
    let good = known == Some(ck)
        && VerifyingKey::from_bytes(&ck)
            .ok()
            .zip(Signature::from_slice(&sig).ok())
            .is_some_and(|(vk, s)| vk.verify(&info, &s).is_ok());
    if good {
        c.legacy_verified = true;
        status(conn, 200)
    } else {
        status(conn, 470)
    }
}

fn tlv_reply(conn: &mut Conn<TcpStream>, t: &Tlv) -> crate::error::Result<()> {
    conn.write_response(
        200,
        &[("Content-Type", "application/octet-stream".into())],
        &t.encode(),
    )
}

fn server_cipher(ikm: &[u8]) -> Cipher {
    // Mirror image of the client: our output is the client's input.
    let out = hkdf32(ikm, "Control-Salt", "Control-Read-Encryption-Key");
    let inn = hkdf32(ikm, "Control-Salt", "Control-Write-Encryption-Key");
    Cipher::new(&out, &inn)
}

fn hap_setup(
    conn: &mut Conn<TcpStream>,
    sh: &Shared,
    c: &mut Conns,
    req: &Request,
) -> crate::error::Result<Option<Cipher>> {
    let t = Tlv::decode(&req.body)?;
    let seq = t
        .get(tlv::SEQ_NO)
        .and_then(|s| s.first().copied())
        .unwrap_or(0);
    match seq {
        1 => {
            c.transient = t.get(tlv::FLAGS) == Some(&[tlv::FLAG_TRANSIENT][..]);
            let pin = if c.transient { "3939" } else { sh.pin.as_str() };
            let salt: [u8; 16] = random();
            let srv = Server::<Sha512>::new(
                Group::rfc3526_3072(),
                KeyKind::Plain,
                b"Pair-Setup",
                pin.as_bytes(),
                &salt,
                &random::<32>(),
            );
            let reply = Tlv::new()
                .with(tlv::SEQ_NO, [2])
                .with(tlv::PUBLIC_KEY, srv.public_key())
                .with(tlv::SALT, salt);
            c.srp = Some(Srp::Hap(srv));
            tlv_reply(conn, &reply)?;
        }
        3 => {
            let Some(Srp::Hap(srv)) = &c.srp else {
                status(conn, 400)?;
                return Ok(None);
            };
            let (a, m1) = (
                t.get(tlv::PUBLIC_KEY).unwrap_or_default(),
                t.get(tlv::PROOF).unwrap_or_default(),
            );
            match srv.verify(a, m1) {
                Some((m2, key)) => {
                    c.key = key;
                    tlv_reply(
                        conn,
                        &Tlv::new().with(tlv::SEQ_NO, [4]).with(tlv::PROOF, m2),
                    )?;
                    if c.transient {
                        return Ok(Some(server_cipher(&c.key)));
                    }
                }
                None => tlv_reply(
                    conn,
                    &Tlv::new().with(tlv::SEQ_NO, [4]).with(tlv::ERROR, [2]),
                )?,
            }
        }
        5 => {
            let session = hkdf32(&c.key, "Pair-Setup-Encrypt-Salt", "Pair-Setup-Encrypt-Info");
            let plain = open_labelled(
                &session,
                b"PS-Msg05",
                t.get(tlv::ENCRYPTED_DATA).unwrap_or_default(),
            )?;
            let inner = Tlv::decode(&plain)?;
            let id = inner.get(tlv::IDENTIFIER).unwrap_or_default().to_vec();
            let key = inner.get(tlv::PUBLIC_KEY).unwrap_or_default();
            let sig = inner.get(tlv::SIGNATURE).unwrap_or_default();
            let mut k = [0u8; 32];
            if key.len() != 32 {
                return status(conn, 400).map(|()| None);
            }
            k.copy_from_slice(key);
            // Check the controller's signature like a real accessory would.
            let ios_x = hkdf32(
                &c.key,
                "Pair-Setup-Controller-Sign-Salt",
                "Pair-Setup-Controller-Sign-Info",
            );
            let mut info = ios_x.to_vec();
            info.extend_from_slice(&id);
            info.extend_from_slice(&k);
            let ok = VerifyingKey::from_bytes(&k)
                .ok()
                .zip(Signature::from_slice(sig).ok())
                .is_some_and(|(vk, s)| vk.verify(&info, &s).is_ok());
            if !ok {
                return status(conn, 470).map(|()| None);
            }
            {
                let mut st = sh.state.lock().expect("lock");
                st.hap_client_id = Some(id);
                st.hap_client_key = Some(k);
            }
            let acc = SigningKey::from_bytes(&sh.acc_seed);
            let acc_pub = acc.verifying_key().to_bytes();
            let acc_x = hkdf32(
                &c.key,
                "Pair-Setup-Accessory-Sign-Salt",
                "Pair-Setup-Accessory-Sign-Info",
            );
            let mut ainfo = acc_x.to_vec();
            ainfo.extend_from_slice(&sh.acc_id);
            ainfo.extend_from_slice(&acc_pub);
            let inner = Tlv::new()
                .with(tlv::IDENTIFIER, sh.acc_id.clone())
                .with(tlv::PUBLIC_KEY, acc_pub)
                .with(tlv::SIGNATURE, acc.sign(&ainfo).to_bytes());
            tlv_reply(
                conn,
                &Tlv::new().with(tlv::SEQ_NO, [6]).with(
                    tlv::ENCRYPTED_DATA,
                    seal_labelled(&session, b"PS-Msg06", &inner.encode())?,
                ),
            )?;
        }
        _ => status(conn, 400)?,
    }
    Ok(None)
}

fn hap_verify(
    conn: &mut Conn<TcpStream>,
    sh: &Shared,
    c: &mut Conns,
    req: &Request,
) -> crate::error::Result<Option<Cipher>> {
    let t = Tlv::decode(&req.body)?;
    let seq = t
        .get(tlv::SEQ_NO)
        .and_then(|s| s.first().copied())
        .unwrap_or(0);
    if seq == 1 {
        let Ok(client_pub) = <[u8; 32]>::try_from(t.get(tlv::PUBLIC_KEY).unwrap_or_default())
        else {
            status(conn, 400)?;
            return Ok(None);
        };
        let secret = StaticSecret::from(random::<32>());
        let server_pub = PublicKey::from(&secret).to_bytes();
        let shared = secret
            .diffie_hellman(&PublicKey::from(client_pub))
            .to_bytes();
        let session = hkdf32(
            &shared,
            "Pair-Verify-Encrypt-Salt",
            "Pair-Verify-Encrypt-Info",
        );
        let acc = SigningKey::from_bytes(&sh.acc_seed);
        let mut info = server_pub.to_vec();
        info.extend_from_slice(&sh.acc_id);
        info.extend_from_slice(&client_pub);
        let inner = Tlv::new()
            .with(tlv::IDENTIFIER, sh.acc_id.clone())
            .with(tlv::SIGNATURE, acc.sign(&info).to_bytes());
        c.pv_shared = Some(shared);
        c.pv_client_pub = Some(client_pub);
        c.pv_server_pub = Some(server_pub);
        c.pv_session = Some(session);
        tlv_reply(
            conn,
            &Tlv::new()
                .with(tlv::SEQ_NO, [2])
                .with(tlv::PUBLIC_KEY, server_pub)
                .with(
                    tlv::ENCRYPTED_DATA,
                    seal_labelled(&session, b"PV-Msg02", &inner.encode())?,
                ),
        )?;
        return Ok(None);
    }
    let (Some(shared), Some(cp), Some(sp), Some(session)) =
        (c.pv_shared, c.pv_client_pub, c.pv_server_pub, c.pv_session)
    else {
        status(conn, 400)?;
        return Ok(None);
    };
    let plain = open_labelled(
        &session,
        b"PV-Msg03",
        t.get(tlv::ENCRYPTED_DATA).unwrap_or_default(),
    )?;
    let inner = Tlv::decode(&plain)?;
    let id = inner.get(tlv::IDENTIFIER).unwrap_or_default();
    let sig = inner.get(tlv::SIGNATURE).unwrap_or_default();
    let (known_id, known_key) = {
        let st = sh.state.lock().expect("lock");
        (st.hap_client_id.clone(), st.hap_client_key)
    };
    let mut info = cp.to_vec();
    info.extend_from_slice(id);
    info.extend_from_slice(&sp);
    let ok = known_id.as_deref() == Some(id)
        && known_key
            .and_then(|k| VerifyingKey::from_bytes(&k).ok())
            .zip(Signature::from_slice(sig).ok())
            .is_some_and(|(vk, s)| vk.verify(&info, &s).is_ok());
    if !ok {
        tlv_reply(
            conn,
            &Tlv::new().with(tlv::SEQ_NO, [4]).with(tlv::ERROR, [2]),
        )?;
        return Ok(None);
    }
    c.hap_verify_ok = true;
    tlv_reply(conn, &Tlv::new().with(tlv::SEQ_NO, [4]))?;
    Ok(Some(server_cipher(&shared)))
}

/// A very small HLS client: playlist, then every segment in it, a few rounds.
fn fake_hls_client(url: &str, state: &Mutex<FakeState>) {
    let Some(rest) = url.strip_prefix("http://") else {
        return;
    };
    let (hostport, path) = rest.split_once('/').unwrap_or((rest, ""));
    let base = format!("/{}", path.rsplit_once('/').map_or("", |(d, _)| d));
    let base = if base == "/" { String::new() } else { base };
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..12 {
        std::thread::sleep(Duration::from_millis(500));
        let Ok(s) = TcpStream::connect(hostport) else {
            continue;
        };
        let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
        let mut c = Conn::new(s);
        let Ok(pl) = c.exchange(&Request::new("GET", &format!("/{path}"))) else {
            continue;
        };
        for line in String::from_utf8_lossy(&pl.body).lines() {
            if line.ends_with(".ts") && seen.insert(line.to_owned()) {
                if let Ok(seg) = c.exchange(&Request::new("GET", &format!("{base}/{line}"))) {
                    state
                        .lock()
                        .expect("lock")
                        .fetched
                        .push((line.to_owned(), seg.body));
                }
            }
        }
    }
}
