// SPDX-License-Identifier: GPL-3.0-or-later
//! Signed display announcements, layout `v=2` (`docs/design/signed-announcements.md`,
//! `docs/discovery.md` section 3).
//!
//! * A display has an Ed25519 key pair. Its **id** is the first 16 bytes of
//!   `SHA-256(public key)` as 32 lower-case hex digits, so nobody can use an id without the
//!   private key. The **tag** is the first 4 characters of the id in upper case.
//! * A record carries `pk`, `seq`, `state` and `sig`. The signature covers the bytes built by
//!   [`signed_bytes`]: the ASCII string `cometcast-announce-v2`, a 0x00 byte, then for every key of
//!   [`SIGNED_KEYS`] in that fixed order `key=value` and a 0x00 byte; an absent optional key is
//!   written with an empty value.
//! * [`verify`] checks everything about a record except the `seq` memory (that is
//!   `nearby.rs`): version, well-formed values, `id` against `pk`, and the signature with strict
//!   verification. The code never writes a key anywhere: the app holds the 32 secret bytes.
//!
//! The shared vectors in `tests/fixtures/announce-v2.json` pin every byte; the Apple TV code
//! (Swift, CryptoKit) reads the same file.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

/// The layout version of signed records.
pub const LAYOUT_VERSION_V2: &str = "2";
/// The domain string at the start of the signed bytes.
pub const SIGN_DOMAIN: &[u8] = b"cometcast-announce-v2";
/// The keys that are signed, in the order of the signed bytes (`sig` is not part of it).
pub const SIGNED_KEYS: [&str; 10] = [
    "v", "id", "pk", "seq", "state", "srv", "tls", "mode", "code", "name",
];
/// Length of a secret key seed in bytes.
pub const SECRET_LEN: usize = 32;
/// Length of an id in hex digits.
pub const ID_LEN: usize = 32;
/// Length of the short tag in characters.
pub const TAG_LEN: usize = 4;
/// Length of `pk` as base64url without padding.
pub const PK_B64_LEN: usize = 43;
/// Length of `sig` as base64url without padding.
pub const SIG_B64_LEN: usize = 86;

/// `live`: the display is there; `gone`: the display stops announcing (it says so before the
/// mDNS goodbye).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnounceState {
    /// Announcing.
    Live,
    /// Leaving; the presenter removes the row.
    Gone,
}

impl AnnounceState {
    /// The TXT value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Gone => "gone",
        }
    }

    /// Parse the TXT value (exact).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "live" => Some(Self::Live),
            "gone" => Some(Self::Gone),
            _ => None,
        }
    }
}

/// Why a key could not be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    /// The secret is not 32 bytes.
    #[error("display key: the secret must be 32 bytes")]
    BadLength,
}

/// The key pair of a display. The secret is not shown by `Debug`.
#[derive(Clone)]
pub struct DisplayKey(SigningKey);

impl std::fmt::Debug for DisplayKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DisplayKey(id {})", self.id())
    }
}

impl DisplayKey {
    /// The key of a 32-byte secret (the app stores it).
    pub fn from_secret(secret: &[u8]) -> Result<Self, KeyError> {
        let bytes: [u8; SECRET_LEN] = secret.try_into().map_err(|_| KeyError::BadLength)?;
        Ok(Self(SigningKey::from_bytes(&bytes)))
    }

    /// 32 new secret bytes from the operating system's randomness. The caller stores them.
    pub fn generate_secret() -> [u8; SECRET_LEN] {
        use rand::TryRngCore;
        let mut b = [0u8; SECRET_LEN];
        // A machine whose operating system cannot give random bytes cannot make a key.
        rand::rngs::OsRng
            .try_fill_bytes(&mut b)
            .expect("the operating system gave no random bytes");
        b
    }

    /// A new random key (used for short-lived announcements, for example the command line tool).
    pub fn generate() -> Self {
        Self(SigningKey::from_bytes(&Self::generate_secret()))
    }

