// SPDX-License-Identifier: GPL-3.0-or-later
//! HomeKit-style pairing as AirPlay 2 receivers use it: transient pairing (PIN 3939, no
//! stored result), PIN pairing (M1 to M6, long-term keys) and pair-verify (M1 to M4).
//! Messages are TLV8 bodies of `POST /pair-setup` and `POST /pair-verify`; the header
//! `X-Apple-HKP` names the variant.
//!
//! Source: pyatv `protocols/airplay/auth/hap.py`, `hap_transient.py`, `auth/hap_srp.py`
//! (MIT), read, not copied; names of the HKDF labels are the HomeKit Accessory Protocol
//! labels. SRP: 3072-bit group, SHA-512, user name `Pair-Setup`.
//!
//! UNCERTAIN: all of it is checked only against our own test fake, which was written from
//! the same sources. No Apple TV, HomePod or AirPlay 2 TV was available. In particular:
//! * `X-Apple-HKP` values: 4 transient, 3 PIN are from pyatv; 5 (tvOS, with the access
//!   control entry `com.apple.ScreenCapture`, from the doubletake analysis in the project
//!   plan) is NOT implemented because the encoding of that entry is not documented in a
//!   source we may use;
//! * whether a receiver that supports only HKP 5 or newer behaviour accepts these messages
//!   (a tvOS build is reported to answer 470 to transient pairing);
//! * the receiver's M4 proof is checked when present, pyatv does not look at it.

use std::io::{Read, Write};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::Sha512;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::creds::Credentials;
use crate::cryptoutil::{hex, hkdf32, open_labelled, random, seal_labelled, unhex, unhex_vec};
use crate::error::{Result, SenderError};
use crate::framed::Cipher;
use crate::http::{Conn, Request, Response};
use crate::srp::{Client, Group, KeyKind};
use crate::tlv::{self, Tlv};

/// The fixed PIN of transient pairing.
pub const TRANSIENT_PIN: &str = "3939";
const SRP_USER: &[u8] = b"Pair-Setup";

/// Which pairing variant to announce in `X-Apple-HKP`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hkp {
    /// 3: PIN pairing (pyatv).
    Pin = 3,
    /// 4: transient pairing (pyatv).
    Transient = 4,
}

fn req(path: &str, hkp: Hkp, body: &Tlv) -> Request {
    Request::new("POST", path)
        .header("User-Agent", "AirPlay/320.20")
        .header("Connection", "keep-alive")
        .header("X-Apple-HKP", (hkp as u8).to_string())
        .body("application/octet-stream", body.encode())
}

fn tlv_answer(resp: &Response, what: &str) -> Result<Tlv> {
    if resp.status == 470 || resp.status == 403 {
        return Err(SenderError::Auth(format!(
            "the receiver refused {what} (status {}); it may need another pairing variant",
            resp.status
        )));
    }
    if resp.status != 200 {
        return Err(SenderError::Status {
            status: resp.status,
            what: format!("POST {what}"),
        });
    }
    let t = Tlv::decode(&resp.body)?;
    t.check_error()?;
    Ok(t)
}

fn seq_is(t: &Tlv, want: u8) -> Result<()> {
    match t.get(tlv::SEQ_NO) {
        Some([n]) if *n == want => Ok(()),
        _ => Err(SenderError::Protocol(format!(
            "pairing answer is not step {want}"
        ))),
    }
}

/// Keys of the encrypted control channel, to be given to
/// [`crate::framed::Framed::enable`].
pub fn control_cipher(ikm: &[u8]) -> Cipher {
    // "output" is what we write, "input" what we read (pyatv names).
    let out = hkdf32(ikm, "Control-Salt", "Control-Write-Encryption-Key");
    let inn = hkdf32(ikm, "Control-Salt", "Control-Read-Encryption-Key");
    Cipher::new(&out, &inn)
}

