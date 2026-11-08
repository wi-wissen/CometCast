// SPDX-License-Identifier: GPL-3.0-or-later
//! Presenter device identity (`docs/design/presenter-identity.md`).
//!
//! A presenter installation has an Ed25519 key pair. With `join_room` it sends a [`JoinDevice`]:
//! its public key, a time stamp and the signature over [`join_signed_bytes`] (the ASCII string
//! `cometcast-join-v1`, a 0x00 byte, the room code in upper case, a 0x00 byte and the time stamp
//! in decimal). The name is **not** signed. The server relays the proof unchanged; the display
//! calls [`verify_join`]: strict signature check, and `|ts - now| <= 120 s`. The **device id** is
//! the lower-case hex of the first 16 bytes of SHA-256(public key), the same as the id of a
//! display in signed announcements.
//!
//! The shared vectors `tests/fixtures/join-proof.json` (made by `tools/gen-join-proof-vectors.py`)
//! pin every byte; the web app and the Apple TV code read the same file.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

use cc_proto::JoinDevice;

/// The domain string at the start of the signed bytes.
pub const JOIN_DOMAIN: &[u8] = b"cometcast-join-v1";
/// How far `ts` may be from the display's clock, in milliseconds (either direction).
pub const JOIN_WINDOW_MS: u64 = 120_000;
/// Length of a device id in hex digits.
pub const DEVICE_ID_LEN: usize = 32;

/// Why a proof was not accepted. A display treats every reason alike: the request is shown
/// without any device note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum JoinReject {
    /// The shape is wrong (lengths, spelling, not a public key).
    #[error("device proof: malformed")]
    Malformed,
    /// `ts` is further than [`JOIN_WINDOW_MS`] from the display's clock.
    #[error("device proof: time stamp outside the window")]
    Window,
    /// The signature does not verify for this room code.
    #[error("device proof: bad signature")]
    Signature,
}

/// The bytes that are signed: `cometcast-join-v1` 0x00 `<code, upper case>` 0x00 `<ts, decimal>`.
pub fn join_signed_bytes(code: &str, ts: u64) -> Vec<u8> {
    let mut out = JOIN_DOMAIN.to_vec();
    out.push(0);
    out.extend_from_slice(code.to_ascii_uppercase().as_bytes());
    out.push(0);
    out.extend_from_slice(ts.to_string().as_bytes());
    out
}

/// `id = lower-case hex of the first 16 bytes of SHA-256(public key)`.
pub fn device_id(pk: &[u8; 32]) -> String {
    let digest = Sha256::digest(pk);
    digest[..16].iter().map(|b| format!("{b:02x}")).collect()
}

/// The public key of a 32-byte secret (the seed the app stores).
pub fn public_key_of(secret: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(secret).verifying_key().to_bytes()
}

/// The device id of a 32-byte secret.
pub fn device_id_of_secret(secret: &[u8; 32]) -> String {
    device_id(&public_key_of(secret))
}

/// Make the proof of the holder of `secret` for joining room `code` at `ts` (milliseconds since
/// the Unix epoch).
pub fn sign_join(secret: &[u8; 32], code: &str, ts: u64) -> JoinDevice {
    let key = SigningKey::from_bytes(secret);
    let sig = key.sign(&join_signed_bytes(code, ts));
    JoinDevice {
        pk: URL_SAFE_NO_PAD.encode(key.verifying_key().to_bytes()),
        ts,
        sig: URL_SAFE_NO_PAD.encode(sig.to_bytes()),
    }
}

/// The secret of a presenter installation (its 32 seed bytes). The app stores the bytes with its
/// settings; the same key may be the display key of `signed-announcements.md`. The secret is not
/// shown by `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceKey([u8; 32]);

impl std::fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DeviceKey(id {})", self.id())
    }
}

impl DeviceKey {
    /// The key of a 32-byte secret; `None` for any other length.
    pub fn from_secret(secret: &[u8]) -> Option<Self> {
        <[u8; 32]>::try_from(secret).ok().map(Self)
    }

    /// The key of a secret written as base64url without padding (43 characters), the form the
    /// iOS app hands to the broadcast extension; `None` when it is not 32 bytes of that.
    pub fn from_secret_b64(text: &str) -> Option<Self> {
        let bytes = URL_SAFE_NO_PAD.decode(text.trim()).ok()?;
        Self::from_secret(&bytes)
    }