    /// The public key, 32 bytes.
    pub fn public_key(&self) -> [u8; 32] {
        self.0.verifying_key().to_bytes()
    }

    /// The public key as it is written in `pk`.
    pub fn public_key_b64(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.public_key())
    }

    /// The id derived from the public key.
    pub fn id(&self) -> String {
        id_from_public_key(&self.public_key())
    }

    /// The short tag (`3F7A`).
    pub fn tag(&self) -> String {
        tag_of(&self.id())
    }

    /// The signature over `message`, 64 bytes.
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.0.sign(message).to_bytes()
    }
}

/// `id = lower-case hex of the first 16 bytes of SHA-256(public key)`.
pub fn id_from_public_key(pk: &[u8; 32]) -> String {
    let digest = Sha256::digest(pk);
    let mut s = String::with_capacity(ID_LEN);
    for b in &digest[..16] {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// The short tag of an id: its first 4 characters in upper case (the whole id when shorter).
pub fn tag_of(id: &str) -> String {
    id.chars().take(TAG_LEN).collect::<String>().to_uppercase()
}

/// Is `id` exactly 32 lower-case hex digits?
pub fn is_valid_id(id: &str) -> bool {
    id.len() == ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The bytes that are signed. `value_of(key)` gives the exact value of a key, `None` for an
/// absent one (written as an empty value).
pub fn signed_bytes<'a>(value_of: impl Fn(&str) -> Option<&'a str>) -> Vec<u8> {
    let mut out = Vec::with_capacity(320);
    out.extend_from_slice(SIGN_DOMAIN);
    out.push(0);
    for key in SIGNED_KEYS {
        out.extend_from_slice(key.as_bytes());
        out.push(b'=');
        out.extend_from_slice(value_of(key).unwrap_or("").as_bytes());
        out.push(0);
    }
    out
}

/// Sign a record: the TXT pairs without `pk`, `seq`, `state`, `sig` are given as `fields`
/// (`v` is added). Returns the complete pairs in the order of [`SIGNED_KEYS`] plus `sig` last;
/// absent optional keys are left out of the pairs (and signed as empty).
pub fn sign_record(
    key: &DisplayKey,
    seq: u64,
    state: AnnounceState,
    fields: &[(&str, &str)],
) -> Vec<(String, String)> {
    let id = key.id();
    let pk = key.public_key_b64();
    let seq = seq.to_string();
    let lookup = |k: &str| -> Option<&str> {
        match k {
            "v" => Some(LAYOUT_VERSION_V2),
            "id" => Some(id.as_str()),
            "pk" => Some(pk.as_str()),
            "seq" => Some(seq.as_str()),
            "state" => Some(state.as_str()),
            other => fields.iter().find(|(f, _)| *f == other).map(|(_, v)| *v),
        }
    };
    let sig = URL_SAFE_NO_PAD.encode(key.sign(&signed_bytes(lookup)));
    let mut txt = Vec::with_capacity(SIGNED_KEYS.len() + 1);
    for k in SIGNED_KEYS {
        if let Some(v) = lookup(k).filter(|v| !v.is_empty()) {
            txt.push((k.to_owned(), v.to_owned()));
        }
    }
    txt.push(("sig".to_owned(), sig));
    txt
}

/// Why a record was not accepted. A refused record is ignored; it is never a reason to change
/// or remove a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject {
    /// No `v`, or not `2`.
    Version,
    /// A required key is missing, a value is malformed or too long, or a value has a NUL byte.
    Malformed,
    /// `id` is not the id derived from `pk`.
    IdMismatch,
    /// The signature does not verify.
    Signature,
}

/// The values of a record whose signature verified. Everything is still unchecked as far as the
/// *content* goes (`srv`, `mode`, `code`, `name` are checked by `nearby.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    /// The id (equal to the one derived from `pk`).
    pub id: String,
    /// The sequence number.
    pub seq: u64,
    /// `live` or `gone`.
    pub state: AnnounceState,
    /// `srv`.
    pub srv: String,
    /// `tls` is `1`.
    pub tls: bool,
    /// `mode`.
    pub mode: String,
    /// `code`, empty when absent.
    pub code: String,
    /// `name`, empty when absent.
    pub name: String,
}

