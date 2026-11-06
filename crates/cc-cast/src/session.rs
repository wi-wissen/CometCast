// SPDX-License-Identifier: GPL-3.0-or-later
//! The sender state machine up to the point where media can flow:
//!
//! 1. TCP + TLS to the receiver (port 8009), self-signed certificate recorded,
//! 2. device authentication ([`crate::device_auth`]),
//! 3. `CONNECT` to `receiver-0`, heartbeat starts,
//! 4. `GET_STATUS`, then `LAUNCH` of the mirroring app, wait for `RECEIVER_STATUS` with the
//!    app's `sessionId` and `transportId` (or `LAUNCH_ERROR`),
//! 5. `CONNECT` to the app's transport id,
//! 6. `OFFER` on `urn:x-cast:com.google.cast.webrtc`, wait for `ANSWER`.
//!
//! The flow, message names and JSON fields follow
//! `cast/standalone_sender/looping_file_cast_agent.cc`, `cast/sender/channel/message_util.cc`,
//! `cast/common/channel/message_util.cc`, `cast/common/channel/connection_namespace_handler.cc`
//! and `cast/streaming/public/sender_session.cc` of Open Screen, commit
//! `8b62b21479f1adb7663f182d97962b960c5c64ef` (BSD-3-Clause, Copyright 2019-2020 The Chromium
//! Authors; see THIRD_PARTY_NOTICES.md).
//!
//! UNCERTAIN: nothing here has run against a real Cast device (measurement point A5).

use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::channel::{
    CONNECTION_NS, Connection, Heartbeat, PLATFORM_RECEIVER, PLATFORM_SENDER, RECEIVER_NS,
    WEBRTC_NS, json_message,
};
use crate::device_auth::{self, DeviceAuth};
use crate::error::CastError;
use crate::framing;
use crate::offer::{Answer, AnswerMessage, VideoOffer, offer_message, parse_answer};
use crate::tls;

/// App id of Google's mirroring receiver for audio+video (`GetCastStreamingAudioVideoAppId()`
/// in `cast/common/public/cast_streaming_app_ids.h`).
pub const MIRRORING_APP_ID: &str = "0F5096E8";

/// How to run a session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Address of the receiver's Cast port (normally `<ip>:8009`).
    pub addr: SocketAddr,
    /// Device authentication policy. There is deliberately no `Default`.
    pub auth: DeviceAuth,
    /// App to launch.
    pub app_id: String,
    /// Heartbeat timing.
    pub heartbeat: Heartbeat,
    /// Time allowed for each step (TLS, auth, status, launch, answer).
    pub step_timeout: Duration,
    /// Use payload type 96 instead of 101 (see [`crate::offer::PAYLOAD_TYPE_ANDROID_HACK`]).
    pub android_rtp_hack: bool,
}

impl SessionConfig {
    /// Config with the defaults and the given policy.
    pub fn new(addr: SocketAddr, auth: DeviceAuth) -> Self {
        Self {
            addr,
            auth,
            app_id: MIRRORING_APP_ID.to_owned(),
            heartbeat: Heartbeat::default(),
            step_timeout: Duration::from_secs(10),
            android_rtp_hack: false,
        }
    }
}

/// One application the receiver reports as running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningApp {
    /// App id.
    pub app_id: String,
    /// Display name.
    pub display_name: String,
    /// Session id (used for `STOP`).
    pub session_id: String,
    /// Transport id (destination of app messages).
    pub transport_id: String,
}

/// Parsed `RECEIVER_STATUS`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReceiverStatus {
    /// Running applications.
    pub apps: Vec<RunningApp>,
    /// True if the message carried an explicit `applications` array (possibly empty). A
    /// status without one says nothing about which apps run (for example a volume change).
    pub applications_listed: bool,
}

