// SPDX-License-Identifier: GPL-3.0-or-later
//! TLV8, the tag-length-value format of HomeKit-style pairing (one byte tag, one byte length,
//! values over 255 bytes are split into consecutive items with the same tag).
//!
//! Source of the tag numbers and the splitting rule: pyatv `auth/hap_tlv8.py` (MIT) and the
//! public description of the HomeKit Accessory Protocol pairing. Tags 0x11 (name) and 0x13
//! (flags) are Apple additions known from pyatv.
//! UNCERTAIN: tag meanings were not checked against a real Apple receiver.

use crate::error::{Result, SenderError};

/// Method (`0x00`).
pub const METHOD: u8 = 0x00;
/// Identifier (`0x01`).
pub const IDENTIFIER: u8 = 0x01;
/// Salt (`0x02`).
pub const SALT: u8 = 0x02;
/// Public key (`0x03`).
pub const PUBLIC_KEY: u8 = 0x03;
/// Proof (`0x04`).
pub const PROOF: u8 = 0x04;
/// Encrypted data (`0x05`).
pub const ENCRYPTED_DATA: u8 = 0x05;
/// Sequence number / state (`0x06`).
pub const SEQ_NO: u8 = 0x06;
/// Error (`0x07`).
pub const ERROR: u8 = 0x07;
/// Back-off seconds (`0x08`).
pub const BACK_OFF: u8 = 0x08;
/// Signature (`0x0A`).
pub const SIGNATURE: u8 = 0x0A;
/// Flags (`0x13`, Apple addition); `0x10` asks for transient pairing.
pub const FLAGS: u8 = 0x13;
/// Flag value for transient pairing.
pub const FLAG_TRANSIENT: u8 = 0x10;

/// Largest TLV buffer we parse (answers are untrusted).
pub const MAX_TLV_BYTES: usize = 64 * 1024;

/// Parsed TLV items in order. Fragments of the same value are already joined.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tlv(Vec<(u8, Vec<u8>)>);

impl Tlv {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an item (builder style).
    pub fn with(mut self, tag: u8, value: impl Into<Vec<u8>>) -> Self {
        self.0.push((tag, value.into()));
        self
    }

    /// First value with this tag.
    pub fn get(&self, tag: u8) -> Option<&[u8]> {
        self.0
            .iter()
            .find(|(t, _)| *t == tag)
            .map(|(_, v)| v.as_slice())
    }

    /// Like [`Tlv::get`] but a missing item is a protocol error.
    pub fn require(&self, tag: u8, what: &str) -> Result<&[u8]> {
        self.get(tag)
            .ok_or_else(|| SenderError::Protocol(format!("pairing answer lacks {what}")))
    }

    /// Serialise. A value over 255 bytes becomes several items; an empty value is one item of
    /// length 0.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for (tag, value) in &self.0 {
            if value.is_empty() {
                out.extend_from_slice(&[*tag, 0]);
                continue;
            }
            for chunk in value.chunks(255) {
                out.push(*tag);
                out.push(chunk.len() as u8);
                out.extend_from_slice(chunk);
            }
        }
        out
    }

    /// Parse. Fails on a truncated item or input over [`MAX_TLV_BYTES`].
    /// Consecutive items with the same tag where the earlier one is 255 bytes long are joined.
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() > MAX_TLV_BYTES {
            return Err(SenderError::TooLarge("pairing data"));
        }
        let mut items: Vec<(u8, Vec<u8>)> = Vec::new();
        let mut last_full = false;
        let mut pos = 0;
        while pos < data.len() {
            if pos + 2 > data.len() {
                return Err(SenderError::Protocol("truncated TLV header".into()));
            }
            let tag = data[pos];
            let len = data[pos + 1] as usize;
            let start = pos + 2;
            let end = start + len;
            if end > data.len() {
                return Err(SenderError::Protocol("truncated TLV value".into()));
            }
            let value = &data[start..end];
            match items.last_mut() {
                Some((t, v)) if *t == tag && last_full => v.extend_from_slice(value),
                _ => items.push((tag, value.to_vec())),
            }
            last_full = len == 255;
            pos = end;
        }
        Ok(Self(items))
    }

    /// If the answer carries an error item, turn it into an error.
    pub fn check_error(&self) -> Result<()> {
        if let Some(code) = self.get(ERROR) {
            let code = code.first().copied().unwrap_or(0);
            let text = match code {
                0x02 => "authentication failed (wrong PIN?)",
                0x03 => "too many tries, back off",
                0x04 => "maximum number of paired devices reached",
                0x05 => "maximum number of tries reached",
                0x06 => "receiver unavailable",
                0x07 => "receiver busy",
                _ => "unknown error code",
            };
            return Err(SenderError::Auth(format!(
                "receiver reported {text} ({code})"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_with_long_value() {
        let long = vec![7u8; 600];
        let t = Tlv::new()
            .with(SEQ_NO, [1])
            .with(PUBLIC_KEY, long.clone())
            .with(PROOF, [9; 64]);
        let back = Tlv::decode(&t.encode()).unwrap();
        assert_eq!(back.get(PUBLIC_KEY).unwrap(), long.as_slice());
        assert_eq!(back.get(SEQ_NO).unwrap(), &[1]);
        assert_eq!(back, t);
    }

    #[test]
    fn exactly_255_bytes_then_other_tag() {
        let t = Tlv::new().with(PUBLIC_KEY, vec![1u8; 255]).with(PROOF, [2]);
        assert_eq!(Tlv::decode(&t.encode()).unwrap(), t);
    }

    #[test]
    fn hostile_inputs_do_not_panic() {
        assert!(Tlv::decode(&[3]).is_err());
        assert!(Tlv::decode(&[3, 5, 1, 2]).is_err());
        assert!(Tlv::decode(&vec![0u8; MAX_TLV_BYTES + 1]).is_err());
        assert!(Tlv::decode(&[]).unwrap().get(1).is_none());
    }

    #[test]
    fn error_item_becomes_error() {
        let t = Tlv::new().with(ERROR, [2]);
        assert!(matches!(t.check_error(), Err(SenderError::Auth(_))));
        assert!(Tlv::new().check_error().is_ok());
    }
}
