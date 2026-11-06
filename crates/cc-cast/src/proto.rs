// SPDX-License-Identifier: GPL-3.0-or-later
//! The Cast v2 channel protobuf messages (`CastMessage` and the device-auth messages).
//!
//! The field numbers, types and labels follow
//! `cast/common/channel/proto/cast_channel.proto` of the Open Screen Library, commit
//! `8b62b21479f1adb7663f182d97962b960c5c64ef` (BSD-3-Clause, Copyright 2015 The Chromium
//! Authors; see THIRD_PARTY_NOTICES.md). The structs are hand-written with `prost` derive
//! macros, so no `protoc` and no build script are needed.
//!
//! Only the messages this sender uses are defined.

use prost::Message;

/// `CastMessage.ProtocolVersion`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum ProtocolVersion {
    /// `CASTV2_1_0`.
    V1_0 = 0,
    /// `CASTV2_1_1` (chunking, deprecated).
    V1_1 = 1,
    /// `CASTV2_1_2`.
    V1_2 = 2,
    /// `CASTV2_1_3`.
    V1_3 = 3,
}

/// `CastMessage.PayloadType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum PayloadType {
    /// UTF-8 text (JSON for almost all namespaces).
    String = 0,
    /// Binary payload (device auth).
    Binary = 1,
}

/// One message on the Cast channel.
#[derive(Clone, PartialEq, Message)]
pub struct CastMessage {
    /// Protocol version, see [`ProtocolVersion`].
    #[prost(enumeration = "ProtocolVersion", required, tag = "1")]
    pub protocol_version: i32,
    /// Who sent it (`sender-0` for the platform, or a per-connection id).
    #[prost(string, required, tag = "2")]
    pub source_id: String,
    /// Who it is for (`receiver-0`, a transport id, or `*`).
    #[prost(string, required, tag = "3")]
    pub destination_id: String,
    /// The multiplexing key, e.g. `urn:x-cast:com.google.cast.receiver`.
    #[prost(string, required, tag = "4")]
    pub namespace: String,
    /// See [`PayloadType`].
    #[prost(enumeration = "PayloadType", required, tag = "5")]
    pub payload_type: i32,
    /// Text payload.
    #[prost(string, optional, tag = "6")]
    pub payload_utf8: Option<String>,
    /// Binary payload.
    #[prost(bytes = "vec", optional, tag = "7")]
    pub payload_binary: Option<Vec<u8>>,
}

impl CastMessage {
    /// A text message (CASTV2_1_0).
    pub fn text(source: &str, destination: &str, namespace: &str, payload: String) -> Self {
        Self {
            protocol_version: ProtocolVersion::V1_0 as i32,
            source_id: source.to_owned(),
            destination_id: destination.to_owned(),
            namespace: namespace.to_owned(),
            payload_type: PayloadType::String as i32,
            payload_utf8: Some(payload),
            payload_binary: None,
        }
    }

    /// A binary message (CASTV2_1_0).
    pub fn binary(source: &str, destination: &str, namespace: &str, payload: Vec<u8>) -> Self {
        Self {
            protocol_version: ProtocolVersion::V1_0 as i32,
            source_id: source.to_owned(),
            destination_id: destination.to_owned(),
            namespace: namespace.to_owned(),
            payload_type: PayloadType::Binary as i32,
            payload_utf8: None,
            payload_binary: Some(payload),
        }
    }

    /// The text payload. Open Screen notes that receivers may fill either field
    /// (crbug.com/1429410), so a text message that only has the binary field is read from it.
    pub fn text_payload(&self) -> Option<String> {
        if let Some(t) = self.payload_utf8.as_deref().filter(|t| !t.is_empty()) {
            return Some(t.to_owned());
        }
        self.payload_binary
            .as_deref()
            .and_then(|b| std::str::from_utf8(b).ok())
            .map(str::to_owned)
    }
}

/// `SignatureAlgorithm`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum SignatureAlgorithm {
    /// `UNSPECIFIED`.
    Unspecified = 0,
    /// `RSASSA_PKCS1v15` (the proto default).
    RsassaPkcs1v15 = 1,
    /// `RSASSA_PSS`.
    RsassaPss = 2,
}

