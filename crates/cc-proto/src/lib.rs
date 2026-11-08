// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast protocol types: messages, room codes, versioning.
//!
//! Must stay usable on every target including WebAssembly, so its normal
//! dependencies are only serde and thiserror (serde_json is used in tests).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Version of the wire protocol. Sent in [`ClientMsg::Hello`].
pub const PROTO_VERSION: u32 = 2;

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
    /// Presenter to display: the presenter paused its screen (it stops sending video). The
    /// display keeps the last picture. Idempotent (docs/design/pause.md).
    Pause,
    /// Presenter to display: the presenter resumed. Idempotent.
    Resume,
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

    // --- Join approval (see docs/protocol.md, "Join approval") ---

    /// The sender is waiting for the display device to decide and tried to relay something.
    pub const JOIN_PENDING: &str = "join_pending";
    /// The display device refused this join.
    pub const JOIN_DENIED: &str = "join_denied";
    /// The room is locked: nobody can join right now.
    pub const ROOM_LOCKED: &str = "room_locked";
    /// Nobody decided on the join request in time.
    pub const APPROVAL_TIMEOUT: &str = "approval_timeout";
    /// The display device removed this sender from the room.
    pub const REMOVED_BY_HOST: &str = "removed_by_host";
    /// The server does not allow rooms with `join_mode` `open`.
    pub const OPEN_ROOMS_DISABLED: &str = "open_rooms_disabled";
    /// `join_decision` named a request that is not (or no longer) waiting.
    pub const NO_SUCH_REQUEST: &str = "no_such_request";
    /// `unblock` named an entry that is not on the room's block list.
    pub const NO_SUCH_BLOCK: &str = "no_such_block";
    /// The room's block list is full; unblock somebody first.
    pub const BLOCK_LIST_FULL: &str = "block_list_full";
}

/// Longest display name (in characters) that the server relays to the display device.
pub const MAX_DISPLAY_NAME_CHARS: usize = 40;

/// Longest room name (in characters) that the server keeps and relays to senders.
pub const MAX_ROOM_NAME_CHARS: usize = 32;

/// Longest hand-over token (characters) that the server relays (native-apps.md 20.5). A token
/// is 128 random bits as base64url (22 characters); the bound leaves room for longer ones.
pub const MAX_HANDOVER_CHARS: usize = 64;

/// Who may join a room. Chosen by the receiver (display device) and changeable at any time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinMode {
    /// Anyone who knows the room code joins at once (the behaviour before join approval).
    Open,
    /// Every join waits for an explicit allow/deny on the display device.
    Confirm,
    /// Nobody can join.
    Locked,
}

impl JoinMode {
    /// The wire text (`open`, `confirm`, `locked`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Confirm => "confirm",
            Self::Locked => "locked",
        }
    }
}

impl fmt::Display for JoinMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for JoinMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "open" => Ok(Self::Open),
            "confirm" => Ok(Self::Confirm),
            "locked" => Ok(Self::Locked),
            other => Err(format!(
                "unknown join mode {other:?} (use open, confirm or locked)"
            )),
        }
    }
}

/// True for characters that must never reach a display: control characters, bidi
/// overrides/isolates, zero-width and invisible formatting characters, line/paragraph
/// separators and the byte-order mark.
fn is_unsafe_name_char(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
                | '\u{E000}'..='\u{F8FF}'
        )
}

/// Remove unsafe characters, collapse whitespace, trim and cut to `max` characters.
/// Returns an empty string when nothing is left.
fn clean_text(raw: &str, max: usize) -> String {
    let mut out = String::new();
    let mut pending_space = false;
    for c in raw.chars() {
        // A tab or a line break inside a name is a space, not an invisible character.
        let spacing = matches!(c, '\t' | '\n' | '\r');
        if !spacing && is_unsafe_name_char(c) {
            continue;
        }
        if c.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(c);
        if out.chars().count() >= max {
            break;
        }
    }
    let out: String = out.chars().take(max).collect();
    out.trim().to_string()
}

/// Make a display name that a joiner chose safe to show on the display device.
///
/// The name is untrusted. Control, bidi and invisible characters are removed, runs of
/// whitespace become one space, the result is trimmed and cut to
/// [`MAX_DISPLAY_NAME_CHARS`] characters. If nothing is left, `Guest {guest_number}` is
/// returned. Clients must still show the name as plain text, never as markup.
pub fn sanitize_display_name(raw: Option<&str>, guest_number: u32) -> String {
    let out = clean_text(raw.unwrap_or(""), MAX_DISPLAY_NAME_CHARS);
    if out.is_empty() {
        format!("Guest {guest_number}")
    } else {
        out
    }
}

