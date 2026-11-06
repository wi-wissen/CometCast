// SPDX-License-Identifier: GPL-3.0-or-later
//! The encrypted control channel of HomeKit-style AirPlay: after pairing, the bytes of the
//! TCP connection are cut into frames of at most 1024 bytes, each sent as
//! `length (2 bytes, little endian) | ChaCha20-Poly1305 ciphertext | tag (16 bytes)`
//! with the length as additional data and an 8-byte little-endian counter (padded with four
//! zero bytes in front) as nonce, one counter per direction.
//!
//! Source: pyatv `auth/hap_session.py` and `support/chacha20.py` (MIT), and the HomeKit
//! Accessory Protocol description of the session encryption.
//! UNCERTAIN: not checked against a real Apple receiver.

use std::io::{self, Read, Write};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

/// Largest plaintext per frame when sending.
pub const FRAME_PLAIN_MAX: usize = 1024;
const TAG: usize = 16;

/// Two directions of ChaCha20-Poly1305 with counters.
pub struct Cipher {
    out: ChaCha20Poly1305,
    inn: ChaCha20Poly1305,
    out_ctr: u64,
    in_ctr: u64,
}

fn nonce(counter: u64) -> Nonce {
    let mut n = [0u8; 12];
    n[4..].copy_from_slice(&counter.to_le_bytes());
    *Nonce::from_slice(&n)
}

impl Cipher {
    /// `out_key` encrypts what we send, `in_key` decrypts what we receive.
    pub fn new(out_key: &[u8; 32], in_key: &[u8; 32]) -> Self {
        Self {
            out: ChaCha20Poly1305::new(Key::from_slice(out_key)),
            inn: ChaCha20Poly1305::new(Key::from_slice(in_key)),
            out_ctr: 0,
            in_ctr: 0,
        }
    }

    fn seal(&mut self, plain: &[u8]) -> io::Result<Vec<u8>> {
        let len = (plain.len() as u16).to_le_bytes();
        let ct = self
            .out
            .encrypt(
                &nonce(self.out_ctr),
                Payload {
                    msg: plain,
                    aad: &len,
                },
            )
            .map_err(|_| io::Error::other("encryption failed"))?;
        self.out_ctr = self.out_ctr.wrapping_add(1);
        let mut frame = len.to_vec();
        frame.extend_from_slice(&ct);
        Ok(frame)
    }

    fn open(&mut self, len: [u8; 2], ct: &[u8]) -> io::Result<Vec<u8>> {
        let plain = self
            .inn
            .decrypt(&nonce(self.in_ctr), Payload { msg: ct, aad: &len })
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad frame authentication"))?;
        self.in_ctr = self.in_ctr.wrapping_add(1);
        Ok(plain)
    }
}

/// A stream that is plain until [`Framed::enable`] is called.
pub struct Framed<S> {
    inner: S,
    cipher: Option<Cipher>,
    raw: Vec<u8>,
    plain: Vec<u8>,
    plain_pos: usize,
}

impl<S> Framed<S> {
    /// Wrap a stream; bytes pass through unchanged.
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            cipher: None,
            raw: Vec::new(),
            plain: Vec::new(),
            plain_pos: 0,
        }
    }

    /// Switch to the encrypted framing. Call only between messages (nothing buffered).
    pub fn enable(&mut self, cipher: Cipher) {
        self.raw.clear();
        self.plain.clear();
        self.plain_pos = 0;
        self.cipher = Some(cipher);
    }

    /// Whether encryption is on.
    pub fn is_encrypted(&self) -> bool {
        self.cipher.is_some()
    }

    /// The wrapped stream.
    pub fn get_ref(&self) -> &S {
        &self.inner
    }
}

impl<S: Read> Read for Framed<S> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let Some(cipher) = self.cipher.as_mut() else {
            return self.inner.read(out);
        };
        if out.is_empty() {
            return Ok(0);
        }
        loop {
            if self.plain_pos < self.plain.len() {
                let n = out.len().min(self.plain.len() - self.plain_pos);
                out[..n].copy_from_slice(&self.plain[self.plain_pos..self.plain_pos + n]);
                self.plain_pos += n;
                return Ok(n);
            }
            self.plain.clear();
            self.plain_pos = 0;
            if self.raw.len() >= 2 {
                let len = u16::from_le_bytes([self.raw[0], self.raw[1]]) as usize;
                let total = 2 + len + TAG;
                if self.raw.len() >= total {
                    let frame: Vec<u8> = self.raw.drain(..total).collect();
                    self.plain = cipher.open([frame[0], frame[1]], &frame[2..])?;
                    continue;
                }
            }
            let mut chunk = [0u8; 4096];
            let n = self.inner.read(&mut chunk)?;
            if n == 0 {
                return Ok(0);
            }
            self.raw.extend_from_slice(&chunk[..n]);
        }
    }
}

impl<S: Write> Write for Framed<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let Some(cipher) = self.cipher.as_mut() else {
            return self.inner.write(buf);
        };
        let mut wire = Vec::with_capacity(buf.len() + buf.len() / FRAME_PLAIN_MAX * 18 + 18);
        for chunk in buf.chunks(FRAME_PLAIN_MAX) {
            wire.extend(cipher.seal(chunk)?);
        }
        self.inner.write_all(&wire)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn roundtrip_across_several_frames() {
        let (k1, k2) = ([1u8; 32], [2u8; 32]);
        let data: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        let mut sender = Framed::new(Vec::new());
        sender.enable(Cipher::new(&k1, &k2));
        sender.write_all(&data).unwrap();
        let wire = sender.inner.clone();
        assert!(wire.len() > data.len());
        let mut receiver = Framed::new(Cursor::new(wire));
        receiver.enable(Cipher::new(&k2, &k1));
        let mut got = Vec::new();
        receiver.read_to_end(&mut got).unwrap();
        assert_eq!(got, data);
    }

    #[test]
    fn tampering_and_wrong_key_are_errors() {
        let mut sender = Framed::new(Vec::new());
        sender.enable(Cipher::new(&[1; 32], &[2; 32]));
        sender.write_all(b"hello").unwrap();
        let mut wire = sender.inner.clone();
        let last = wire.len() - 1;
        wire[last] ^= 1;
        let mut r = Framed::new(Cursor::new(wire));
        r.enable(Cipher::new(&[2; 32], &[1; 32]));
        let mut buf = [0u8; 10];
        assert!(r.read(&mut buf).is_err());
    }

    #[test]
    fn truncated_frame_ends_without_panic() {
        let mut sender = Framed::new(Vec::new());
        sender.enable(Cipher::new(&[1; 32], &[2; 32]));
        sender.write_all(b"hello world").unwrap();
        let mut wire = sender.inner.clone();
        wire.truncate(wire.len() - 3);
        let mut r = Framed::new(Cursor::new(wire));
        r.enable(Cipher::new(&[2; 32], &[1; 32]));
        let mut got = Vec::new();
        assert_eq!(r.read_to_end(&mut got).unwrap(), 0);
    }

    #[test]
    fn plain_mode_passes_through() {
        let mut f = Framed::new(Cursor::new(b"abc".to_vec()));
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        assert_eq!(s, "abc");
        assert!(!f.is_encrypted());
    }
}
