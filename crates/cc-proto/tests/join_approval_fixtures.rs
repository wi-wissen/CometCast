// SPDX-License-Identifier: GPL-3.0-or-later
//! The fixtures in `tests/fixtures/join-approval/` (for other implementations, such as the
//! Swift client) must stay true: every message parses and serialises back to the same JSON, and
//! the name vectors give the documented results.

use std::path::PathBuf;

use cc_proto::{ClientMsg, ServerMsg, sanitize_display_name};
use serde_json::Value;

fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/join-approval")
        .join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    serde_json::from_str(&text).unwrap()
}

/// Parse as the type of its direction and serialise again: the JSON must be the same value.
fn round_trip(direction: &str, message: &Value) {
    let text = message.to_string();
    let back = match direction {
        "client_to_server" => {
            let m: ClientMsg =
                serde_json::from_str(&text).unwrap_or_else(|e| panic!("{text}: {e}"));
            serde_json::to_value(&m).unwrap()
        }
        "server_to_client" => {
            let m: ServerMsg =
                serde_json::from_str(&text).unwrap_or_else(|e| panic!("{text}: {e}"));
            serde_json::to_value(&m).unwrap()
        }
        other => panic!("unknown direction {other}"),
    };
    assert_eq!(&back, message, "round trip of {text}");
}

#[test]
fn every_message_fixture_round_trips() {
    let f = fixture("messages.json");
    let list = f["messages"].as_array().unwrap();
    assert!(list.len() > 30);
    for m in list {
        round_trip(m["direction"].as_str().unwrap(), &m["message"]);
    }
}

#[test]
fn every_flow_step_is_a_valid_message() {
    let f = fixture("flows.json");
    for flow in f["flows"].as_array().unwrap() {
        for s in flow["steps"].as_array().unwrap() {
            let from = s["from"].as_str().unwrap();
            let direction = if from == "server" {
                "server_to_client"
            } else {
                "client_to_server"
            };
            let msg = &s["message"];
            // Relayed offers have the shape of a client message; skip the server copy.
            if from == "server" && msg["type"] == "offer" {
                continue;
            }
            round_trip(direction, msg);
        }
    }
}

#[test]
fn name_vectors_hold() {
    let f = fixture("name-sanitising.json");
    assert_eq!(f["max_chars"], 40);
    for v in f["vectors"].as_array().unwrap() {
        let input = v["input"].as_str();
        let n = v["guest_number"].as_u64().unwrap() as u32;
        assert_eq!(
            sanitize_display_name(input, n),
            v["expected"].as_str().unwrap(),
            "{}",
            v["note"]
        );
    }
}