/// Make a room name that the display device chose safe to relay to senders.
///
/// Cleaned exactly like a display name (control, bidi, zero-width and private-use
/// characters removed, whitespace collapsed, trimmed) and cut to [`MAX_ROOM_NAME_CHARS`]
/// characters. Returns `None` when `raw` is absent or nothing is left (there is no
/// placeholder: clients show no room name then).
pub fn sanitize_room_name(raw: Option<&str>) -> Option<String> {
    let out = clean_text(raw?, MAX_ROOM_NAME_CHARS);
    (!out.is_empty()).then_some(out)
}

/// Check the optional `handover` field of `join_room` before it is relayed (native-apps.md
/// 20.5): 1 to [`MAX_HANDOVER_CHARS`] characters of the base64url alphabet (`A-Z a-z 0-9 - _`).
/// Anything else (empty, too long, other characters) is dropped and the join is an ordinary
/// join: the field is a hint for the display, never a reason to refuse a join. The server
/// does not know or check tokens; the display that minted the room decides.
pub fn sanitize_handover(raw: Option<&str>) -> Option<String> {
    let t = raw?;
    let ok = !t.is_empty()
        && t.len() <= MAX_HANDOVER_CHARS
        && t.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    ok.then(|| t.to_owned())
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
        /// Who may join the room. Omitted = the server's default (see docs/server.md).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        join_mode: Option<JoinMode>,
        /// Name of the room, shown to senders (`join_pending`, `joined`). Untrusted: the
        /// server cleans it ([`sanitize_room_name`], at most [`MAX_ROOM_NAME_CHARS`]
        /// characters) and drops it when empty. Optional; not sent when `None`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        room_name: Option<String>,
    },
    /// Sender only: join the room with this code.
    JoinRoom {
        /// Room code.
        code: RoomCode,
        /// Name shown to the display device when it has to approve the join. Untrusted:
        /// the server sanitises it ([`sanitize_display_name`]).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// Single-use hand-over token (iOS broadcast extension, native-apps.md 20.5): at most
        /// [`MAX_HANDOVER_CHARS`] characters of the base64url alphabet. Relayed unchanged to
        /// the display device in `join_request` / `peer_joined`; the server neither stores nor
        /// checks it (the display does). Invalid values are dropped
        /// ([`sanitize_handover`]). Optional; older servers ignore the field.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        handover: Option<String>,
    },
    /// Receiver (room owner) only: change who may join, at any time.
    SetJoinMode {
        /// The new mode.
        join_mode: JoinMode,
    },
    /// Receiver (room owner) only: decide on waiting join requests.
    JoinDecision {
        /// Request id from `join_request`. Absent together with `allow: false` means
        /// "deny all waiting requests"; absent with `allow: true` is invalid.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<u64>,
        /// Allow or deny.
        allow: bool,
        /// With `allow: false` and an `id`: also block the sender's address bucket for the
        /// rest of this room session. A blocked sender gets the same `join_denied` as any
        /// denied sender, at once, and the host is not asked again.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        block: bool,
    },
    /// Receiver (room owner) only: remove the connected sender from the room (end its
    /// presentation). The room stays.
    RemovePeer {
        /// Also block the sender's address bucket for the rest of this room session.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        block: bool,
    },
    /// Receiver (room owner) only: take an entry off the room's block list.
    Unblock {
        /// Block id from `blocked`.
        id: u64,
    },
    /// Receiver (room owner) only: end the room session. The connected sender is told
    /// `peer_left`, waiting senders `room_not_found`, the room and its code are gone and the
    /// owner gets `room_ended`; it may then create a new room on the same connection.
    EndRoom,
    /// Sender only: stop waiting for approval.
    CancelJoin,
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
        /// The join mode the room really has.
        join_mode: JoinMode,
    },
    /// Sent to the sender after a successful `JoinRoom`.
    Joined {
        /// Cleaned name of the room as the display device set it; absent when it set none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        room_name: Option<String>,
    },
    /// Sent to the receiver when a sender has joined its room.
    PeerJoined {
        /// Sanitised display name of the sender (`Guest N` if it gave none).
        name: String,
        /// The hand-over token the sender joined with, when the join needed no decision (join
        /// mode `open`); see `join_room`. Absent otherwise.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        handover: Option<String>,
    },
    /// Sent to a sender whose join waits for the display device's decision.
    JoinPending {
        /// Seconds after which the server gives up (`approval_timeout`).
        timeout_secs: u64,
        /// Two-digit check number (10..=99) that the server picked for this request. The
        /// display device shows the same number on the request, so the host can match the
        /// person in the room with the request. Not a secret.
        check: u8,
        /// Cleaned name of the room as the display device set it; absent when it set none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        room_name: Option<String>,
    },
    /// Sent to the receiver: somebody wants to join (join mode `confirm`).
    JoinRequest {
        /// Id to answer with `join_decision`. Unique within the room.
        id: u64,
        /// Sanitised display name; nothing else about the joiner is passed on.
        name: String,
        /// The check number the joiner sees on its waiting screen (see `join_pending`).
        check: u8,
        /// The hand-over token of the join, as checked by [`sanitize_handover`]; the display
        /// admits a join with a token it issued without asking (native-apps.md 20.5). Absent
        /// for an ordinary join.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        handover: Option<String>,
    },
    /// Sent to the receiver: a waiting request is gone (joiner left, cancelled, timed out,
    /// or the mode became `locked`).
    JoinRequestCancelled {
        /// The request id.
        id: u64,
        /// `left`, `cancelled`, `timeout` or `locked`.
        reason: String,
    },
    /// Sent to the receiver: an entry was added to the room's block list.
    Blocked {
        /// Block id for `unblock`. Unique within the room.
        id: u64,
        /// Sanitised display name of the blocked sender (for the list only).
        name: String,
    },
    /// Sent to the receiver: an entry left the block list.
    Unblocked {
        /// Block id.
        id: u64,
    },
    /// Sent to the receiver after `end_room`: the room is gone.
    RoomEnded,
    /// Sent to the receiver: the join mode of the room (after `create_room` and after
    /// every `set_join_mode`).
    JoinModeChanged {
        /// The mode now in force.
        join_mode: JoinMode,
    },
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
            ClientMsg::CreateRoom {
                altcha: None,
                join_mode: None,
                room_name: None,
            },
            ClientMsg::CreateRoom {
                altcha: Some("eyJ9".into()),
                join_mode: Some(JoinMode::Confirm),
                room_name: Some("Meeting room Elbe".into()),
            },
            ClientMsg::JoinRoom {
                code: RoomCode::parse("k7m2qx").unwrap(),
                name: Some("Ana".into()),
                handover: None,
            },
            ClientMsg::JoinRoom {
                code: RoomCode::parse("k7m2qx").unwrap(),
                name: None,
                handover: Some("x".repeat(64)),
            },
            ClientMsg::SetJoinMode {
                join_mode: JoinMode::Locked,
            },
            ClientMsg::JoinDecision {
                id: Some(3),
                allow: true,
                block: false,
            },
            ClientMsg::JoinDecision {
                id: None,
                allow: false,
                block: false,
            },
            ClientMsg::JoinDecision {
                id: Some(3),
                allow: false,
                block: true,
            },
            ClientMsg::RemovePeer { block: false },
            ClientMsg::RemovePeer { block: true },
            ClientMsg::Unblock { id: 2 },
            ClientMsg::EndRoom,
            ClientMsg::CancelJoin,
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
                join_mode: JoinMode::Confirm,
            },
            ServerMsg::Joined { room_name: None },
            ServerMsg::Joined {
                room_name: Some("Meeting room Elbe".into()),
            },
            ServerMsg::PeerJoined {
                name: "Ana".into(),
                handover: None,
            },
            ServerMsg::PeerJoined {
                name: "Ana".into(),
                handover: Some("tok_en-1".into()),
            },
            ServerMsg::JoinPending {
                timeout_secs: 60,
                check: 42,
                room_name: None,
            },
            ServerMsg::JoinPending {
                timeout_secs: 60,
                check: 42,
                room_name: Some("Wohnzimmer".into()),
            },
            ServerMsg::JoinRequest {
                id: 1,
                name: "Ana".into(),
                check: 42,
                handover: None,
            },
            ServerMsg::JoinRequest {
                id: 2,
                name: "Ana".into(),
                check: 43,
                handover: Some("tok_en-1".into()),
            },
            ServerMsg::JoinRequestCancelled {
                id: 1,
                reason: "timeout".into(),
            },
            ServerMsg::JoinModeChanged {
                join_mode: JoinMode::Open,
            },
            ServerMsg::Blocked {
                id: 1,
                name: "Ana".into(),
            },
            ServerMsg::Unblocked { id: 1 },
            ServerMsg::RoomEnded,
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
        let ClientMsg::JoinRoom {
            code,
            name,
            handover,
        } = serde_json::from_str(j).unwrap()
        else {
            panic!("wrong variant");
        };
        assert_eq!(code.as_str(), "K7M2QX");
        assert_eq!(name, None);
        assert_eq!(handover, None);
    }

    #[test]
    fn join_approval_wire_format_is_stable() {
        let j = |m: &ClientMsg| serde_json::to_string(m).unwrap();
        assert_eq!(
            j(&ClientMsg::CreateRoom {
                altcha: None,
                join_mode: Some(JoinMode::Confirm),
                room_name: None
            }),
            r#"{"type":"create_room","join_mode":"confirm"}"#
        );
        assert_eq!(
            j(&ClientMsg::CreateRoom {
                altcha: None,
                join_mode: Some(JoinMode::Open),
                room_name: Some("Room 204".into())
            }),
            r#"{"type":"create_room","join_mode":"open","room_name":"Room 204"}"#
        );
        assert_eq!(
            j(&ClientMsg::JoinRoom {
                code: RoomCode::parse("K7M2QX").unwrap(),
                name: Some("Ana".into()),
                handover: None
            }),
            r#"{"type":"join_room","code":"K7M2QX","name":"Ana"}"#
        );
        assert_eq!(
            j(&ClientMsg::JoinRoom {
                code: RoomCode::parse("K7M2QX").unwrap(),
                name: Some("Ana".into()),
                handover: Some("AbC-_123".into())
            }),
            r#"{"type":"join_room","code":"K7M2QX","name":"Ana","handover":"AbC-_123"}"#
        );
        assert_eq!(
            j(&ClientMsg::SetJoinMode {
                join_mode: JoinMode::Locked
            }),
            r#"{"type":"set_join_mode","join_mode":"locked"}"#
        );
        assert_eq!(
            j(&ClientMsg::JoinDecision {
                id: Some(2),
                allow: true,
                block: false
            }),
            r#"{"type":"join_decision","id":2,"allow":true}"#
        );
        assert_eq!(
            j(&ClientMsg::JoinDecision {
                id: None,
                allow: false,
                block: false
            }),
            r#"{"type":"join_decision","allow":false}"#
        );
        assert_eq!(
            j(&ClientMsg::JoinDecision {
                id: Some(2),
                allow: false,
                block: true
            }),
            r#"{"type":"join_decision","id":2,"allow":false,"block":true}"#
        );
        assert_eq!(
            j(&ClientMsg::RemovePeer { block: false }),
            r#"{"type":"remove_peer"}"#
        );
        assert_eq!(
            j(&ClientMsg::RemovePeer { block: true }),
            r#"{"type":"remove_peer","block":true}"#
        );
        assert_eq!(
            j(&ClientMsg::Unblock { id: 4 }),
            r#"{"type":"unblock","id":4}"#
        );
        assert_eq!(j(&ClientMsg::EndRoom), r#"{"type":"end_room"}"#);
        assert_eq!(j(&ClientMsg::CancelJoin), r#"{"type":"cancel_join"}"#);
        let s = |m: &ServerMsg| serde_json::to_string(m).unwrap();
        assert_eq!(
            s(&ServerMsg::JoinRequest {
                id: 7,
                name: "Ana".into(),
                check: 42,
                handover: None
            }),
            r#"{"type":"join_request","id":7,"name":"Ana","check":42}"#
        );
        assert_eq!(
            s(&ServerMsg::JoinRequest {
                id: 7,
                name: "Ana".into(),
                check: 42,
                handover: Some("tok_en-1".into())
            }),
            r#"{"type":"join_request","id":7,"name":"Ana","check":42,"handover":"tok_en-1"}"#
        );
        assert_eq!(
            s(&ServerMsg::JoinPending {
                timeout_secs: 60,
                check: 42,
                room_name: None,
            }),
            r#"{"type":"join_pending","timeout_secs":60,"check":42}"#
        );
        assert_eq!(
            s(&ServerMsg::JoinPending {
                timeout_secs: 60,
                check: 42,
                room_name: Some("Room 204".into()),
            }),
            r#"{"type":"join_pending","timeout_secs":60,"check":42,"room_name":"Room 204"}"#
        );
        assert_eq!(
            s(&ServerMsg::JoinModeChanged {
                join_mode: JoinMode::Open
            }),
            r#"{"type":"join_mode_changed","join_mode":"open"}"#
        );
        assert_eq!(
            s(&ServerMsg::Blocked {
                id: 3,
                name: "Ana".into()
            }),
            r#"{"type":"blocked","id":3,"name":"Ana"}"#
        );
        assert_eq!(
            s(&ServerMsg::Unblocked { id: 3 }),
            r#"{"type":"unblocked","id":3}"#
        );
        assert_eq!(s(&ServerMsg::RoomEnded), r#"{"type":"room_ended"}"#);
        assert_eq!(
            s(&ServerMsg::PeerJoined {
                name: "Ana".into(),
                handover: None
            }),
            r#"{"type":"peer_joined","name":"Ana"}"#
        );
        assert!(
            serde_json::from_str::<ClientMsg>(r#"{"type":"set_join_mode","join_mode":"x"}"#)
                .is_err()
        );
    }

    #[test]
    fn join_mode_text() {
        assert_eq!("Confirm".parse::<JoinMode>(), Ok(JoinMode::Confirm));
        assert!("nope".parse::<JoinMode>().is_err());
        assert_eq!(JoinMode::Locked.to_string(), "locked");
    }

    #[test]
    fn display_names_are_sanitised() {
        assert_eq!(
            sanitize_display_name(Some("  Ana   Maria "), 1),
            "Ana Maria"
        );
        assert_eq!(sanitize_display_name(None, 4), "Guest 4");
        assert_eq!(sanitize_display_name(Some("   "), 2), "Guest 2");
        // Control, bidi, zero-width, line separators: removed.
        assert_eq!(
            sanitize_display_name(Some("A\u{202E}b\u{200B}c\n\u{0}d\u{2066}e\u{FEFF}"), 1),
            "Abc de"
        );
        assert_eq!(
            sanitize_display_name(Some("\u{202E}\u{202D}"), 9),
            "Guest 9"
        );
        // Length cut at 40 characters, also for multi-byte characters.
        let long = "ä".repeat(100);
        let n = sanitize_display_name(Some(&long), 1);
        assert_eq!(n.chars().count(), MAX_DISPLAY_NAME_CHARS);
        let spaced = format!("{} x", "a".repeat(39));
        assert_eq!(sanitize_display_name(Some(&spaced), 1), "a".repeat(39));
        // Markup is not interpreted here (clients must render as text), but stays harmless text.
        assert_eq!(sanitize_display_name(Some("<b>Ana</b>"), 1), "<b>Ana</b>");
    }

    #[test]
    fn new_messages_wire_format() {
        assert_eq!(
            serde_json::to_string(&ClientMsg::CreateRoom {
                altcha: None,
                join_mode: None,
                room_name: None
            })
            .unwrap(),
            r#"{"type":"create_room"}"#
        );
        assert_eq!(
            serde_json::to_string(&ClientMsg::CreateRoom {
                altcha: Some("e30=".into()),
                join_mode: None,
                room_name: None
            })
            .unwrap(),
            r#"{"type":"create_room","altcha":"e30="}"#
        );
        let created = ServerMsg::RoomCreated {
            code: RoomCode::parse("K7M2QX").unwrap(),
            join_mode: JoinMode::Open,
        };
        assert_eq!(
            serde_json::to_string(&created).unwrap(),
            r#"{"type":"room_created","code":"K7M2QX","join_mode":"open"}"#
        );
        assert_eq!(
            serde_json::to_string(&ServerMsg::Joined { room_name: None }).unwrap(),
            r#"{"type":"joined"}"#
        );
        assert_eq!(
            serde_json::to_string(&ServerMsg::Joined {
                room_name: Some("Room 204".into())
            })
            .unwrap(),
            r#"{"type":"joined","room_name":"Room 204"}"#
        );
        // An old server sends no `room_name`.
        let m: ServerMsg = serde_json::from_str(r#"{"type":"joined"}"#).unwrap();
        assert_eq!(m, ServerMsg::Joined { room_name: None });
        let m: ServerMsg =
            serde_json::from_str(r#"{"type":"join_pending","timeout_secs":60,"check":42}"#)
                .unwrap();
        assert_eq!(
            m,
            ServerMsg::JoinPending {
                timeout_secs: 60,
                check: 42,
                room_name: None
            }
        );
        assert_eq!(
            serde_json::to_string(&ServerMsg::PeerJoined {
                name: "Ana".into(),
                handover: None
            })
            .unwrap(),
            r#"{"type":"peer_joined","name":"Ana"}"#
        );
        assert_eq!(
            serde_json::to_string(&ServerMsg::PeerLeft).unwrap(),
            r#"{"type":"peer_left"}"#
        );
        let m: ClientMsg = serde_json::from_str(r#"{"type":"create_room"}"#).unwrap();
        assert_eq!(
            m,
            ClientMsg::CreateRoom {
                altcha: None,
                join_mode: None,
                room_name: None
            }
        );
        // Old clients send no `altcha` field; a null is accepted too.
        let m: ClientMsg = serde_json::from_str(r#"{"type":"create_room","altcha":null}"#).unwrap();
        assert_eq!(
            m,
            ClientMsg::CreateRoom {
                altcha: None,
                join_mode: None,
                room_name: None
            }
        );
        let m: ClientMsg =
            serde_json::from_str(r#"{"type":"create_room","altcha":"e30="}"#).unwrap();
        assert_eq!(
            m,
            ClientMsg::CreateRoom {
                altcha: Some("e30=".into()),
                join_mode: None,
                room_name: None
            }
        );
        let m: ClientMsg =
            serde_json::from_str(r#"{"type":"create_room","room_name":"Raum 204"}"#).unwrap();
        assert_eq!(
            m,
            ClientMsg::CreateRoom {
                altcha: None,
                join_mode: None,
                room_name: Some("Raum 204".into())
            }
        );
    }

    #[test]
    fn room_names_are_sanitised() {
        assert_eq!(sanitize_room_name(None), None);
        assert_eq!(sanitize_room_name(Some("")), None);
        assert_eq!(sanitize_room_name(Some("  \t\n ")), None);
        assert_eq!(sanitize_room_name(Some("\u{202E}\u{200B}\u{0}")), None);
        assert_eq!(
            sanitize_room_name(Some("  Meeting   room\u{202E} Elbe\u{200B} ")).as_deref(),
            Some("Meeting room Elbe")
        );
        assert_eq!(
            sanitize_room_name(Some("A\u{0}\nB\u{FEFF}\u{E000}")).as_deref(),
            Some("A B")
        );
        // Cut at 32 characters, also for multi-byte characters; no trailing space.
        let long = "ä".repeat(100);
        assert_eq!(
            sanitize_room_name(Some(&long)).unwrap().chars().count(),
            MAX_ROOM_NAME_CHARS
        );
        let spaced = format!("{} x", "a".repeat(31));
        assert_eq!(
            sanitize_room_name(Some(&spaced)).as_deref(),
            Some("a".repeat(31).as_str())
        );
    }

    #[test]
    fn new_error_codes_are_stable() {
        assert_eq!(error_codes::ALTCHA_REQUIRED, "altcha_required");
        assert_eq!(error_codes::ALTCHA_INVALID, "altcha_invalid");
        assert_eq!(error_codes::RATE_LIMITED, "rate_limited");
        assert_eq!(error_codes::BANNED, "banned");
        assert_eq!(error_codes::JOIN_PENDING, "join_pending");
        assert_eq!(error_codes::JOIN_DENIED, "join_denied");
        assert_eq!(error_codes::ROOM_LOCKED, "room_locked");
        assert_eq!(error_codes::APPROVAL_TIMEOUT, "approval_timeout");
    }

    #[test]
    fn invalid_room_code_fails_to_deserialize() {
        let j = r#"{"type":"join_room","code":"bad"}"#;
        assert!(serde_json::from_str::<ClientMsg>(j).is_err());
    }

    #[test]
    fn handover_tokens_are_bounded_and_base64url() {
        assert_eq!(
            sanitize_handover(Some("AbC-_123")),
            Some("AbC-_123".to_owned())
        );
        // A token of 128 random bits as base64url is 22 characters.
        assert!(sanitize_handover(Some("0123456789abcdefABCDEF")).is_some());
        assert!(sanitize_handover(Some(&"a".repeat(MAX_HANDOVER_CHARS))).is_some());
        assert_eq!(
            sanitize_handover(Some(&"a".repeat(MAX_HANDOVER_CHARS + 1))),
            None
        );
        for bad in [
            "",
            " ",
            "a b",
            "a+b",
            "a/b",
            "a=",
            "\u{e4}",
            "a\n",
            "{\"x\":1}",
        ] {
            assert_eq!(sanitize_handover(Some(bad)), None, "{bad:?}");
        }
        assert_eq!(sanitize_handover(None), None);
    }
}
