// SPDX-License-Identifier: GPL-3.0-or-later
//! Legacy AirPlay PIN pairing and pair-verify (the "AirPlay 1 style" that UxPlay also
//! speaks). Source for the message flow: pyatv `protocols/airplay/auth/legacy.py` and
//! `protocols/airplay/srp.py` (MIT), read, not copied.
//!
//! Flow: `POST /pair-pin-start` (the receiver shows a PIN), `POST /pair-setup-pin` three
//! times (SRP-6a with SHA-1 and the 2048-bit group; plists with `method`/`user`, then
//! `pk`/`proof`, then `epk`/`authTag`), later `POST /pair-verify` twice (X25519 key
//! exchange, Ed25519 signature encrypted with AES-128-CTR).
//!
//! The receiver is authenticated by trust on first use: credentials may hold the receiver's
//! Ed25519 public key as it was announced when the pairing was made
//! ([`Credentials::with_receiver_key`]); pair-verify then checks the receiver's signature
//! against it and fails with [`RECEIVER_KEY_MISMATCH`] when another key signed.
//!
//! UNCERTAIN: the exact behaviour of Apple receivers; tested only against UxPlay 1.68 and
//! against our own test fake.

use std::io::{Read, Write};

use aes::Aes128;
use aes::cipher::{KeyIvInit, StreamCipher};
use aes_gcm::AesGcm;
use aes_gcm::aead::consts::U16;
use aes_gcm::aead::{Aead, KeyInit, Nonce};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use plist::{Dictionary, Value};
use sha1::Sha1;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::creds::Credentials;
use crate::cryptoutil::{hex, random, sha512, unhex};
use crate::error::{Result, SenderError};
use crate::http::{Conn, Request, Response};
use crate::plistx;
use crate::srp::{Client, Group, KeyKind};
use zeroize::Zeroizing;

const USER_AGENT: &str = "AirPlay/320.20";

/// The text of the [`SenderError::Auth`] error of [`pair_verify`] when the receiver did not
/// sign with the key that was remembered at pairing (see [`is_receiver_key_mismatch`]).
pub const RECEIVER_KEY_MISMATCH: &str = "the receiver did not prove the key it had when it was paired (another device answers, or the receiver was reset); pair it again";

/// Whether an error says that the receiver's signature did not match the remembered key.
/// The pairing can be made again with the PIN (that authenticates the receiver anew).
pub fn is_receiver_key_mismatch(e: &SenderError) -> bool {
    matches!(e, SenderError::Auth(m) if m == RECEIVER_KEY_MISMATCH)
}

type Aes128Gcm16 = AesGcm<Aes128, U16>;
type Aes128Ctr = ctr::Ctr128BE<Aes128>;

fn base(path: &str) -> Request {
    Request::new("POST", path)
        .header("User-Agent", USER_AGENT)
        .header("Connection", "keep-alive")
}

fn expect_ok(resp: &Response, what: &str) -> Result<()> {
    if resp.status == 200 {
        Ok(())
    } else {
        Err(SenderError::Status {
            status: resp.status,
            what: what.into(),
        })
    }
}

/// Step 0: ask the receiver to show its PIN.
pub fn pair_pin_start<S: Read + Write>(conn: &mut Conn<S>) -> Result<()> {
    let resp = conn.exchange(&base("/pair-pin-start"))?;
    expect_ok(&resp, "POST /pair-pin-start")
}

fn plist_post<S: Read + Write>(
    conn: &mut Conn<S>,
    path: &str,
    pairs: Vec<(&str, Value)>,
) -> Result<Dictionary> {
    let mut d = Dictionary::new();
    for (k, v) in pairs {
        d.insert(k.to_owned(), v);
    }
    let req = base(path).body(plistx::BPLIST_TYPE, plistx::encode(d)?);
    let resp = conn.exchange(&req)?;
    if resp.status == 470 || resp.status == 403 {
        return Err(SenderError::Auth(format!(
            "the receiver refused {path} (status {}); wrong PIN?",
            resp.status
        )));
    }
    expect_ok(&resp, &format!("POST {path}"))?;
    if resp.body.is_empty() {
        return Ok(Dictionary::new());
    }
    plistx::decode_dict(&resp.body)
}

