// SPDX-License-Identifier: GPL-3.0-or-later
//! Small wrappers around the RustCrypto crates used by pairing.

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use sha2::{Digest, Sha512};

use crate::error::{Result, SenderError};

/// HKDF-SHA-512 with a 32-byte output.
pub fn hkdf32(ikm: &[u8], salt: &str, info: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    // expand() only fails for outputs over 255 hash lengths; 32 bytes cannot.
    let _ = Hkdf::<Sha512>::new(Some(salt.as_bytes()), ikm).expand(info.as_bytes(), &mut out);
    out
}

/// SHA-512 over several parts.
pub fn sha512(parts: &[&[u8]]) -> [u8; 64] {
    let mut d = Sha512::new();
    for p in parts {
        d.update(p);
    }
    let mut out = [0u8; 64];
    let full = d.finalize();
    out.copy_from_slice(AsRef::<[u8]>::as_ref(&full));
    out
}

fn nonce8(label: &[u8; 8]) -> Nonce {
    let mut n = [0u8; 12];
    n[4..].copy_from_slice(label);
    *Nonce::from_slice(&n)
}

/// ChaCha20-Poly1305 with a fixed 8-byte label as nonce (padded with four zero bytes), as
/// the pairing messages `PS-Msg05`, `PS-Msg06`, `PV-Msg02`, `PV-Msg03` do.
pub fn seal_labelled(key: &[u8; 32], label: &[u8; 8], plain: &[u8]) -> Result<Vec<u8>> {
    ChaCha20Poly1305::new(Key::from_slice(key))
        .encrypt(&nonce8(label), plain)
        .map_err(|_| SenderError::Auth("encryption failed".into()))
}

/// Inverse of [`seal_labelled`].
pub fn open_labelled(key: &[u8; 32], label: &[u8; 8], sealed: &[u8]) -> Result<Vec<u8>> {
    ChaCha20Poly1305::new(Key::from_slice(key))
        .decrypt(&nonce8(label), sealed)
        .map_err(|_| SenderError::Auth("the receiver's encrypted answer did not verify".into()))
}

/// Random bytes from `rand::rng()`: the thread-local `ThreadRng`, a ChaCha-based CSPRNG that
/// is seeded from the operating system and reseeded regularly (not a direct `getrandom`
/// call per use). Fine for keys and nonces here.
pub fn random<const N: usize>() -> [u8; N] {
    use rand::RngCore;
    let mut b = [0u8; N];
    rand::rng().fill_bytes(&mut b);
    b
}

/// Lower-case hex.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Parse hex of an exact length.
pub fn unhex<const N: usize>(s: &str) -> Option<[u8; N]> {
    if s.len() != N * 2 || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Parse hex of any even length up to `max` bytes.
pub fn unhex_vec(s: &str, max: usize) -> Option<Vec<u8>> {
    if s.len() & 1 == 1 || s.len() > max * 2 || !s.is_ascii() {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labelled_seal_roundtrip_and_failure() {
        let k = [3u8; 32];
        let sealed = seal_labelled(&k, b"PS-Msg05", b"hello").unwrap();
        assert_eq!(open_labelled(&k, b"PS-Msg05", &sealed).unwrap(), b"hello");
        assert!(open_labelled(&k, b"PS-Msg06", &sealed).is_err());
        assert!(open_labelled(&[4; 32], b"PS-Msg05", &sealed).is_err());
        assert!(open_labelled(&k, b"PS-Msg05", &[1, 2]).is_err());
    }

    #[test]
    fn hex_helpers() {
        assert_eq!(hex(&[0, 255, 16]), "00ff10");
        assert_eq!(unhex::<3>("00ff10"), Some([0, 255, 16]));
        assert_eq!(unhex::<3>("00ff1"), None);
        assert_eq!(unhex::<1>("zz"), None);
        assert_eq!(unhex_vec("abcd", 2), Some(vec![0xab, 0xcd]));
        assert_eq!(unhex_vec("abcdef", 2), None);
        assert_eq!(unhex_vec("é1", 4), None);
    }

    #[test]
    fn hkdf_is_deterministic_and_separates_inputs() {
        let a = hkdf32(b"k", "salt", "a");
        assert_eq!(a, hkdf32(b"k", "salt", "a"));
        assert_ne!(a, hkdf32(b"k", "salt", "b"));
    }
}
