// SPDX-License-Identifier: GPL-3.0-or-later
//! The shared vectors `tests/fixtures/join-proof.json` for the presenter device proof. The file
//! was made by an independent implementation (`tools/gen-join-proof-vectors.py`); the web and
//! the Apple TV tests read the same file.

use cc_proto::JoinDevice;
use cc_transport::device::{
    JOIN_WINDOW_MS, JoinReject, device_id_of_secret, join_signed_bytes, public_key_of, sign_join,
    verify_join,
};
use serde_json::Value;

fn fixture() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/join-proof.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

fn secret_of(f: &Value, name: &str) -> [u8; 32] {
    let k = f["keys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["name"] == name)
        .unwrap();
    hex(k["seed_hex"].as_str().unwrap()).try_into().unwrap()
}

/// The shape check of the server and the display: anything that is not a well-formed device.
fn check(device: &Value, code: &str, now: u64) -> Result<String, JoinReject> {
    let d: JoinDevice =
        serde_json::from_value(device.clone()).map_err(|_| JoinReject::Malformed)?;
    verify_join(&d, code, now)
}

#[test]
fn keys_and_ids_match() {
    let f = fixture();
    assert_eq!(f["window_ms"].as_u64().unwrap(), JOIN_WINDOW_MS);
    for k in f["keys"].as_array().unwrap() {
        let secret: [u8; 32] = hex(k["seed_hex"].as_str().unwrap()).try_into().unwrap();
        assert_eq!(
            public_key_of(&secret).to_vec(),
            hex(k["public_key_hex"].as_str().unwrap())
        );
        assert_eq!(device_id_of_secret(&secret), k["id"].as_str().unwrap());
    }
}

#[test]
fn signed_bytes_and_signatures_match_the_independent_implementation() {
    let f = fixture();
    for v in f["valid"].as_array().unwrap() {
        let name = v["name"].as_str().unwrap();
        let code = v["signed_for_code"].as_str().unwrap();
        let ts = v["device"]["ts"].as_u64().unwrap();
        assert_eq!(
            join_signed_bytes(code, ts),
            hex(v["signed_bytes_hex"].as_str().unwrap()),
            "{name}"
        );
        // Ed25519 is deterministic: our signature is byte for byte the vector's.
        let ours = sign_join(&secret_of(&f, v["key"].as_str().unwrap()), code, ts);
        assert_eq!(serde_json::to_value(&ours).unwrap(), v["device"], "{name}");
    }
}

#[test]
fn valid_proofs_verify_to_their_device_id() {
    let f = fixture();
    for v in f["valid"].as_array().unwrap() {
        let got = check(
            &v["device"],
            v["code"].as_str().unwrap(),
            v["now_ms"].as_u64().unwrap(),
        );
        assert_eq!(
            got.as_deref(),
            Ok(v["device_id"].as_str().unwrap()),
            "{}",
            v["name"]
        );
    }
}

#[test]
fn refused_proofs_are_refused_for_the_documented_reason() {
    let f = fixture();
    for r in f["refused"].as_array().unwrap() {
        let got = check(
            &r["device"],
            r["code"].as_str().unwrap(),
            r["now_ms"].as_u64().unwrap(),
        );
        let want = match r["reason"].as_str().unwrap() {
            "malformed" => JoinReject::Malformed,
            "window" => JoinReject::Window,
            "signature" => JoinReject::Signature,
            other => panic!("unknown reason {other}"),
        };
        assert_eq!(got, Err(want), "{}", r["name"]);
    }
}
