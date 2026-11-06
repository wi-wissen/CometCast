// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast protocol types: messages, room codes, versioning.
//!
//! Must stay usable on every target including WebAssembly, so its normal
//! dependencies are only serde and thiserror (serde_json is used in tests).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Version of the wire protocol. Sent in [`ClientMsg::Hello`].
pub const PROTO_VERSION: u32 = 1;

/// Which side of a transfer a client is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Sends content (screen, documents, links, files).
    Sender,
    /// Shows content (TV, mini-PC, browser).
    Receiver,
}

/// Errors when reading a room code.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RoomCodeError {
    /// The code does not have exactly [`RoomCode::LEN`] characters.
    #[error("room code must have {} characters", RoomCode::LEN)]
    WrongLength,
    /// The code contains a character that is not in [`RoomCode::ALPHABET`].
    #[error("room code contains an invalid character: {0:?}")]
    InvalidChar(char),
}

/// Short code that joins a sender and a receiver, e.g. `K7M2QX`.
///
/// Always stored in uppercase. Easily confused characters (0/O, 1/I) are not used.
/// A newtype is a tiny wrapper struct that gives a plain `String` its own type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RoomCode(String);

impl RoomCode {
    /// Number of characters in a code.
    pub const LEN: usize = 6;
    /// Allowed characters: 32 symbols without 0, O, 1, I.
    pub const ALPHABET: &'static str = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

    /// Parse user input. Ignores case and surrounding spaces.
    pub fn parse(input: &str) -> Result<Self, RoomCodeError> {
        // ASCII-only uppercase: non-ASCII look-alikes (e.g. 'ſ') must not be accepted.
        let upper = input.trim().to_ascii_uppercase();
        if upper.chars().count() != Self::LEN {
            return Err(RoomCodeError::WrongLength);
        }
        if let Some(bad) = upper.chars().find(|c| !Self::ALPHABET.contains(*c)) {
            return Err(RoomCodeError::InvalidChar(bad));
        }
        Ok(Self(upper))
    }

    /// Build a code from six indexes (each 0..32) that the caller got from a random
    /// source. This crate does no random generation itself.
    pub fn from_indices(indices: [u8; Self::LEN]) -> Option<Self> {
        let alphabet = Self::ALPHABET.as_bytes();
        let mut code = String::with_capacity(Self::LEN);
        for i in indices {
            code.push(*alphabet.get(usize::from(i))? as char);
        }
        Some(Self(code))
    }

    /// The code as text (uppercase).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RoomCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// These two conversions are used by serde (see the `try_from`/`into` attribute above).
impl TryFrom<String> for RoomCode {
    type Error = RoomCodeError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<RoomCode> for String {
    fn from(code: RoomCode) -> Self {
        code.0
    }
}

/// Remote-control commands from the sender (presenter) to the receiver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlCmd {
    /// Show the next page or slide.
    NextPage,
    /// Show the previous page or slide.
    PrevPage,
    /// Jump to a page (first page is 1).
    GoToPage {
        /// Target page number.
        page: u32,
    },
    /// Stop showing the current content.
    Stop,
}

/// Error codes added with server hardening (milestone M8). They are additive: older
/// clients treat an unknown code like any other error. See docs/protocol.md.
pub mod error_codes {
    /// `create_room` needs a solved ALTCHA payload and none was sent.
    pub const ALTCHA_REQUIRED: &str = "altcha_required";
    /// The ALTCHA payload is wrong, expired or was already used.
    pub const ALTCHA_INVALID: &str = "altcha_invalid";
    /// Too many requests from this address, or the server is full. Try again later.
    pub const RATE_LIMITED: &str = "rate_limited";
    /// This address is temporarily banned (too many failed joins). The connection is closed.
    pub const BANNED: &str = "banned";
}

/// Messages from a client (sender or receiver) to the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// First message: protocol version and role.
    Hello {
        /// Protocol version the client speaks.
        proto_version: u32,
        /// Sender or receiver.
        role: Role,
    },
    /// Receiver only: ask the server for a new room.
    CreateRoom {
        /// Solved ALTCHA proof-of-work payload (base64 of the JSON that the ALTCHA
        /// widget or library produces; see docs/server.md). Optional on the wire:
        /// servers in internal mode do not need it, open servers answer
        /// `altcha_required` without it. Not sent when `None`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        altcha: Option<String>,
    },
    /// Sender only: join the room with this code.
    JoinRoom {
        /// Room code.
        code: RoomCode,
    },
    /// WebRTC offer (SDP text).
    Offer {
        /// SDP text.
        sdp: String,
    },
    /// WebRTC answer (SDP text).
    Answer {
        /// SDP text.
        sdp: String,
    },
    /// WebRTC ICE candidate. For browsers this string holds the JSON text of an
    /// `RTCIceCandidateInit` (see docs/protocol.md).
    IceCandidate {
        /// Candidate text.
        candidate: String,
    },
    /// Remote-control command.
    Control {
        /// The command.
        cmd: ControlCmd,
    },
}