/// Parse the text of a `RECEIVER_STATUS` message. `None`: another message type.
pub fn parse_receiver_status(text: &str) -> Option<(Option<i64>, ReceiverStatus)> {
    let v: Value = serde_json::from_str(text).ok()?;
    if v.get("type")?.as_str()? != "RECEIVER_STATUS" {
        return None;
    }
    let s = |a: &Value, k: &str| {
        a.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let listed = v
        .get("status")
        .and_then(|st| st.get("applications"))
        .and_then(Value::as_array);
    let applications_listed = listed.is_some();
    let apps = listed
        .map(|apps| {
            apps.iter()
                .map(|a| RunningApp {
                    app_id: s(a, "appId"),
                    display_name: s(a, "displayName"),
                    session_id: s(a, "sessionId"),
                    transport_id: s(a, "transportId"),
                })
                .collect()
        })
        .unwrap_or_default();
    Some((
        v.get("requestId").and_then(Value::as_i64),
        ReceiverStatus {
            apps,
            applications_listed,
        },
    ))
}

/// What the receiver said about `LAUNCH`.
#[derive(Debug, PartialEq, Eq)]
enum LaunchReply {
    App(RunningApp),
    Error(String),
    Invalid(String),
}

fn launch_reply(app_id: &str, m: &crate::proto::CastMessage) -> Option<LaunchReply> {
    if m.namespace != RECEIVER_NS {
        return None;
    }
    let text = m.text_payload()?;
    if let Some((_, st)) = parse_receiver_status(&text) {
        return st
            .apps
            .into_iter()
            .find(|a| {
                a.app_id.eq_ignore_ascii_case(app_id)
                    && !a.session_id.is_empty()
                    && !a.transport_id.is_empty()
            })
            .map(LaunchReply::App);
    }
    let v: Value = serde_json::from_str(&text).ok()?;
    // The reason comes from the network and is printed by callers: escape and cap it.
    let reason = crate::error::sanitize_display(
        v.get("reason").and_then(Value::as_str).unwrap_or("UNKNOWN"),
        64,
    );
    match v.get("type")?.as_str()? {
        "LAUNCH_ERROR" => Some(LaunchReply::Error(reason)),
        "INVALID_REQUEST" => Some(LaunchReply::Invalid(reason)),
        _ => None,
    }
}

/// The streaming parameters both sides agreed on. `Debug` hides the AES key and IV mask.
#[derive(Clone)]
pub struct StreamParams {
    /// Our SSRC (from the OFFER).
    pub sender_ssrc: u32,
    /// The receiver's SSRC (from the ANSWER).
    pub receiver_ssrc: u32,
    /// RTP payload type.
    pub payload_type: u8,
    /// AES key.
    pub aes_key: [u8; 16],
    /// AES IV mask.
    pub aes_iv_mask: [u8; 16],
}

impl std::fmt::Debug for StreamParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamParams")
            .field("sender_ssrc", &self.sender_ssrc)
            .field("receiver_ssrc", &self.receiver_ssrc)
            .field("payload_type", &self.payload_type)
            .field("aes_key", &"<redacted>")
            .field("aes_iv_mask", &"<redacted>")
            .finish()
    }
}

/// A session that is ready to stream. Keep it alive while streaming: dropping it closes the
/// channel and stops the heartbeat.
pub struct Negotiated {
    /// The Cast channel (heartbeat keeps running).
    pub connection: Connection,
    /// Where RTP and RTCP go (receiver IP + the ANSWER's `udpPort`).
    pub remote_udp: SocketAddr,
    /// Stream parameters.
    pub stream: StreamParams,
    /// The raw ANSWER.
    pub answer: Answer,
    /// The app session.
    pub app: RunningApp,
    /// Our id on the virtual connection to the app.
    pub local_id: String,
    /// Device authentication result: `Some` if verified.
    pub auth_outcome: Option<device_auth::AuthOutcome>,
    /// Warnings the caller must show (for example "device authentication skipped").
    pub warnings: Vec<String>,
}

