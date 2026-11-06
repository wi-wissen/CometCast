// SPDX-License-Identifier: GPL-3.0-or-later
//! The `OFFER` / `ANSWER` JSON of a mirroring session (namespace
//! `urn:x-cast:com.google.cast.webrtc`).
//!
//! Field names and defaults follow Open Screen, commit
//! `8b62b21479f1adb7663f182d97962b960c5c64ef` (BSD-3-Clause, Copyright 2019-2020 The
//! Chromium Authors; see THIRD_PARTY_NOTICES.md), files `cast/streaming/public/offer_messages.cc`,
//! `answer_messages.cc`, `cast/streaming/sender_message.cc`, `receiver_message.cc`,
//! `message_fields.h`, `public/sender_session.cc`, `impl/rtp_defines.h`, `public/constants.h`.
//!
//! UNCERTAIN: the exact set of fields Google's own mirroring receiver app needs or tolerates
//! was not checked against a device. Only video is offered (no audio).

use serde_json::{Value, json};

use crate::error::CastError;

/// `kRtpVideoTimebase`.
pub const VIDEO_TIMEBASE: u32 = 90_000;
/// `RtpPayloadType::kVideoH264`.
pub const PAYLOAD_TYPE_H264: u8 = 101;
/// `RtpPayloadType::kVideoHackForAndroidTV` (some Android TV receivers want this, per the
/// Open Screen comment; UNCERTAIN whether any current device does).
pub const PAYLOAD_TYPE_ANDROID_HACK: u8 = 96;
/// `kDefaultTargetPlayoutDelay` in milliseconds.
pub const DEFAULT_TARGET_DELAY_MS: u32 = 400;

/// What we offer for the single video stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoOffer {
    /// Stream index (audio streams would come first; video-only starts at 0).
    pub index: u32,
    /// Our SSRC. Open Screen draws normal-priority video SSRCs from 50001 to 100000.
    pub ssrc: u32,
    /// RTP payload type.
    pub payload_type: u8,
    /// AES-128 key for the frame payload.
    pub aes_key: [u8; 16],
    /// IV mask the frame nonce is XORed with.
    pub aes_iv_mask: [u8; 16],
    /// Playout delay in milliseconds.
    pub target_delay_ms: u32,
    /// Largest picture we will send.
    pub width: u32,
    /// See `width`.
    pub height: u32,
    /// Maximum frame rate.
    pub max_fps: u32,
    /// Maximum bit rate in bits per second.
    pub max_bitrate_bps: u32,
}

impl VideoOffer {
    /// A fresh offer with random SSRC, key and IV mask.
    pub fn new(
        width: u32,
        height: u32,
        max_fps: u32,
        max_bitrate_bps: u32,
        android_rtp_hack: bool,
    ) -> Self {
        Self {
            index: 0,
            ssrc: rand::random_range(50_001..=100_000),
            payload_type: if android_rtp_hack {
                PAYLOAD_TYPE_ANDROID_HACK
            } else {
                PAYLOAD_TYPE_H264
            },
            aes_key: rand::random(),
            aes_iv_mask: rand::random(),
            target_delay_ms: DEFAULT_TARGET_DELAY_MS,
            width,
            height,
            max_fps,
            max_bitrate_bps,
        }
    }

    /// The JSON of the stream, as `VideoStream::ToJson` writes it.
    pub fn stream_json(&self) -> Value {
        json!({
            "index": self.index,
            "type": "video_source",
            "channels": 1,
            "rtpPayloadType": self.payload_type,
            "rtpProfile": "cast",
            "ssrc": self.ssrc,
            "targetDelay": self.target_delay_ms,
            "aesKey": hex(&self.aes_key),
            "aesIvMask": hex(&self.aes_iv_mask),
            "receiverRtcpEventLog": true,
            "timeBase": format!("1/{VIDEO_TIMEBASE}"),
            "codecParameter": "",
            "codecName": "h264",
            "maxFrameRate": self.max_fps.to_string(),
            "maxBitRate": self.max_bitrate_bps,
            "protection": "",
            "profile": "",
            "level": "",
            "errorRecoveryMode": "",
            "resolutions": [{"width": self.width, "height": self.height}],
        })
    }
}