/// Most TXT entries that are looked at.
const MAX_ENTRIES: usize = 32;
/// Longest value of one TXT entry (a DNS-SD entry has at most 255 bytes).
const MAX_VALUE_BYTES: usize = 255;

/// A canonical decimal `u64`: digits only, no sign, no leading zero (except `0` itself).
fn parse_seq(s: &str) -> Option<u64> {
    if s.is_empty() || (s.len() > 1 && s.starts_with('0')) || !s.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    s.parse().ok()
}

/// Check version, shape, id derivation and signature of a TXT record. The first occurrence of
/// a key wins (the same occurrence is signed and read). Only the first 32 entries are looked at.
pub fn verify(txt: &[(String, String)]) -> Result<Verified, Reject> {
    let get = |k: &str| -> Option<&str> {
        txt.iter()
            .take(MAX_ENTRIES)
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
    };
    if get("v") != Some(LAYOUT_VERSION_V2) {
        return Err(Reject::Version);
    }
    // Every value that goes into the signed bytes: bounded, no NUL.
    for k in SIGNED_KEYS.iter().chain(["sig"].iter()) {
        if let Some(v) = get(k)
            && (v.len() > MAX_VALUE_BYTES || v.contains('\0'))
        {
            return Err(Reject::Malformed);
        }
    }
    let id = get("id").ok_or(Reject::Malformed)?;
    let pk_b64 = get("pk").ok_or(Reject::Malformed)?;
    let seq_text = get("seq").ok_or(Reject::Malformed)?;
    let state = get("state")
        .and_then(AnnounceState::parse)
        .ok_or(Reject::Malformed)?;
    let srv = get("srv")
        .filter(|s| !s.is_empty())
        .ok_or(Reject::Malformed)?;
    let mode = get("mode")
        .filter(|s| !s.is_empty())
        .ok_or(Reject::Malformed)?;
    let sig_b64 = get("sig").ok_or(Reject::Malformed)?;
    let tls = match get("tls") {
        None | Some("") => false,
        Some("1") => true,
        Some(_) => return Err(Reject::Malformed),
    };
    if !is_valid_id(id) || pk_b64.len() != PK_B64_LEN || sig_b64.len() != SIG_B64_LEN {
        return Err(Reject::Malformed);
    }
    let seq = parse_seq(seq_text).ok_or(Reject::Malformed)?;
    let pk: [u8; 32] = URL_SAFE_NO_PAD
        .decode(pk_b64)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or(Reject::Malformed)?;
    let sig: [u8; 64] = URL_SAFE_NO_PAD
        .decode(sig_b64)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or(Reject::Malformed)?;
    // A non-canonical base64 spelling would verify but is not what a display writes.
    if URL_SAFE_NO_PAD.encode(pk) != pk_b64 || URL_SAFE_NO_PAD.encode(sig) != sig_b64 {
        return Err(Reject::Malformed);
    }
    if id_from_public_key(&pk) != id {
        return Err(Reject::IdMismatch);
    }
    let vk = VerifyingKey::from_bytes(&pk).map_err(|_| Reject::Signature)?;
    let message = signed_bytes(get);
    vk.verify_strict(&message, &Signature::from_bytes(&sig))
        .map_err(|_| Reject::Signature)?;
    Ok(Verified {
        id: id.to_owned(),
        seq,
        state,
        srv: srv.to_owned(),
        tls,
        mode: mode.to_owned(),
        code: get("code").unwrap_or("").to_owned(),
        name: get("name").unwrap_or("").to_owned(),
    })
}