/// State between `setup_pin_start` and `setup_pin_finish`.
pub struct PinSetup {
    salt: Vec<u8>,
    server_pub: Vec<u8>,
}

/// Transient pairing: SRP with the fixed PIN, no long-term keys. On success the connection
/// switches to the encrypted framing.
pub fn pair_transient<S: Read + Write>(conn: &mut Conn<S>) -> Result<()> {
    // pyatv sends an (empty) /pair-pin-start first, also for transient pairing.
    let r = conn.exchange(&req("/pair-pin-start", Hkp::Transient, &Tlv::new()))?;
    if r.status != 200 {
        return Err(SenderError::Status {
            status: r.status,
            what: "POST /pair-pin-start".into(),
        });
    }
    let m1 = Tlv::new()
        .with(tlv::METHOD, [0])
        .with(tlv::SEQ_NO, [1])
        .with(tlv::FLAGS, [tlv::FLAG_TRANSIENT]);
    let m2 = tlv_answer(
        &conn.exchange(&req("/pair-setup", Hkp::Transient, &m1))?,
        "/pair-setup (M1)",
    )?;
    seq_is(&m2, 2)?;
    let salt = m2.require(tlv::SALT, "a salt")?.to_vec();
    let server_pub = m2.require(tlv::PUBLIC_KEY, "a public key")?.to_vec();

    let mut srp = new_srp(TRANSIENT_PIN)?;
    let res = srp.process(&salt, &server_pub)?;
    let m3 = Tlv::new()
        .with(tlv::SEQ_NO, [3])
        .with(tlv::PUBLIC_KEY, srp.public_key())
        .with(tlv::PROOF, res.proof);
    let m4 = tlv_answer(
        &conn.exchange(&req("/pair-setup", Hkp::Transient, &m3))?,
        "/pair-setup (M3)",
    )?;
    if let Some(p) = m4.get(tlv::PROOF) {
        if !srp.verify_server_proof(p) {
            return Err(SenderError::Auth("the receiver's M4 proof is wrong".into()));
        }
    }
    conn.framed_mut().enable(control_cipher(&res.session_key));
    Ok(())
}

fn new_srp(pin: &str) -> Result<Client<Sha512>> {
    Client::<Sha512>::new(
        Group::rfc3526_3072(),
        KeyKind::Plain,
        SRP_USER,
        pin.as_bytes(),
        &random::<32>(),
    )
}

/// PIN pairing, part 1: make the receiver show a PIN and fetch its salt and public value.
pub fn setup_pin_start<S: Read + Write>(conn: &mut Conn<S>) -> Result<PinSetup> {
    let r = conn.exchange(&req("/pair-pin-start", Hkp::Pin, &Tlv::new()))?;
    if r.status != 200 {
        return Err(SenderError::Status {
            status: r.status,
            what: "POST /pair-pin-start".into(),
        });
    }
    let m1 = Tlv::new().with(tlv::METHOD, [0]).with(tlv::SEQ_NO, [1]);
    let m2 = tlv_answer(
        &conn.exchange(&req("/pair-setup", Hkp::Pin, &m1))?,
        "/pair-setup (M1)",
    )?;
    seq_is(&m2, 2)?;
    Ok(PinSetup {
        salt: m2.require(tlv::SALT, "a salt")?.to_vec(),
        server_pub: m2.require(tlv::PUBLIC_KEY, "a public key")?.to_vec(),
    })
}