/// The complete `OFFER` message body.
pub fn offer_message(offer: &VideoOffer, seq_num: i64) -> Value {
    json!({
        "type": "OFFER",
        "seqNum": seq_num,
        "offer": {
            "castMode": "mirroring",
            "supportedStreams": [offer.stream_json()],
        },
    })
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02X}");
        s
    })
}

/// The parts of the `ANSWER` this sender uses.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    /// UDP port of the receiver for RTP and RTCP.
    pub udp_port: u16,
    /// Which offered stream indexes the receiver accepted.
    pub send_indexes: Vec<i64>,
    /// The receiver's SSRC for each accepted stream, in the same order.
    pub ssrcs: Vec<u32>,
    /// `constraints` (max bit rate, dimensions, ...), unparsed. UNCERTAIN: not applied.
    pub constraints: Option<Value>,
    /// `display`, unparsed.
    pub display: Option<Value>,
}

/// Outcome of looking at a message on the webrtc namespace.
#[derive(Debug, Clone, PartialEq)]
pub enum AnswerMessage {
    /// An `ANSWER` with `result: ok`.
    Ok {
        /// The `seqNum` it answers.
        seq_num: i64,
        /// The parsed answer.
        answer: Answer,
    },
    /// An `ANSWER` with an error.
    Error {
        /// The `seqNum` it answers (-1 if missing).
        seq_num: i64,
        /// Text made from `error.code` and `error.description`.
        description: String,
    },
}

/// Parse a message from the webrtc namespace. `None` means: not an ANSWER.
pub fn parse_answer(text: &str) -> Option<Result<AnswerMessage, CastError>> {
    let v: Value = serde_json::from_str(text).ok()?;
    if !v.get("type")?.as_str()?.eq_ignore_ascii_case("ANSWER") {
        return None;
    }
    let seq_num = v.get("seqNum").and_then(Value::as_i64).unwrap_or(-1);
    // Open Screen: a message without `result: ok` is an error.
    if v.get("result").and_then(Value::as_str) != Some("ok") {
        let e = v.get("error");
        let code = e
            .and_then(|e| e.get("code"))
            .map(Value::to_string)
            .unwrap_or_default();
        let desc = e
            .and_then(|e| e.get("description"))
            .and_then(Value::as_str)
            .unwrap_or("");
        return Some(Ok(AnswerMessage::Error {
            seq_num,
            description: format!("code {code}: {desc}"),
        }));
    }
    let bad = |m: &str| Some(Err(CastError::Protocol(format!("invalid ANSWER: {m}"))));
    let Some(a) = v.get("answer") else {
        return bad("no answer object");
    };
    let Some(port) = a
        .get("udpPort")
        .and_then(Value::as_u64)
        .filter(|p| (1..=65_535).contains(p))
    else {
        return bad("missing or bad udpPort");
    };
    let Some(send_indexes) = a
        .get("sendIndexes")
        .and_then(Value::as_array)
        .map(|x| x.iter().filter_map(Value::as_i64).collect::<Vec<_>>())
    else {
        return bad("missing sendIndexes");
    };
    let Some(ssrcs) = a.get("ssrcs").and_then(Value::as_array).map(|x| {
        x.iter()
            .filter_map(|n| n.as_u64().and_then(|n| u32::try_from(n).ok()))
            .collect::<Vec<_>>()
    }) else {
        return bad("missing ssrcs");
    };
    if send_indexes.is_empty() || ssrcs.is_empty() || send_indexes.iter().any(|i| *i < 0) {
        return bad("empty or negative sendIndexes / ssrcs");
    }
    Some(Ok(AnswerMessage::Ok {
        seq_num,
        answer: Answer {
            udp_port: port as u16,
            send_indexes,
            ssrcs,
            constraints: a.get("constraints").cloned(),
            display: a.get("display").cloned(),
        },
    }))
}