/// Steps 1 to 3: finish pairing with the PIN that the receiver shows.
pub fn pair_finish<S: Read + Write>(conn: &mut Conn<S>, pin: &str) -> Result<Credentials> {
    if pin.is_empty() || pin.len() > 16 || !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(SenderError::Input("the PIN must be digits only".into()));
    }
    let seed: Zeroizing<[u8; 32]> = Zeroizing::new(random());
    let client_id = hex(&random::<8>()).to_ascii_uppercase();
    let signing = SigningKey::from_bytes(&seed);
    let auth_public = signing.verifying_key().to_bytes();

    let mut srp = Client::<Sha1>::new(
        Group::rfc5054_2048(),
        KeyKind::Interleaved,
        client_id.as_bytes(),
        pin.as_bytes(),
        &random::<32>(),
    )?;

    // Step 1: announce the method and our client id; the answer has the receiver's
    // public value and the salt.
    let a1 = plist_post(
        conn,
        "/pair-setup-pin",
        vec![
            ("method", Value::String("pin".into())),
            ("user", Value::String(client_id.clone())),
        ],
    )?;
    let pk = plistx::data(&a1, "pk")
        .ok_or_else(|| SenderError::Protocol("answer 1 has no pk".into()))?;
    let salt = plistx::data(&a1, "salt")
        .ok_or_else(|| SenderError::Protocol("answer 1 has no salt".into()))?;
    let res = srp.process(salt, pk)?;

    // Step 2: our public value and proof.
    let a2 = plist_post(
        conn,
        "/pair-setup-pin",
        vec![
            ("pk", Value::Data(srp.public_key())),
            ("proof", Value::Data(res.proof.clone())),
        ],
    )?;
    // pyatv ignores the receiver's proof. We require it and check it (a receiver that
    // sends none cannot be told from a man in the middle that knows no PIN).
    // UNCERTAIN: whether every Apple receiver sends it; UxPlay is covered by the interop test.
    let m2 = plistx::data(&a2, "proof")
        .ok_or_else(|| SenderError::Protocol("answer 2 has no receiver proof".into()))?;
    if !srp.verify_server_proof(m2) {
        return Err(SenderError::Auth(
            "the receiver's proof is wrong (wrong PIN or a man in the middle)".into(),
        ));
    }

    // Step 3: our Ed25519 public key, encrypted with AES-128-GCM under a key from K.
    let aes_key = &sha512(&[b"Pair-Setup-AES-Key", &res.session_key])[..16];
    let mut iv = [0u8; 16];
    iv.copy_from_slice(&sha512(&[b"Pair-Setup-AES-IV", &res.session_key])[..16]);
    // pyatv: "last byte must be increased by 1".
    iv[15] = iv[15].wrapping_add(1);
    let gcm = Aes128Gcm16::new_from_slice(aes_key)
        .map_err(|_| SenderError::Auth("bad AES key length".into()))?;
    let sealed = gcm
        .encrypt(
            Nonce::<Aes128Gcm16>::from_slice(&iv),
            auth_public.as_slice(),
        )
        .map_err(|_| SenderError::Auth("encryption failed".into()))?;
    let (epk, tag) = sealed.split_at(sealed.len() - 16);
    plist_post(
        conn,
        "/pair-setup-pin",
        vec![
            ("epk", Value::Data(epk.to_vec())),
            ("authTag", Value::Data(tag.to_vec())),
        ],
    )?;

    // The receiver's long-term key is not part of what we read from the answers; the caller
    // adds the announced one with `Credentials::with_receiver_key`.
    // UNCERTAIN: UxPlay is said to answer step 3 with its own key, encrypted like ours; that
    // would be a source bound to the PIN, but neither pyatv nor our test fake has it.
    Ok(Credentials::Legacy {
        client_id,
        seed: hex(seed.as_slice()),
        receiver_key: None,
    })
}

