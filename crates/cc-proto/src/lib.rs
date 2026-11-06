// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast protocol types: messages, room codes, versioning.
//!
//! Must stay usable on every target including WebAssembly, so it only depends on
//! serde, serde_json and thiserror.

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
        let upper = input.trim().to_uppercase();
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
    /// Join the room with this code.
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
    /// WebRTC ICE candidate (candidate line).
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
        /// Short machine-readable code, e.g. `room_not_found`.
        code: String,
        /// Human-readable text.
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

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
            ServerMsg::Control {
                cmd: ControlCmd::NextPage,
            },
            ServerMsg::Error {
                code: "room_not_found".into(),
                message: "No such room".into(),
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
    fn invalid_room_code_fails_to_deserialize() {
        let j = r#"{"type":"join_room","code":"bad"}"#;
        assert!(serde_json::from_str::<ClientMsg>(j).is_err());
    }
}