/// PIN pairing, part 2: prove the PIN (M3), exchange long-term keys (M5, M6).
/// Returns the credentials and whether the receiver's M6 signature verified.
pub fn setup_pin_finish<S: Read + Write>(
    conn: &mut Conn<S>,
    pending: PinSetup,
    pin: &str,
) -> Result<(Credentials, bool)> {
    if pin.is_empty() || pin.len() > 16 || !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(SenderError::Input("the PIN must be digits only".into()));
    }
    let mut srp = new_srp(pin)?;
    let res = srp.process(&pending.salt, &pending.server_pub)?;
    let m3 = Tlv::new()
        .with(tlv::SEQ_NO, [3])
        .with(tlv::PUBLIC_KEY, srp.public_key())
        .with(tlv::PROOF, res.proof);
    let m4 = tlv_answer(
        &conn.exchange(&req("/pair-setup", Hkp::Pin, &m3))?,
        "/pair-setup (M3)",
    )?;
    seq_is(&m4, 4)?;
    if let Some(p) = m4.get(tlv::PROOF) {
        if !srp.verify_server_proof(p) {
            return Err(SenderError::Auth(
                "the receiver's proof is wrong (wrong PIN?)".into(),
            ));
        }
    }
    let k = &res.session_key;

    let seed: [u8; 32] = random();
    let signing = SigningKey::from_bytes(&seed);
    let ltpk = signing.verifying_key().to_bytes();
    let pairing_id = uuid_text();
    let ios_x = hkdf32(
        k,
        "Pair-Setup-Controller-Sign-Salt",
        "Pair-Setup-Controller-Sign-Info",
    );
    let mut info = ios_x.to_vec();
    info.extend_from_slice(pairing_id.as_bytes());
    info.extend_from_slice(&ltpk);
    let inner = Tlv::new()
        .with(tlv::IDENTIFIER, pairing_id.as_bytes())
        .with(tlv::PUBLIC_KEY, ltpk)
        .with(tlv::SIGNATURE, signing.sign(&info).to_bytes());
    let session = hkdf32(k, "Pair-Setup-Encrypt-Salt", "Pair-Setup-Encrypt-Info");
    let m5 = Tlv::new().with(tlv::SEQ_NO, [5]).with(
        tlv::ENCRYPTED_DATA,
        seal_labelled(&session, b"PS-Msg05", &inner.encode())?,
    );
    let m6 = tlv_answer(
        &conn.exchange(&req("/pair-setup", Hkp::Pin, &m5))?,
        "/pair-setup (M5)",
    )?;
    seq_is(&m6, 6)?;
    let plain = open_labelled(
        &session,
        b"PS-Msg06",
        m6.require(tlv::ENCRYPTED_DATA, "encrypted data")?,
    )?;
    let acc = Tlv::decode(&plain)?;
    let acc_id = acc.require(tlv::IDENTIFIER, "an identifier")?;
    let acc_key = acc.require(tlv::PUBLIC_KEY, "a public key")?;
    let acc_sig = acc.require(tlv::SIGNATURE, "a signature")?;
    let acc_key: [u8; 32] = acc_key
        .try_into()
        .map_err(|_| SenderError::Protocol("receiver key is not 32 bytes".into()))?;

    // HomeKit says the accessory signs AccessoryX | id | key. pyatv does not check it.
    // UNCERTAIN: whether AirPlay receivers follow this; a failure is only reported.
    let acc_x = hkdf32(
        k,
        "Pair-Setup-Accessory-Sign-Salt",
        "Pair-Setup-Accessory-Sign-Info",
    );
    let mut acc_info = acc_x.to_vec();
    acc_info.extend_from_slice(acc_id);
    acc_info.extend_from_slice(&acc_key);
    let verified = VerifyingKey::from_bytes(&acc_key)
        .ok()
        .zip(Signature::from_slice(acc_sig).ok())
        .is_some_and(|(vk, sig)| vk.verify(&acc_info, &sig).is_ok());

    Ok((
        Credentials::Hap {
            pairing_id,
            seed: hex(&seed),
            device_id: hex(acc_id),
            device_key: hex(&acc_key),
        },
        verified,
    ))
}