    /// The device id of the key.
    pub fn id(&self) -> String {
        device_id_of_secret(&self.0)
    }

    /// The proof for joining room `code` at `ts` (see [`sign_join`]).
    pub fn sign_join(&self, code: &str, ts: u64) -> JoinDevice {
        sign_join(&self.0, code, ts)
    }

    /// The proof for joining room `code` now.
    pub fn proof_now(&self, code: &str) -> JoinDevice {
        self.sign_join(code, unix_ms())
    }
}

/// Milliseconds since the Unix epoch now.
pub fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Check a proof for room `code` against the clock `now_ms`. Returns the device id of the public
/// key. Strict verification (`verify_strict`: no weak keys, no non-reduced signatures).
pub fn verify_join(device: &JoinDevice, code: &str, now_ms: u64) -> Result<String, JoinReject> {
    if !device.is_well_formed() {
        return Err(JoinReject::Malformed);
    }
    let pk: [u8; 32] = URL_SAFE_NO_PAD
        .decode(&device.pk)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or(JoinReject::Malformed)?;
    let sig: [u8; 64] = URL_SAFE_NO_PAD
        .decode(&device.sig)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or(JoinReject::Malformed)?;
    if device.ts.abs_diff(now_ms) > JOIN_WINDOW_MS {
        return Err(JoinReject::Window);
    }
    let vk = VerifyingKey::from_bytes(&pk).map_err(|_| JoinReject::Malformed)?;
    vk.verify_strict(
        &join_signed_bytes(code, device.ts),
        &Signature::from_bytes(&sig),
    )
    .map_err(|_| JoinReject::Signature)?;
    Ok(device_id(&pk))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: [u8; 32] = [7; 32];

    #[test]
    fn signed_bytes_have_the_documented_layout() {
        assert_eq!(
            join_signed_bytes("k7qm2w", 1767225600000),
            b"cometcast-join-v1\0K7QM2W\x001767225600000".to_vec()
        );
    }

    #[test]
    fn a_proof_verifies_for_its_room_and_time_only() {
        let now = 1_767_225_600_000;
        let d = sign_join(&SECRET, "K7QM2W", now);
        assert!(d.is_well_formed());
        assert_eq!(
            verify_join(&d, "k7qm2w", now),
            Ok(device_id_of_secret(&SECRET))
        );
        assert_eq!(verify_join(&d, "K7QM2X", now), Err(JoinReject::Signature));
        assert!(verify_join(&d, "K7QM2W", now + JOIN_WINDOW_MS).is_ok());
        assert!(verify_join(&d, "K7QM2W", now - JOIN_WINDOW_MS).is_ok());
        assert_eq!(
            verify_join(&d, "K7QM2W", now + JOIN_WINDOW_MS + 1),
            Err(JoinReject::Window)
        );
        assert_eq!(
            verify_join(&d, "K7QM2W", now - JOIN_WINDOW_MS - 1),
            Err(JoinReject::Window)
        );
    }

    #[test]
    fn a_device_key_signs_for_the_room_and_hides_its_secret() {
        let key = DeviceKey::from_secret(&SECRET).unwrap();
        assert!(DeviceKey::from_secret(&SECRET[..31]).is_none());
        let d = key.proof_now("K7QM2W");
        assert_eq!(
            verify_join(&d, "K7QM2W", unix_ms()),
            Ok(device_id_of_secret(&SECRET))
        );
        let text = URL_SAFE_NO_PAD.encode(SECRET);
        assert_eq!(DeviceKey::from_secret_b64(&text), Some(key.clone()));
        assert!(DeviceKey::from_secret_b64("short").is_none());
        let shown = format!("{key:?}");
        assert!(
            shown.contains(&key.id()) && !shown.contains(&text),
            "{shown}"
        );
    }

    #[test]
    fn a_changed_ts_or_signature_is_refused() {
        let now = 1_000_000;
        let mut d = sign_join(&SECRET, "K7QM2W", now);
        d.ts += 1;
        assert_eq!(verify_join(&d, "K7QM2W", now), Err(JoinReject::Signature));
        let mut d = sign_join(&SECRET, "K7QM2W", now);
        d.sig = sign_join(&[8; 32], "K7QM2W", now).sig;
        assert_eq!(verify_join(&d, "K7QM2W", now), Err(JoinReject::Signature));
    }
}
