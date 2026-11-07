// SPDX-License-Identifier: GPL-3.0-or-later
//! The shared fixtures in `<repository>/tests/fixtures` against this crate. The same files are checked against the
//! web client (apps/web/tests/fixtures.test.ts) and the Apple TV receiver (apps/apple-tv/CometCastCore/Tests).
//! A case may carry `"expect_rust"` for a known and documented difference of this crate from the web client
//! (the web client's answer stays in `"expect"`).

use cc_files::wire::{MAX_DOCUMENT_BYTES, MAX_IMAGE_BYTES, parse_message, validate_link};
use serde_json::Value;
use std::path::PathBuf;

fn load(name: &str) -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name);
    serde_json::from_str(
        &std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())),
    )
    .unwrap()
}

fn input(c: &Value) -> String {
    if let Some(s) = c.get("input").and_then(Value::as_str) {
        return s.to_string();
    }
    let mut out = String::new();
    for p in c["input_parts"].as_array().unwrap() {
        match p {
            Value::String(s) => out.push_str(s),
            Value::Object(o) => out.push_str(
                &o["repeat"]
                    .as_str()
                    .unwrap()
                    .repeat(o["count"].as_u64().unwrap() as usize),
            ),
            _ => {}
        }
    }
    out
}

fn expected(c: &Value) -> &Value {
    c.get("expect_rust").unwrap_or(&c["expect"])
}

#[test]
fn control_parse_matches_the_fixtures() {
    let f = load("control-parse.json");
    let mut bad = Vec::new();
    for c in f["cases"].as_array().unwrap() {
        let got: Value = match parse_message(&input(c), MAX_IMAGE_BYTES, MAX_DOCUMENT_BYTES) {
            Ok(m) => serde_json::from_str(&m.to_json()).unwrap(),
            Err(_) => Value::Null,
        };
        if &got != expected(c) {
            bad.push(format!(
                "{}: got {got}, fixture says {}",
                c["name"],
                expected(c)
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} differences:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

#[test]
fn link_validation_matches_the_fixtures() {
    let f = load("link-url.json");
    let mut bad = Vec::new();
    for c in f["cases"].as_array().unwrap() {
        let got = validate_link(&input(c)).map_or(Value::Null, Value::String);
        if &got != expected(c) {
            bad.push(format!(
                "{}: got {got}, fixture says {}",
                c["name"],
                expected(c)
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} differences:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