impl Answer {
    /// The receiver SSRC for the stream with this offer index, if the receiver accepted it.
    pub fn receiver_ssrc_for(&self, index: u32) -> Option<u32> {
        self.send_indexes
            .iter()
            .position(|i| *i == i64::from(index))
            .and_then(|p| self.ssrcs.get(p).copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer() -> VideoOffer {
        VideoOffer {
            index: 0,
            ssrc: 60_000,
            payload_type: PAYLOAD_TYPE_H264,
            aes_key: [0xab; 16],
            aes_iv_mask: [0x01; 16],
            target_delay_ms: 400,
            width: 1280,
            height: 720,
            max_fps: 30,
            max_bitrate_bps: 4_000_000,
        }
    }

    #[test]
    fn offer_has_the_documented_fields() {
        let m = offer_message(&offer(), 7);
        assert_eq!(m["type"], "OFFER");
        assert_eq!(m["seqNum"], 7);
        assert_eq!(m["offer"]["castMode"], "mirroring");
        let s = &m["offer"]["supportedStreams"][0];
        assert_eq!(s["type"], "video_source");
        assert_eq!(s["codecName"], "h264");
        assert_eq!(s["rtpPayloadType"], 101);
        assert_eq!(s["rtpProfile"], "cast");
        assert_eq!(s["timeBase"], "1/90000");
        assert_eq!(s["aesKey"], "AB".repeat(16));
        assert_eq!(s["aesIvMask"], "01".repeat(16));
        assert_eq!(s["maxFrameRate"], "30");
        assert_eq!(s["resolutions"][0]["width"], 1280);
    }

    #[test]
    fn parses_ok_answer() {
        let t = r#"{"type":"ANSWER","seqNum":7,"result":"ok","answer":
            {"udpPort":2344,"sendIndexes":[0],"ssrcs":[12345],"constraints":{"video":{"maxBitRate":5000000}}}}"#;
        let Some(Ok(AnswerMessage::Ok { seq_num, answer })) = parse_answer(t) else {
            panic!()
        };
        assert_eq!(seq_num, 7);
        assert_eq!(answer.udp_port, 2344);
        assert_eq!(answer.receiver_ssrc_for(0), Some(12_345));
        assert_eq!(answer.receiver_ssrc_for(1), None);
        assert!(answer.constraints.is_some());
    }

    #[test]
    fn parses_error_answer_and_ignores_other_types() {
        let t = r#"{"type":"ANSWER","seqNum":3,"result":"error","error":{"code":5,"description":"nope"}}"#;
        let Some(Ok(AnswerMessage::Error {
            seq_num,
            description,
        })) = parse_answer(t)
        else {
            panic!()
        };
        assert_eq!(seq_num, 3);
        assert!(description.contains("nope"));
        assert!(parse_answer(r#"{"type":"STATUS_RESPONSE"}"#).is_none());
        assert!(parse_answer("not json").is_none());
    }

    #[test]
    fn rejects_incomplete_answer() {
        let t = r#"{"type":"ANSWER","seqNum":1,"result":"ok","answer":{"udpPort":0,"sendIndexes":[0],"ssrcs":[1]}}"#;
        assert!(matches!(parse_answer(t), Some(Err(_))));
        let t = r#"{"type":"ANSWER","seqNum":1,"result":"ok","answer":{"udpPort":5,"sendIndexes":[],"ssrcs":[]}}"#;
        assert!(matches!(parse_answer(t), Some(Err(_))));
    }

    #[test]
    fn random_offer_uses_open_screen_ranges() {
        let o = VideoOffer::new(1920, 1080, 30, 5_000_000, false);
        assert!((50_001..=100_000).contains(&o.ssrc));
        assert_eq!(o.payload_type, 101);
        assert_eq!(VideoOffer::new(1, 1, 1, 1, true).payload_type, 96);
    }
}