impl Negotiated {
    /// Ask the receiver to stop the app (`STOP` with the session id) and close.
    pub async fn stop(self) {
        let stop = json_message(
            PLATFORM_SENDER,
            PLATFORM_RECEIVER,
            RECEIVER_NS,
            &json!({"type": "STOP", "requestId": 100, "sessionId": self.app.session_id}),
        );
        let _ = self.connection.send(stop).await;
        // Give the writer task a moment to put it on the wire before the tasks are aborted.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Connect, authenticate, launch and negotiate. See the module documentation.
pub async fn negotiate(cfg: &SessionConfig, video: &VideoOffer) -> Result<Negotiated, CastError> {
    let step = cfg.step_timeout;
    let mut warnings = Vec::new();

    // 1. TLS.
    let mut tls = tokio::time::timeout(step, tls::connect(cfg.addr))
        .await
        .map_err(|_| CastError::Timeout(step, "the TLS handshake"))??;

    // 2. Device authentication.
    let auth_outcome = match &cfg.auth {
        DeviceAuth::InsecureSkip => {
            warnings.push(
                "DEVICE AUTHENTICATION SKIPPED (--insecure-skip-device-auth): the receiver is NOT \
                 verified. Anybody who answers at this address gets your screen picture."
                    .to_owned(),
            );
            None
        }
        DeviceAuth::Verify(anchors) => {
            let nonce = device_auth::new_nonce();
            framing::write_message(&mut tls.stream, &device_auth::challenge_message(&nonce))
                .await?;
            let reply = tokio::time::timeout(step, async {
                loop {
                    let m = framing::read_message(&mut tls.stream).await?;
                    if m.namespace == device_auth::AUTH_NAMESPACE {
                        return Ok::<_, CastError>(m);
                    }
                }
            })
            .await
            .map_err(|_| CastError::Timeout(step, "the device authentication reply"))??;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64);
            Some(device_auth::verify_response(
                &reply,
                &nonce,
                &tls.peer_cert,
                anchors,
                now,
            )?)
        }
    };

    // 3. Channel + CONNECT to the platform.
    let mut conn = Connection::start(tls.stream, cfg.heartbeat);
    conn.send(connect_message(PLATFORM_SENDER, PLATFORM_RECEIVER))
        .await?;

    // 4. GET_STATUS, LAUNCH.
    let get_status = json_message(
        PLATFORM_SENDER,
        PLATFORM_RECEIVER,
        RECEIVER_NS,
        &json!({"type": "GET_STATUS", "requestId": 1}),
    );
    conn.send(get_status).await?;
    // The reply is informational (the app may already be running); do not fail if it is slow.
    let _ = conn
        .recv_matching(Duration::from_secs(3).min(step), "RECEIVER_STATUS", |m| {
            (m.namespace == RECEIVER_NS)
                .then(|| m.text_payload().and_then(|t| parse_receiver_status(&t)))
                .flatten()
        })
        .await;

    // Open Screen's launch message: language en-US and supportedAppTypes ["WEB"].
    let launch = json_message(
        PLATFORM_SENDER,
        PLATFORM_RECEIVER,
        RECEIVER_NS,
        &json!({"type": "LAUNCH", "requestId": 2, "appId": cfg.app_id, "language": "en-US",
                "supportedAppTypes": ["WEB"]}),
    );
    conn.send(launch).await?;
    let app_id = cfg.app_id.clone();
    let reply = conn
        .recv_matching(step, "the mirroring app to start (RECEIVER_STATUS)", |m| {
            launch_reply(&app_id, m)
        })
        .await?;
    let app = match reply {
        LaunchReply::App(a) => a,
        LaunchReply::Error(reason) => return Err(CastError::LaunchError { reason }),
        LaunchReply::Invalid(reason) => return Err(CastError::InvalidRequest(reason)),
    };

    // 5. CONNECT to the app.
    let local_id = format!("streaming_sender-{:08x}", rand::random::<u32>());
    conn.send(connect_message(&local_id, &app.transport_id))
        .await?;

    // 6. OFFER / ANSWER.
    let seq = 1;
    let offer = json_message(
        &local_id,
        &app.transport_id,
        WEBRTC_NS,
        &offer_message(video, seq),
    );
    conn.send(offer).await?;
    let answer = conn
        .recv_matching(step, "the ANSWER", |m| {
            if m.namespace != WEBRTC_NS {
                return None;
            }
            match parse_answer(&m.text_payload()?)? {
                Ok(AnswerMessage::Ok { seq_num, answer }) if seq_num == seq => Some(Ok(answer)),
                Ok(AnswerMessage::Error {
                    seq_num,
                    description,
                }) if seq_num == seq => Some(Err(CastError::OfferRejected(
                    crate::error::sanitize_display(&description, 200),
                ))),
                Ok(_) => None,
                Err(e) => Some(Err(e)),
            }
        })
        .await??;

    let receiver_ssrc = answer.receiver_ssrc_for(video.index).ok_or_else(|| {
        CastError::OfferRejected("the receiver did not accept the video stream".into())
    })?;
    if receiver_ssrc == video.ssrc {
        return Err(CastError::Protocol(
            "receiver SSRC equals sender SSRC".into(),
        ));
    }
    let remote_udp = SocketAddr::new(cfg.addr.ip(), answer.udp_port);
    Ok(Negotiated {
        connection: conn,
        remote_udp,
        stream: StreamParams {
            sender_ssrc: video.ssrc,
            receiver_ssrc,
            payload_type: video.payload_type,
            aes_key: video.aes_key,
            aes_iv_mask: video.aes_iv_mask,
        },
        answer,
        app,
        local_id,
        auth_outcome,
        warnings,
    })
}

/// The virtual-connection `CONNECT` message. Field values follow `MakeConnectMessage` in
/// Open Screen's `message_util.cc` (connType 0 = strong; sdkType 2; connectionType 1 = local).
/// UNCERTAIN: platform 6 (Linux) is sent on every OS; the user agent says Comet Cast.
pub fn connect_message(source: &str, destination: &str) -> crate::proto::CastMessage {
    json_message(
        source,
        destination,
        CONNECTION_NS,
        &json!({
            "type": "CONNECT",
            "protocolVersionList": [0, 1, 2, 3],
            "userAgent": "Comet Cast (cc-cast beta)",
            "connType": 0,
            "origin": {},
            "senderInfo": {
                "skdType": 2,
                "version": "Comet Cast (cc-cast beta)",
                "browserVersion": "Comet Cast (cc-cast beta)",
                "platform": 6,
                "connectionType": 1,
            },
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::CastMessage;

    fn status(text: &str) -> CastMessage {
        CastMessage::text("receiver-0", "sender-0", RECEIVER_NS, text.to_owned())
    }

    #[test]
    fn parses_receiver_status() {
        let t = r#"{"type":"RECEIVER_STATUS","requestId":2,"status":{"applications":[
            {"appId":"0F5096E8","displayName":"Mirror","sessionId":"s-1","transportId":"t-1"}],
            "volume":{"level":1.0}}}"#;
        let (req, st) = parse_receiver_status(t).unwrap();
        assert_eq!(req, Some(2));
        assert_eq!(st.apps.len(), 1);
        assert_eq!(st.apps[0].transport_id, "t-1");
        assert!(st.applications_listed);
        let (_, st) =
            parse_receiver_status(r#"{"type":"RECEIVER_STATUS","status":{"volume":{}}}"#).unwrap();
        assert!(!st.applications_listed);
        let (_, st) =
            parse_receiver_status(r#"{"type":"RECEIVER_STATUS","status":{"applications":[]}}"#)
                .unwrap();
        assert!(st.applications_listed && st.apps.is_empty());
        // Debug output never contains key material.
        let p = StreamParams {
            sender_ssrc: 1,
            receiver_ssrc: 2,
            payload_type: 101,
            aes_key: [0xab; 16],
            aes_iv_mask: [0xcd; 16],
        };
        let d = format!("{p:?}");
        assert!(d.contains("redacted") && !d.contains("171") && !d.contains("ab"));
        assert!(parse_receiver_status(r#"{"type":"PONG"}"#).is_none());
    }

    #[test]
    fn launch_reply_variants() {
        let ok = status(
            r#"{"type":"RECEIVER_STATUS","status":{"applications":[{"appId":"0f5096e8","sessionId":"s","transportId":"t"}]}}"#,
        );
        assert!(matches!(
            launch_reply(MIRRORING_APP_ID, &ok),
            Some(LaunchReply::App(_))
        ));
        // App listed without a transport id yet: keep waiting.
        let early = status(
            r#"{"type":"RECEIVER_STATUS","status":{"applications":[{"appId":"0F5096E8","sessionId":"s"}]}}"#,
        );
        assert_eq!(launch_reply(MIRRORING_APP_ID, &early), None);
        let idle = status(r#"{"type":"RECEIVER_STATUS","status":{"isActiveInput":true}}"#);
        assert_eq!(launch_reply(MIRRORING_APP_ID, &idle), None);
        let err = status(r#"{"type":"LAUNCH_ERROR","requestId":2,"reason":"NOT_FOUND"}"#);
        assert_eq!(
            launch_reply(MIRRORING_APP_ID, &err),
            Some(LaunchReply::Error("NOT_FOUND".into()))
        );
        let evil = status(r#"{"type":"LAUNCH_ERROR","reason":"X\u001b[2J\nY"}"#);
        assert_eq!(
            launch_reply(MIRRORING_APP_ID, &evil),
            Some(LaunchReply::Error("X\\u{1b}[2J\\u{a}Y".into()))
        );
        let bad = status(r#"{"type":"INVALID_REQUEST","reason":"INVALID_COMMAND"}"#);
        assert_eq!(
            launch_reply(MIRRORING_APP_ID, &bad),
            Some(LaunchReply::Invalid("INVALID_COMMAND".into()))
        );
    }

    #[test]
    fn connect_message_shape() {
        let m = connect_message("sender-0", "receiver-0");
        assert_eq!(m.namespace, CONNECTION_NS);
        let v: Value = serde_json::from_str(&m.text_payload().unwrap()).unwrap();
        assert_eq!(v["type"], "CONNECT");
        assert_eq!(v["connType"], 0);
        assert_eq!(v["protocolVersionList"], json!([0, 1, 2, 3]));
    }
}