/// Pair-verify with stored credentials; on success the connection switches to the
/// encrypted framing.
pub fn pair_verify<S: Read + Write>(conn: &mut Conn<S>, creds: &Credentials) -> Result<()> {
    let Credentials::Hap {
        pairing_id,
        seed,
        device_id,
        device_key,
    } = creds
    else {
        return Err(SenderError::Input("not HomeKit-style credentials".into()));
    };
    let seed: [u8; 32] = unhex(seed).ok_or_else(|| SenderError::Store("bad stored seed".into()))?;
    let device_key: [u8; 32] =
        unhex(device_key).ok_or_else(|| SenderError::Store("bad stored device key".into()))?;
    let device_id = unhex_vec(device_id, 256)
        .ok_or_else(|| SenderError::Store("bad stored device id".into()))?;
    let signing = SigningKey::from_bytes(&seed);

    let secret = StaticSecret::from(random::<32>());
    let our_pub = PublicKey::from(&secret).to_bytes();
    let m1 = Tlv::new()
        .with(tlv::SEQ_NO, [1])
        .with(tlv::PUBLIC_KEY, our_pub);
    let m2 = tlv_answer(
        &conn.exchange(&req("/pair-verify", Hkp::Pin, &m1))?,
        "/pair-verify (M1)",
    )?;
    seq_is(&m2, 2)?;
    let server_pub: [u8; 32] = m2
        .require(tlv::PUBLIC_KEY, "a public key")?
        .try_into()
        .map_err(|_| SenderError::Protocol("receiver key is not 32 bytes".into()))?;
    let shared = secret.diffie_hellman(&PublicKey::from(server_pub));
    if !shared.was_contributory() {
        return Err(SenderError::Auth(
            "the receiver's key is a weak point".into(),
        ));
    }
    let shared = shared.to_bytes();
    let session = hkdf32(
        &shared,
        "Pair-Verify-Encrypt-Salt",
        "Pair-Verify-Encrypt-Info",
    );
    let plain = open_labelled(
        &session,
        b"PV-Msg02",
        m2.require(tlv::ENCRYPTED_DATA, "encrypted data")?,
    )?;
    let inner = Tlv::decode(&plain)?;
    let id = inner.require(tlv::IDENTIFIER, "an identifier")?;
    if id != device_id.as_slice() {
        return Err(SenderError::Auth(
            "the receiver is not the one we paired with".into(),
        ));
    }
    let sig = Signature::from_slice(inner.require(tlv::SIGNATURE, "a signature")?)
        .map_err(|_| SenderError::Protocol("bad signature length".into()))?;
    let mut info = server_pub.to_vec();
    info.extend_from_slice(id);
    info.extend_from_slice(&our_pub);
    VerifyingKey::from_bytes(&device_key)
        .map_err(|_| SenderError::Store("stored device key is invalid".into()))?
        .verify(&info, &sig)
        .map_err(|_| SenderError::Auth("the receiver's signature is wrong".into()))?;

    let mut ours = our_pub.to_vec();
    ours.extend_from_slice(pairing_id.as_bytes());
    ours.extend_from_slice(&server_pub);
    let m3_inner = Tlv::new()
        .with(tlv::IDENTIFIER, pairing_id.as_bytes())
        .with(tlv::SIGNATURE, signing.sign(&ours).to_bytes());
    let m3 = Tlv::new().with(tlv::SEQ_NO, [3]).with(
        tlv::ENCRYPTED_DATA,
        seal_labelled(&session, b"PV-Msg03", &m3_inner.encode())?,
    );
    let m4 = tlv_answer(
        &conn.exchange(&req("/pair-verify", Hkp::Pin, &m3))?,
        "/pair-verify (M3)",
    )?;
    let _ = m4;
    conn.framed_mut().enable(control_cipher(&shared));
    Ok(())
}

fn uuid_text() -> String {
    let b: [u8; 16] = random();
    format!(
        "{}-{}-{}-{}-{}",
        hex(&b[0..4]),
        hex(&b[4..6]),
        hex(&b[6..8]),
        hex(&b[8..10]),
        hex(&b[10..16])
    )
    .to_ascii_uppercase()
}