/// `HashAlgorithm`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum HashAlgorithm {
    /// `SHA1` (the proto default).
    Sha1 = 0,
    /// `SHA256`.
    Sha256 = 1,
}

/// `AuthChallenge`.
#[derive(Clone, PartialEq, Message)]
pub struct AuthChallenge {
    /// Default (when absent): `RSASSA_PKCS1v15`.
    #[prost(enumeration = "SignatureAlgorithm", optional, tag = "1")]
    pub signature_algorithm: Option<i32>,
    /// The sender's random nonce.
    #[prost(bytes = "vec", optional, tag = "2")]
    pub sender_nonce: Option<Vec<u8>>,
    /// Default (when absent): `SHA1`.
    #[prost(enumeration = "HashAlgorithm", optional, tag = "3")]
    pub hash_algorithm: Option<i32>,
}

/// `AuthResponse`.
#[derive(Clone, PartialEq, Message)]
pub struct AuthResponse {
    /// Signature over `nonce || DER of the TLS certificate`.
    #[prost(bytes = "vec", required, tag = "1")]
    pub signature: Vec<u8>,
    /// The device certificate (DER) whose key made the signature.
    #[prost(bytes = "vec", required, tag = "2")]
    pub client_auth_certificate: Vec<u8>,
    /// Intermediate certificates (DER).
    #[prost(bytes = "vec", repeated, tag = "3")]
    pub intermediate_certificate: Vec<Vec<u8>>,
    /// Default (when absent): `RSASSA_PKCS1v15`.
    #[prost(enumeration = "SignatureAlgorithm", optional, tag = "4")]
    pub signature_algorithm: Option<i32>,
    /// The nonce echoed back.
    #[prost(bytes = "vec", optional, tag = "5")]
    pub sender_nonce: Option<Vec<u8>>,
    /// Default (when absent): `SHA1`.
    #[prost(enumeration = "HashAlgorithm", optional, tag = "6")]
    pub hash_algorithm: Option<i32>,
    /// Certificate revocation list (not evaluated by this sender).
    #[prost(bytes = "vec", optional, tag = "7")]
    pub crl: Option<Vec<u8>>,
}

/// `AuthError`.
#[derive(Clone, PartialEq, Message)]
pub struct AuthError {
    /// 0 `INTERNAL_ERROR`, 1 `NO_TLS`, 2 `SIGNATURE_ALGORITHM_UNAVAILABLE`.
    #[prost(int32, required, tag = "1")]
    pub error_type: i32,
}

/// `DeviceAuthMessage`.
#[derive(Clone, PartialEq, Message)]
pub struct DeviceAuthMessage {
    /// Request.
    #[prost(message, optional, tag = "1")]
    pub challenge: Option<AuthChallenge>,
    /// Response.
    #[prost(message, optional, tag = "2")]
    pub response: Option<AuthResponse>,
    /// Error.
    #[prost(message, optional, tag = "3")]
    pub error: Option<AuthError>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cast_message_round_trip_and_known_bytes() {
        let m = CastMessage::text("sender-0", "receiver-0", "urn:x-cast:a", "{}".into());
        let bytes = m.encode_to_vec();
        // Hand-checked against the .proto: field 1 varint 0, field 2 string, ...
        assert_eq!(&bytes[..4], &[0x08, 0x00, 0x12, 0x08]);
        assert_eq!(CastMessage::decode(bytes.as_slice()).unwrap(), m);
    }

    #[test]
    fn required_fields_are_always_written() {
        // An empty source id must still be encoded because the field is `required`.
        let m = CastMessage::text("", "", "", String::new());
        let bytes = m.encode_to_vec();
        assert_eq!(&bytes[..6], &[0x08, 0x00, 0x12, 0x00, 0x1a, 0x00]);
    }

    #[test]
    fn text_payload_falls_back_to_binary() {
        let mut m = CastMessage::binary("a", "b", "c", b"{\"x\":1}".to_vec());
        m.payload_type = PayloadType::String as i32;
        assert_eq!(m.text_payload().as_deref(), Some("{\"x\":1}"));
    }
}