/// Messages from the server to a client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Answer to `Hello`.
    Hello {
        /// Protocol version the server speaks.
        proto_version: u32,
    },
    /// Sent to the receiver after `CreateRoom`: the code to show on screen.
    RoomCreated {
        /// The new room code.
        code: RoomCode,
    },
    /// Sent to the sender after a successful `JoinRoom`.
    Joined,
    /// Sent to the receiver when a sender has joined its room.
    PeerJoined,
    /// Sent to the remaining peer when the other one disconnected.
    PeerLeft,
    /// WebRTC offer forwarded from the peer.
    Offer {
        /// SDP text.
        sdp: String,
    },
    /// WebRTC answer forwarded from the peer.
    Answer {
        /// SDP text.
        sdp: String,
    },
    /// ICE candidate forwarded from the peer.
    IceCandidate {
        /// Candidate text.
        candidate: String,
    },
    /// Remote-control command forwarded from the peer.
    Control {
        /// The command.
        cmd: ControlCmd,
    },
    /// Something went wrong.
    Error {
        /// Short machine-readable code, e.g. `room_not_found`. The codes added for
        /// server hardening are in [`error_codes`].
        code: String,
        /// Human-readable text.
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_non_ascii_lookalikes() {
        // 'ſ' (long s) uppercases to 'S' in Unicode, but is not a valid code character.
        assert!(RoomCode::parse("ſ7M2QX").is_err());
    }

    #[test]
    fn room_code_parse_is_case_insensitive_and_uppercases() {
        let c = RoomCode::parse(" k7m2qx ").unwrap();
        assert_eq!(c.as_str(), "K7M2QX");
        assert_eq!(c.to_string(), "K7M2QX");
    }

    #[test]
    fn room_code_rejects_bad_input() {
        assert_eq!(RoomCode::parse("ABC"), Err(RoomCodeError::WrongLength));
        assert_eq!(
            RoomCode::parse("ABCDE0"),
            Err(RoomCodeError::InvalidChar('0'))
        );
        assert_eq!(
            RoomCode::parse("ABCDEI"),
            Err(RoomCodeError::InvalidChar('I'))
        );
    }

    #[test]
    fn room_code_from_indices() {
        assert_eq!(
            RoomCode::from_indices([0, 1, 2, 3, 4, 5]).unwrap().as_str(),
            "ABCDEF"
        );
        assert!(RoomCode::from_indices([32, 0, 0, 0, 0, 0]).is_none());
        assert_eq!(RoomCode::ALPHABET.len(), 32);
    }

    #[test]
    fn client_msg_round_trip() {
        let msgs = vec![
            ClientMsg::Hello {
                proto_version: PROTO_VERSION,
                role: Role::Sender,
            },
            ClientMsg::CreateRoom { altcha: None },
            ClientMsg::CreateRoom {
                altcha: Some("eyJ9".into()),
            },
            ClientMsg::JoinRoom {
                code: RoomCode::parse("k7m2qx").unwrap(),
            },
            ClientMsg::Offer { sdp: "v=0".into() },
            ClientMsg::Answer { sdp: "v=0".into() },
            ClientMsg::IceCandidate {
                candidate: "candidate:1".into(),
            },
            ClientMsg::Control {
                cmd: ControlCmd::GoToPage { page: 3 },
            },
        ];
        for m in msgs {
            let json = serde_json::to_string(&m).unwrap();
            let back: ClientMsg = serde_json::from_str(&json).unwrap();
            assert_eq!(m, back);
        }
    }

    #[test]
    fn server_msg_round_trip() {
        let msgs = vec![
            ServerMsg::Hello {
                proto_version: PROTO_VERSION,
            },
            ServerMsg::RoomCreated {
                code: RoomCode::parse("k7m2qx").unwrap(),
            },
            ServerMsg::Joined,
            ServerMsg::PeerJoined,
            ServerMsg::PeerLeft,
            ServerMsg::Control {
                cmd: ControlCmd::NextPage,
            },
            ServerMsg::Error {
                code: "room_not_found".into(),
                message: "No such room".into(),
            },
            ServerMsg::Error {
                code: error_codes::ALTCHA_REQUIRED.into(),
                message: "solve the puzzle first".into(),
            },
        ];
        for m in msgs {
            let json = serde_json::to_string(&m).unwrap();
            let back: ServerMsg = serde_json::from_str(&json).unwrap();
            assert_eq!(m, back);
        }
    }

    #[test]
    fn wire_format_is_stable() {
        let m = ClientMsg::Hello {
            proto_version: 1,
            role: Role::Receiver,
        };
        assert_eq!(
            serde_json::to_string(&m).unwrap(),
            r#"{"type":"hello","proto_version":1,"role":"receiver"}"#
        );
        let j = r#"{"type":"join_room","code":"k7m2qx"}"#;
        let ClientMsg::JoinRoom { code } = serde_json::from_str(j).unwrap() else {
            panic!("wrong variant");
        };
        assert_eq!(code.as_str(), "K7M2QX");
    }

    #[test]
    fn new_messages_wire_format() {
        assert_eq!(
            serde_json::to_string(&ClientMsg::CreateRoom { altcha: None }).unwrap(),
            r#"{"type":"create_room"}"#
        );
        assert_eq!(
            serde_json::to_string(&ClientMsg::CreateRoom {
                altcha: Some("e30=".into())
            })
            .unwrap(),
            r#"{"type":"create_room","altcha":"e30="}"#
        );
        let created = ServerMsg::RoomCreated {
            code: RoomCode::parse("K7M2QX").unwrap(),
        };
        assert_eq!(
            serde_json::to_string(&created).unwrap(),
            r#"{"type":"room_created","code":"K7M2QX"}"#
        );
        assert_eq!(
            serde_json::to_string(&ServerMsg::Joined).unwrap(),
            r#"{"type":"joined"}"#
        );
        assert_eq!(
            serde_json::to_string(&ServerMsg::PeerJoined).unwrap(),
            r#"{"type":"peer_joined"}"#
        );
        assert_eq!(
            serde_json::to_string(&ServerMsg::PeerLeft).unwrap(),
            r#"{"type":"peer_left"}"#
        );
        let m: ClientMsg = serde_json::from_str(r#"{"type":"create_room"}"#).unwrap();
        assert_eq!(m, ClientMsg::CreateRoom { altcha: None });
        // Old clients send no `altcha` field; a null is accepted too.
        let m: ClientMsg = serde_json::from_str(r#"{"type":"create_room","altcha":null}"#).unwrap();
        assert_eq!(m, ClientMsg::CreateRoom { altcha: None });
        let m: ClientMsg =
            serde_json::from_str(r#"{"type":"create_room","altcha":"e30="}"#).unwrap();
        assert_eq!(
            m,
            ClientMsg::CreateRoom {
                altcha: Some("e30=".into())
            }
        );
    }

    #[test]
    fn new_error_codes_are_stable() {
        assert_eq!(error_codes::ALTCHA_REQUIRED, "altcha_required");
        assert_eq!(error_codes::ALTCHA_INVALID, "altcha_invalid");
        assert_eq!(error_codes::RATE_LIMITED, "rate_limited");
        assert_eq!(error_codes::BANNED, "banned");
    }

    #[test]
    fn invalid_room_code_fails_to_deserialize() {
        let j = r#"{"type":"join_room","code":"bad"}"#;
        assert!(serde_json::from_str::<ClientMsg>(j).is_err());
    }
}
