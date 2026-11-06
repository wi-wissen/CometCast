// SPDX-License-Identifier: GPL-3.0-or-later
//! Binary property list helpers (AirPlay request and answer bodies).

use std::io::Cursor;

use plist::{Dictionary, Value};

use crate::error::{Result, SenderError};

/// Plists we accept from a receiver are at most this big.
///
/// UNCERTAIN: the `plist` crate's own limits against hostile nesting or huge declared
/// counts were not audited; the input size cap bounds the work but not every allocation.
pub const MAX_PLIST_BYTES: usize = 1024 * 1024;

/// Content type of binary plists.
pub const BPLIST_TYPE: &str = "application/x-apple-binary-plist";

/// Encode a dictionary as a binary plist.
pub fn encode(dict: Dictionary) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    Value::Dictionary(dict)
        .to_writer_binary(&mut out)
        .map_err(|e| SenderError::Input(format!("cannot encode plist: {e}")))?;
    Ok(out)
}

/// Decode a binary or XML plist whose top level is a dictionary.
pub fn decode_dict(data: &[u8]) -> Result<Dictionary> {
    if data.len() > MAX_PLIST_BYTES {
        return Err(SenderError::TooLarge("property list"));
    }
    match Value::from_reader(Cursor::new(data)) {
        Ok(Value::Dictionary(d)) => Ok(d),
        Ok(_) => Err(SenderError::Protocol(
            "property list is not a dictionary".into(),
        )),
        Err(e) => Err(SenderError::Protocol(format!("bad property list: {e}"))),
    }
}

/// A byte value of a dictionary.
pub fn data<'a>(d: &'a Dictionary, key: &str) -> Option<&'a [u8]> {
    match d.get(key) {
        Some(Value::Data(b)) => Some(b.as_slice()),
        _ => None,
    }
}

/// A real number or integer value as `f64`.
pub fn number(d: &Dictionary, key: &str) -> Option<f64> {
    match d.get(key) {
        Some(Value::Real(r)) => Some(*r),
        Some(Value::Integer(i)) => i.as_signed().map(|v| v as f64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut d = Dictionary::new();
        d.insert("pk".into(), Value::Data(vec![1, 2, 3]));
        d.insert("duration".into(), Value::Real(12.5));
        let back = decode_dict(&encode(d).unwrap()).unwrap();
        assert_eq!(data(&back, "pk"), Some(&[1u8, 2, 3][..]));
        assert_eq!(number(&back, "duration"), Some(12.5));
        assert_eq!(number(&back, "pk"), None);
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(decode_dict(b"bplist00garbage").is_err());
        assert!(decode_dict(b"").is_err());
        assert!(decode_dict(&vec![0u8; MAX_PLIST_BYTES + 1]).is_err());
        let mut arr = Vec::new();
        Value::Array(vec![]).to_writer_binary(&mut arr).unwrap();
        assert!(decode_dict(&arr).is_err());
    }
}
