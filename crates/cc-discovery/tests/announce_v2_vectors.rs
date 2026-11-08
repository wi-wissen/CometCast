// SPDX-License-Identifier: GPL-3.0-or-later
//! The shared vectors `tests/fixtures/announce-v2.json` for signed display announcements. The
//! file was made by an independent implementation (`tools/gen-announce-v2-vectors.py`); the
//! Swift tests of the Apple TV core read the same file.

use cc_discovery::signed::{
    AnnounceState, DisplayKey, Reject, SIGNED_KEYS, SeqCounter, sign_record, signed_bytes, verify,
};
use cc_discovery::{NearbyTracker, RawRecord};
use serde_json::Value;
use tokio::time::Instant;

fn fixture() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/announce-v2.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

fn txt(v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p[0].as_str().unwrap().to_owned(),
                p[1].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn key_of(f: &Value, name: &str) -> DisplayKey {
    let k = f["keys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["name"] == name)
        .unwrap();
    DisplayKey::from_secret(&hex(k["seed_hex"].as_str().unwrap())).unwrap()
}

#[test]
fn keys_ids_and_tags_match() {
    let f = fixture();
    for k in f["keys"].as_array().unwrap() {
        let key = DisplayKey::from_secret(&hex(k["seed_hex"].as_str().unwrap())).unwrap();
        assert_eq!(
            key.public_key().to_vec(),
            hex(k["public_key_hex"].as_str().unwrap())
        );
        assert_eq!(key.public_key_b64(), k["pk"].as_str().unwrap());
        assert_eq!(key.id(), k["id"].as_str().unwrap());
        assert_eq!(key.tag(), k["tag"].as_str().unwrap());
    }
}

#[test]
fn valid_records_are_reproduced_byte_for_byte_and_verify() {
    let f = fixture();
    let valid = f["valid"].as_array().unwrap();
    assert!(valid.len() >= 7);
    for v in valid {
        let name = v["name"].as_str().unwrap();
        let key = key_of(&f, v["key"].as_str().unwrap());
        let seq = v["seq"].as_u64().unwrap();
        let state = AnnounceState::parse(v["state"].as_str().unwrap()).unwrap();
        let fields: Vec<(&str, &str)> = v["fields"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str().unwrap()))
            .collect();
        // The TXT built here equals the vector (same bytes, same signature, same order).
        let built = sign_record(&key, seq, state, &fields);
        assert_eq!(built, txt(&v["txt"]), "{name}: txt");
        assert_eq!(
            built.last().unwrap().1,
            v["sig"].as_str().unwrap(),
            "{name}: sig"
        );
        // The signed bytes equal the vector.
        let lookup = |k: &str| {
            built
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(
            signed_bytes(lookup),
            hex(v["signed_bytes_hex"].as_str().unwrap()),
            "{name}: signed bytes"
        );
        // And the vector verifies.
        let ok = verify(&txt(&v["txt"])).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!((ok.seq, ok.state), (seq, state), "{name}");
        assert_eq!(ok.id, key.id());
    }
    assert_eq!(SIGNED_KEYS.len(), 10);
}

#[test]
fn refused_records_are_refused_for_the_documented_reason() {
    let f = fixture();
    let refused = f["refused"].as_array().unwrap();
    assert!(refused.len() >= 15);
    for r in refused {
        let name = r["name"].as_str().unwrap();
        let expect = match r["reason"].as_str().unwrap() {
            "signature" => Reject::Signature,
            "id_mismatch" => Reject::IdMismatch,
            "malformed" => Reject::Malformed,
            "version" => Reject::Version,
            other => panic!("unknown reason {other}"),
        };
        assert_eq!(verify(&txt(&r["txt"])), Err(expect), "{name}");
        // A refused record never reaches a row.
        let mut t = NearbyTracker::new();
        let rec = RawRecord {
            instance: "x".into(),
            addresses: vec![],
            txt: txt(&r["txt"]),
        };
        assert!(t.on_seen(&rec, Instant::now()).is_empty(), "{name}");
        assert!(t.screens().is_empty(), "{name}");
    }
}

#[test]
fn seq_cases_follow_the_highest_accepted_seq() {
    let f = fixture();
    for case in f["seq_cases"].as_array().unwrap() {
        let mut t = NearbyTracker::new();
        let now = Instant::now();
        for step in case["steps"].as_array().unwrap() {
            let name = step["record"].as_str().unwrap();
            let v = f["valid"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["name"] == name)
                .unwrap();
            let seq = v["seq"].as_u64().unwrap();
            let before = t.highest_seq(&key_of(&f, "A").id());
            let rec = RawRecord {
                instance: "x".into(),
                addresses: vec![],
                txt: txt(&v["txt"]),
            };
            t.on_seen(&rec, now);
            let after = t.highest_seq(&key_of(&f, "A").id());
            let accepted = after == Some(seq) && before != after;
            assert_eq!(
                accepted,
                step["accepted"].as_bool().unwrap(),
                "{}: {name}",
                case["name"]
            );
        }
    }
}

#[test]
fn seq_counter_vectors() {
    let f = fixture();
    for c in f["seq_counter"].as_array().unwrap() {
        let mut counter = SeqCounter::new(c["floor"].as_u64().unwrap());
        let got: Vec<u64> = c["clock"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| counter.next(t.as_u64().unwrap()))
            .collect();
        let expect: Vec<u64> = c["expect"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_u64().unwrap())
            .collect();
        assert_eq!(got, expect);
    }
}