/// Makes sequence numbers: `seq = max(unix time in ms, last + 1)`. `floor` is the highest value
/// that may have been used before (the app persists it with the key), so a clock that went back
/// after a restart cannot reuse values.
#[derive(Debug, Clone)]
pub struct SeqCounter {
    last: u64,
}

impl SeqCounter {
    /// A counter that continues after `floor`.
    pub fn new(floor: u64) -> Self {
        Self { last: floor }
    }

    /// The next value for the clock reading `now_ms`.
    pub fn next(&mut self, now_ms: u64) -> u64 {
        // UNCERTAIN: at u64::MAX the counter would stop growing; about 5e8 years of milliseconds.
        let n = now_ms.max(self.last.saturating_add(1));
        self.last = n;
        n
    }

    /// The last value handed out (or the floor).
    pub fn last(&self) -> u64 {
        self.last
    }
}

/// Unix time in milliseconds (0 if the clock is before 1970).
pub fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> DisplayKey {
        DisplayKey::from_secret(&[n; 32]).unwrap()
    }

    fn rec(k: &DisplayKey, seq: u64, state: AnnounceState) -> Vec<(String, String)> {
        sign_record(
            k,
            seq,
            state,
            &[
                ("srv", "10.0.0.5:8443"),
                ("mode", "open"),
                ("code", "P4X9TR"),
                ("name", "Room 204"),
            ],
        )
    }

    fn set(txt: &mut [(String, String)], k: &str, v: &str) {
        txt.iter_mut().find(|(key, _)| key == k).unwrap().1 = v.into();
    }

    #[test]
    fn id_is_the_first_16_bytes_of_sha256_of_the_public_key() {
        let k = key(1);
        let digest = Sha256::digest(k.public_key());
        let expect: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(k.id(), expect);
        assert_eq!(k.id().len(), 32);
        assert!(is_valid_id(&k.id()));
        assert_eq!(k.tag(), k.id()[..4].to_uppercase());
        assert_eq!(tag_of("3f7a99"), "3F7A");
        assert_eq!(k.public_key_b64().len(), PK_B64_LEN);
    }

    #[test]
    fn a_signed_record_verifies_and_has_the_fixed_key_order() {
        let k = key(2);
        let txt = rec(&k, 1_700_000_000_000, AnnounceState::Live);
        let keys: Vec<_> = txt.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys,
            [
                "v", "id", "pk", "seq", "state", "srv", "mode", "code", "name", "sig"
            ]
        );
        let v = verify(&txt).unwrap();
        assert_eq!(v.id, k.id());
        assert_eq!(v.seq, 1_700_000_000_000);
        assert_eq!(v.state, AnnounceState::Live);
        assert_eq!((v.srv.as_str(), v.mode.as_str()), ("10.0.0.5:8443", "open"));
        assert_eq!((v.code.as_str(), v.name.as_str()), ("P4X9TR", "Room 204"));
        assert!(!v.tls);
        assert_eq!(txt.last().unwrap().1.len(), SIG_B64_LEN);
    }

    #[test]
    fn signed_bytes_have_the_documented_layout() {
        let m = signed_bytes(|k| match k {
            "v" => Some("2"),
            "srv" => Some("a:1"),
            _ => None,
        });
        let expect = b"cometcast-announce-v2\0v=2\0id=\0pk=\0seq=\0state=\0srv=a:1\0tls=\0mode=\0code=\0name=\0";
        assert_eq!(m, expect);
    }

    #[test]
    fn every_changed_field_breaks_the_signature() {
        let k = key(3);
        let base = rec(&k, 10, AnnounceState::Live);
        for (field, new) in [
            ("seq", "11"),
            ("state", "gone"),
            ("srv", "10.0.0.6:8443"),
            ("mode", "approval"),
            ("code", "K7Q2MW"),
            ("name", "Room 205"),
        ] {
            let mut t = base.clone();
            set(&mut t, field, new);
            assert_eq!(verify(&t), Err(Reject::Signature), "{field}");
        }
        // Removing an optional key changes the signed bytes.
        for field in ["code", "name"] {
            let mut t = base.clone();
            t.retain(|(k, _)| k != field);
            assert_eq!(verify(&t), Err(Reject::Signature), "removed {field}");
        }
        // Adding tls=1 to a record signed without it.
        let mut t = base.clone();
        t.insert(5, ("tls".into(), "1".into()));
        assert_eq!(verify(&t), Err(Reject::Signature));
    }

    #[test]
    fn a_record_with_tls_verifies() {
        let k = key(3);
        let t = sign_record(
            &k,
            5,
            AnnounceState::Live,
            &[
                ("srv", "relay.example.org:443"),
                ("tls", "1"),
                ("mode", "locked"),
            ],
        );
        let v = verify(&t).unwrap();
        assert!(v.tls);
        assert_eq!(v.code, "");
    }

    #[test]
    fn id_must_come_from_the_key() {
        let k = key(4);
        let other = key(5);
        let mut t = rec(&k, 10, AnnounceState::Live);
        set(&mut t, "id", &other.id());
        assert_eq!(verify(&t), Err(Reject::IdMismatch));
        // The victim's id with the attacker's key and a valid signature by the attacker.
        let mut forged = sign_record(
            &other,
            99,
            AnnounceState::Live,
            &[("srv", "10.0.0.5:8443"), ("mode", "locked")],
        );
        set(&mut forged, "id", &k.id());
        assert_eq!(verify(&forged), Err(Reject::IdMismatch));
    }

    #[test]
    fn a_wrong_key_order_is_a_bad_signature() {
        // Signed with the keys in another order: refused.
        let k = key(6);
        let mut order = SIGNED_KEYS.to_vec();
        order.swap(5, 7); // srv <-> mode
        let id = k.id();
        let pk = k.public_key_b64();
        let val = |key: &str| match key {
            "v" => "2",
            "id" => id.as_str(),
            "pk" => pk.as_str(),
            "seq" => "10",
            "state" => "live",
            "srv" => "10.0.0.5:8443",
            "mode" => "open",
            "code" => "P4X9TR",
            "name" => "Room 204",
            _ => "",
        };
        let mut msg = SIGN_DOMAIN.to_vec();
        msg.push(0);
        for key in order {
            msg.extend_from_slice(format!("{key}={}", val(key)).as_bytes());
            msg.push(0);
        }
        let mut t = rec(&k, 10, AnnounceState::Live);
        set(&mut t, "sig", &URL_SAFE_NO_PAD.encode(k.sign(&msg)));
        assert_eq!(verify(&t), Err(Reject::Signature));
    }

    #[test]
    fn malformed_records_are_refused() {
        let k = key(7);
        let good = rec(&k, 10, AnnounceState::Live);
        type Txt = Vec<(String, String)>;
        let case = |f: &dyn Fn(&mut Txt)| {
            let mut t = good.clone();
            f(&mut t);
            verify(&t)
        };
        assert_eq!(case(&|t| set(t, "v", "1")), Err(Reject::Version));
        assert_eq!(case(&|t| t.retain(|(k, _)| k != "v")), Err(Reject::Version));
        for key in ["id", "pk", "seq", "state", "srv", "mode", "sig"] {
            assert_eq!(
                case(&|t| t.retain(|(k, _)| k != key)),
                Err(Reject::Malformed),
                "{key}"
            );
        }
        for bad in [
            "",
            "01",
            "+1",
            "-1",
            "1.0",
            "18446744073709551616",
            " 1",
            "0x10",
        ] {
            assert_eq!(
                case(&|t| set(t, "seq", bad)),
                Err(Reject::Malformed),
                "{bad:?}"
            );
        }
        assert_eq!(case(&|t| set(t, "state", "Live")), Err(Reject::Malformed));
        assert_eq!(case(&|t| set(t, "state", "")), Err(Reject::Malformed));
        assert_eq!(
            case(&|t| set(t, "id", &k.id().to_uppercase())),
            Err(Reject::Malformed)
        );
        assert_eq!(
            case(&|t| set(t, "id", &k.id()[..31])),
            Err(Reject::Malformed)
        );
        assert_eq!(case(&|t| set(t, "pk", "short")), Err(Reject::Malformed));
        assert_eq!(case(&|t| set(t, "sig", "short")), Err(Reject::Malformed));
        assert_eq!(
            case(&|t| t.insert(5, ("tls".into(), "yes".into()))),
            Err(Reject::Malformed)
        );
        assert_eq!(
            case(&|t| set(t, "name", "a\0b")),
            Err(Reject::Malformed),
            "NUL"
        );
        assert_eq!(
            case(&|t| set(t, "name", &"x".repeat(300))),
            Err(Reject::Malformed)
        );
        // Padding or the standard alphabet is not base64url without padding.
        let sig = good.last().unwrap().1.clone();
        assert_eq!(
            case(&|t| set(t, "sig", &format!("{}=", &sig[..85]))),
            Err(Reject::Malformed)
        );
        // Not a point on the curve or a small-order key: refused one way or the other.
        let weak = [0u8; 32];
        let mut t = good.clone();
        set(&mut t, "pk", &URL_SAFE_NO_PAD.encode(weak));
        set(&mut t, "id", &id_from_public_key(&weak));
        assert!(verify(&t).is_err());
    }

    #[test]
    fn the_first_occurrence_of_a_key_is_the_one_that_counts() {
        let k = key(8);
        let mut t = rec(&k, 10, AnnounceState::Live);
        // A second `code` after the signed record changes nothing and is not read.
        t.push(("code".into(), "ZZZZZZ".into()));
        assert_eq!(verify(&t).unwrap().code, "P4X9TR");
        // A first occurrence that differs from the signed one is refused.
        let mut t2 = rec(&k, 10, AnnounceState::Live);
        t2.insert(0, ("code".into(), "ZZZZZZ".into()));
        assert_eq!(verify(&t2), Err(Reject::Signature));
        // Entries beyond the first 32 are not looked at.
        let mut t3: Vec<(String, String)> =
            (0..40).map(|i| (format!("k{i}"), "x".into())).collect();
        t3.extend(rec(&k, 10, AnnounceState::Live));
        assert_eq!(verify(&t3), Err(Reject::Version));
    }

    #[test]
    fn seq_is_max_of_clock_and_last_plus_one() {
        let mut c = SeqCounter::new(0);
        assert_eq!(c.next(1000), 1000);
        // Same millisecond: strictly increasing.
        assert_eq!(c.next(1000), 1001);
        // The clock goes back: no value is reused.
        assert_eq!(c.next(5), 1002);
        assert_eq!(c.next(5000), 5000);
        // A persisted floor after a restart with an earlier clock.
        let mut c = SeqCounter::new(9000);
        assert_eq!(c.next(10), 9001);
        assert_eq!(c.last(), 9001);
        assert_eq!(SeqCounter::new(u64::MAX).next(1), u64::MAX);
    }

    #[test]
    fn keys_need_32_bytes_and_hide_the_secret() {
        assert_eq!(
            DisplayKey::from_secret(&[1; 31]).unwrap_err(),
            KeyError::BadLength
        );
        assert_eq!(
            DisplayKey::from_secret(&[1; 33]).unwrap_err(),
            KeyError::BadLength
        );
        let k = key(9);
        let shown = format!("{k:?}");
        assert!(shown.contains(&k.id()) && !shown.contains("09090909"));
        let a = DisplayKey::generate_secret();
        let b = DisplayKey::generate_secret();
        assert_ne!(a, b);
        assert_ne!(a, [0; 32]);
        assert_eq!(
            DisplayKey::from_secret(&a).unwrap().id(),
            DisplayKey::from_secret(&a).unwrap().id()
        );
    }
}