/// Pair-verify with stored credentials (two POSTs). Afterwards the receiver treats this
/// connection as paired.
///
/// With a receiver key in the credentials (remembered at pairing, trust on first use) the
/// receiver's signature is checked and a mismatch ends the exchange with the error
/// [`RECEIVER_KEY_MISMATCH`] before we prove anything. Without one (credentials of older
/// versions, or a receiver that announced no key) it cannot be checked: then nothing shows
/// that the device that answers is the one that was paired (pyatv does not check it either).
///
/// The signed message is the receiver's X25519 public key followed by ours, the mirror of
/// what we sign. UNCERTAIN: this order is what our test fake does and what UxPlay's
/// `pairing_session_get_signature` is remembered to do; it was not run against a real
/// receiver with a remembered key.
pub fn pair_verify<S: Read + Write>(conn: &mut Conn<S>, creds: &Credentials) -> Result<()> {
    let Credentials::Legacy {
        seed, receiver_key, ..
    } = creds
    else {
        return Err(SenderError::Input("not legacy credentials".into()));
    };
    let receiver_key = match receiver_key {
        Some(k) => Some(
            unhex::<32>(k)
                .and_then(|b| VerifyingKey::from_bytes(&b).ok())
                .ok_or_else(|| SenderError::Store("stored receiver key is not a key".into()))?,
        ),
        None => None,
    };
    let seed: Zeroizing<[u8; 32]> = Zeroizing::new(
        unhex(seed).ok_or_else(|| SenderError::Store("stored seed is not 64 hex digits".into()))?,
    );
    let signing = SigningKey::from_bytes(&seed);
    let auth_public = signing.verifying_key().to_bytes();
    let secret = StaticSecret::from(random::<32>());
    let our_pub = PublicKey::from(&secret).to_bytes();

    let mut first = vec![1, 0, 0, 0];
    first.extend_from_slice(&our_pub);
    first.extend_from_slice(&auth_public);
    let r1 = conn.exchange(
        &base("/pair-verify")
            .header("Content-Type", "application/octet-stream")
            .with_body(first),
    )?;
    expect_ok(&r1, "POST /pair-verify (1)")?;
    if r1.body.len() != 96 {
        return Err(SenderError::Protocol(format!(
            "pair-verify answer has {} bytes, expected 96",
            r1.body.len()
        )));
    }
    let mut server_pub = [0u8; 32];
    server_pub.copy_from_slice(&r1.body[..32]);
    let shared = secret.diffie_hellman(&PublicKey::from(server_pub));
    if !shared.was_contributory() {
        return Err(SenderError::Auth(
            "the receiver's key is a weak point".into(),
        ));
    }
    let shared = shared.to_bytes();
    let key = &sha512(&[b"Pair-Verify-AES-Key", &shared])[..16];
    let iv = &sha512(&[b"Pair-Verify-AES-IV", &shared])[..16];

    let mut to_sign = Vec::with_capacity(64);
    to_sign.extend_from_slice(&our_pub);
    to_sign.extend_from_slice(&server_pub);
    let mut signature = signing.sign(&to_sign).to_bytes();
    let mut cipher = Aes128Ctr::new_from_slices(key, iv)
        .map_err(|_| SenderError::Auth("bad AES key length".into()))?;
    // The keystream first runs over the receiver's 64 encrypted bytes (its signature), then
    // encrypts our signature. This is what pyatv does (without looking at the signature).
    let mut theirs = [0u8; 64];
    theirs.copy_from_slice(&r1.body[32..]);
    cipher.apply_keystream(&mut theirs);
    if let Some(key) = receiver_key {
        let mut signed = Vec::with_capacity(64);
        signed.extend_from_slice(&server_pub);
        signed.extend_from_slice(&our_pub);
        if key
            .verify_strict(&signed, &Signature::from_bytes(&theirs))
            .is_err()
        {
            return Err(SenderError::Auth(RECEIVER_KEY_MISMATCH.into()));
        }
    }
    cipher.apply_keystream(&mut signature);

    let mut second = vec![0, 0, 0, 0];
    second.extend_from_slice(&signature);
    let r2 = conn.exchange(
        &base("/pair-verify")
            .header("Content-Type", "application/octet-stream")
            .with_body(second),
    )?;
    expect_ok(&r2, "POST /pair-verify (2)")
}

/// Parse the stored client id and seed (used by tests and the CLI to show something safe).
pub fn client_id(creds: &Credentials) -> Option<&str> {
    match creds {
        Credentials::Legacy { client_id, .. } => Some(client_id),
        _ => None,
    }
}
