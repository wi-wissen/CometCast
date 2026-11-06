// SPDX-License-Identifier: GPL-3.0-or-later
//! SRP-6a as used by AirPlay pairing, written for this project on top of `num-bigint` and
//! the RustCrypto hashes.
//!
//! Two variants exist, both known from pyatv (`protocols/airplay/srp.py`, `auth/hap_srp.py`,
//! MIT) and its helper library srptools (BSD-3-Clause), which were *read*, not copied:
//!
//! * legacy AirPlay PIN pairing: 2048-bit group (RFC 5054), SHA-1, the session key is
//!   `H(S || 00000000) || H(S || 00000001)`, user name is the client id;
//! * HomeKit-style pairing (AirPlay 2): 3072-bit group (RFC 3526 group 15, g = 5), SHA-512,
//!   session key `H(S)`, user name `Pair-Setup`.
//!
//! Formulas (as in srptools): `k = H(N | PAD(g))`, `x = H(s | H(I ":" P))`,
//! `u = H(PAD(A) | PAD(B))`, `S = (B - k g^x)^(a + u x) mod N`,
//! `M1 = H(H(N) xor H(g) | H(I) | s | A | B | K)` with `A`, `B`, `N`, `g`, `S` written as
//! minimal big-endian integers (no leading zeros), `M2 = H(A | M1 | K)`.
//!
//! UNCERTAIN: the exact byte conventions (minimal vs padded integers, raw salt) match what
//! pyatv sends, but were only verified against our own test fake and, for the legacy
//! variant, against UxPlay 1.68 (see docs/airplay-sender.md). No Apple receiver was tested.
//! pyatv hashes the salt as an integer (leading zero bytes dropped); we hash the raw salt.
//! The two differ in about 1 of 256 salts.
//!
//! This code is not constant-time (UNCERTAIN: acceptable for a LAN pairing client; the PIN
//! has only 4 digits anyway and the attacker model is the receiver, not a timing observer).

use std::marker::PhantomData;

use num_bigint::BigUint;
use sha2::Digest;
use zeroize::Zeroizing;

use crate::error::{Result, SenderError};

/// How the session key `K` is derived from the shared secret `S`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    /// `K = H(S)` (HomeKit style).
    Plain,
    /// `K = H(S || 00 00 00 00) || H(S || 00 00 00 01)` (legacy AirPlay).
    Interleaved,
}

/// A prime and a generator.
#[derive(Debug, Clone)]
pub struct Group {
    n: BigUint,
    g: BigUint,
}

const PRIME_2048: &str = "AC6BDB41324A9A9BF166DE5E1389582FAF72B6651987EE07FC3192943DB56050A37329CBB4A099ED8193E0757767A13DD52312AB4B03310DCD7F48A9DA04FD50E8083969EDB767B0CF6095179A163AB3661A05FBD5FAAAE82918A9962F0B93B855F97993EC975EEAA80D740ADBF4FF747359D041D5C33EA71D281E446B14773BCA97B43A23FB801676BD207A436C6481F1D2B9078717461A5B9D32E688F87748544523B524B0D57D5EA77A2775D2ECFA032CFBDBF52FB3786160279004E57AE6AF874E7303CE53299CCC041C7BC308D82A5698F3A8D0C38271AE35F8E9DBFBB694B5C803D89F7AE435DE236D525F54759B65E372FCD68EF20FA7111F9E4AFF73";
const PRIME_3072: &str = "FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD129024E088A67CC74020BBEA63B139B22514A08798E3404DDEF9519B3CD3A431B302B0A6DF25F14374FE1356D6D51C245E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7EDEE386BFB5A899FA5AE9F24117C4B1FE649286651ECE45B3DC2007CB8A163BF0598DA48361C55D39A69163FA8FD24CF5F83655D23DCA3AD961C62F356208552BB9ED529077096966D670C354E4ABC9804F1746C08CA18217C32905E462E36CE3BE39E772C180E86039B2783A2EC07A28FB5C55DF06F4C52C9DE2BCBF6955817183995497CEA956AE515D2261898FA051015728E5A8AAAC42DAD33170D04507A33A85521ABDF1CBA64ECFB850458DBEF0A8AEA71575D060C7DB3970F85A6E1E4C7ABF5AE8CDB0933D71E8C94E04A25619DCEE3D2261AD2EE6BF12FFA06D98A0864D87602733EC86A64521F2B18177B200CBBE117577A615D6C770988C0BAD946E208E24FA074E5AB3143DB5BFCE0FD108E4B82D120A93AD2CAFFFFFFFFFFFFFFFF";

impl Group {
    /// RFC 5054 2048-bit group, g = 2 (legacy AirPlay).
    pub fn rfc5054_2048() -> Self {
        Self::from_hex(PRIME_2048, 2)
    }

    /// RFC 3526 3072-bit group, g = 5 (HomeKit style).
    pub fn rfc3526_3072() -> Self {
        Self::from_hex(PRIME_3072, 5)
    }

    fn from_hex(hex: &str, g: u32) -> Self {
        let clean: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
        // The constants are compiled in; a typo would be caught by the test that checks
        // the bit length and primality.
        let n = BigUint::parse_bytes(clean.as_bytes(), 16).unwrap_or_default();
        Self {
            n,
            g: BigUint::from(g),
        }
    }

    /// Length of the prime in bytes.
    pub fn len(&self) -> usize {
        self.n.to_bytes_be().len()
    }

    /// Never true; present for the `len` convention.
    pub fn is_empty(&self) -> bool {
        false
    }
}

fn hash<D: Digest>(parts: &[&[u8]]) -> Vec<u8> {
    let mut d = D::new();
    for p in parts {
        d.update(p);
    }
    let out = d.finalize();
    AsRef::<[u8]>::as_ref(&out).to_vec()
}

/// Minimal big-endian bytes (zero gives one zero byte, like srptools).
fn int_bytes(v: &BigUint) -> Vec<u8> {
    v.to_bytes_be()
}

fn padded(v: &BigUint, len: usize) -> Vec<u8> {
    let b = v.to_bytes_be();
    let mut out = vec![0u8; len.saturating_sub(b.len())];
    out.extend_from_slice(&b);
    out
}

/// Compare without stopping at the first difference.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

struct Common<D> {
    group: Group,
    kind: KeyKind,
    _d: PhantomData<D>,
}

impl<D: Digest> Common<D> {
    fn k(&self) -> BigUint {
        let n = int_bytes(&self.group.n);
        let g = padded(&self.group.g, self.group.len());
        BigUint::from_bytes_be(&hash::<D>(&[&n, &g]))
    }

    fn x(&self, user: &[u8], password: &[u8], salt: &[u8]) -> BigUint {
        let inner = hash::<D>(&[user, b":", password]);
        BigUint::from_bytes_be(&hash::<D>(&[salt, &inner]))
    }

    fn u(&self, a: &BigUint, b: &BigUint) -> BigUint {
        let len = self.group.len();
        BigUint::from_bytes_be(&hash::<D>(&[&padded(a, len), &padded(b, len)]))
    }

    fn session_key(&self, s: &BigUint) -> Vec<u8> {
        let sb = int_bytes(s);
        match self.kind {
            KeyKind::Plain => hash::<D>(&[&sb]),
            KeyKind::Interleaved => {
                let mut k = hash::<D>(&[&sb, &[0, 0, 0, 0]]);
                k.extend(hash::<D>(&[&sb, &[0, 0, 0, 1]]));
                k
            }
        }
    }

    fn m1(&self, user: &[u8], salt: &[u8], a: &BigUint, b: &BigUint, key: &[u8]) -> Vec<u8> {
        let hn = hash::<D>(&[&int_bytes(&self.group.n)]);
        let hg = hash::<D>(&[&int_bytes(&self.group.g)]);
        let xor: Vec<u8> = hn.iter().zip(&hg).map(|(x, y)| x ^ y).collect();
        let hu = hash::<D>(&[user]);
        hash::<D>(&[&xor, &hu, salt, &int_bytes(a), &int_bytes(b), key])
    }

    fn m2(&self, a: &BigUint, m1: &[u8], key: &[u8]) -> Vec<u8> {
        hash::<D>(&[&int_bytes(a), m1, key])
    }
}

/// What the client sends in step 2 and keeps.
#[derive(Clone)]
pub struct ClientResult {
    /// Client proof `M1` to send to the receiver.
    pub proof: Vec<u8>,
    /// Session key `K` (secret, wiped when dropped).
    pub session_key: Zeroizing<Vec<u8>>,
}

impl std::fmt::Debug for ClientResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientResult")
            .field("proof", &self.proof)
            .field("session_key", &"<hidden>")
            .finish()
    }
}

/// Client side of SRP-6a.
pub struct Client<D: Digest> {
    c: Common<D>,
    user: Vec<u8>,
    password: Zeroizing<Vec<u8>>,
    a: BigUint,
    a_pub: BigUint,
    state: Option<(Vec<u8>, BigUint, Zeroizing<Vec<u8>>)>,
}

impl<D: Digest> Client<D> {
    /// New client. `private` is the secret exponent `a` (at least 32 random bytes).
    pub fn new(
        group: Group,
        kind: KeyKind,
        user: &[u8],
        password: &[u8],
        private: &[u8],
    ) -> Result<Self> {
        if private.len() < 32 {
            return Err(SenderError::Input("SRP private value too short".into()));
        }
        let a = BigUint::from_bytes_be(private);
        let a_pub = group.g.modpow(&a, &group.n);
        Ok(Self {
            c: Common {
                group,
                kind,
                _d: PhantomData,
            },
            user: user.to_vec(),
            password: Zeroizing::new(password.to_vec()),
            a,
            a_pub,
            state: None,
        })
    }

    /// Public value `A`, left-padded to the prime length.
    pub fn public_key(&self) -> Vec<u8> {
        padded(&self.a_pub, self.c.group.len())
    }

    /// Step 2: take the receiver's salt and public value `B`; returns proof and session key.
    pub fn process(&mut self, salt: &[u8], b_pub: &[u8]) -> Result<ClientResult> {
        if salt.is_empty() || salt.len() > 64 {
            return Err(SenderError::Protocol(
                "SRP salt has a strange length".into(),
            ));
        }
        if b_pub.is_empty() || b_pub.len() > self.c.group.len() {
            return Err(SenderError::Protocol(
                "SRP public value has a strange length".into(),
            ));
        }
        let n = &self.c.group.n;
        let b = BigUint::from_bytes_be(b_pub);
        // 0 < B < N, and not 1 or N-1 (powers of those stay in {1, N-1}).
        let one = BigUint::from(1u8);
        if b >= *n || b <= one || b == n - &one {
            return Err(SenderError::Auth("SRP public value B is invalid".into()));
        }
        let u = self.c.u(&self.a_pub, &b);
        if u == BigUint::default() {
            return Err(SenderError::Auth("SRP scrambling value is zero".into()));
        }
        let x = self.c.x(&self.user, &self.password, salt);
        let k = self.c.k();
        let gx = self.c.group.g.modpow(&x, n);
        // (B - k g^x) mod N, kept non-negative.
        let kgx = (&k * &gx) % n;
        let base = ((&b % n) + n - kgx) % n;
        let exp = &self.a + &u * &x;
        let s = base.modpow(&exp, n);
        let key = self.c.session_key(&s);
        let proof = self.c.m1(&self.user, salt, &self.a_pub, &b, &key);
        let key = Zeroizing::new(key);
        self.state = Some((proof.clone(), b, key.clone()));
        Ok(ClientResult {
            proof,
            session_key: key,
        })
    }

    /// Check the receiver's proof `M2`. Only valid after [`Client::process`].
    pub fn verify_server_proof(&self, m2: &[u8]) -> bool {
        match &self.state {
            Some((m1, _, key)) => ct_eq(&self.c.m2(&self.a_pub, m1, key), m2),
            None => false,
        }
    }
}

/// Server side, only needed by the test fake.
#[cfg(test)]
pub(crate) struct Server<D: Digest> {
    c: Common<D>,
    user: Vec<u8>,
    salt: Vec<u8>,
    v: BigUint,
    b: BigUint,
    b_pub: BigUint,
}

#[cfg(test)]
impl<D: Digest> Server<D> {
    pub(crate) fn new(
        group: Group,
        kind: KeyKind,
        user: &[u8],
        password: &[u8],
        salt: &[u8],
        private: &[u8],
    ) -> Self {
        let c = Common::<D> {
            group,
            kind,
            _d: PhantomData,
        };
        let x = c.x(user, password, salt);
        let v = c.group.g.modpow(&x, &c.group.n);
        let b = BigUint::from_bytes_be(private);
        let k = c.k();
        let b_pub = ((&k * &v) + c.group.g.modpow(&b, &c.group.n)) % &c.group.n;
        Self {
            c,
            user: user.to_vec(),
            salt: salt.to_vec(),
            v,
            b,
            b_pub,
        }
    }

    pub(crate) fn public_key(&self) -> Vec<u8> {
        padded(&self.b_pub, self.c.group.len())
    }

    /// Returns `(M2, K)` if the client proof is right.
    pub(crate) fn verify(&self, a_pub: &[u8], m1: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
        let n = &self.c.group.n;
        let a = BigUint::from_bytes_be(a_pub);
        if (&a % n) == BigUint::default() {
            return None;
        }
        let u = self.c.u(&a, &self.b_pub);
        let s = (&a * self.v.modpow(&u, n)).modpow(&self.b, n);
        let key = self.c.session_key(&s);
        let expect = self.c.m1(&self.user, &self.salt, &a, &self.b_pub, &key);
        if !ct_eq(&expect, m1) {
            return None;
        }
        Some((self.c.m2(&a, m1, &key), key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha1::Sha1;
    use sha2::Sha512;

    fn is_probable_prime(n: &BigUint) -> bool {
        let one = BigUint::from(1u8);
        let two = BigUint::from(2u8);
        let nm1 = n - &one;
        let mut d = nm1.clone();
        let mut s = 0;
        while (&d % &two) == BigUint::default() {
            d /= &two;
            s += 1;
        }
        'witness: for a in [2u32, 3, 5, 7, 11, 13, 17, 19, 23, 29] {
            let mut x = BigUint::from(a).modpow(&d, n);
            if x == one || x == nm1 {
                continue;
            }
            for _ in 0..s - 1 {
                x = (&x * &x) % n;
                if x == nm1 {
                    continue 'witness;
                }
            }
            return false;
        }
        true
    }

    #[test]
    fn constants_are_safe_primes_of_the_right_size() {
        for (g, bits) in [(Group::rfc5054_2048(), 2048), (Group::rfc3526_3072(), 3072)] {
            assert_eq!(g.n.bits(), bits);
            assert!(is_probable_prime(&g.n));
            assert!(is_probable_prime(&((&g.n - 1u8) / 2u8)));
        }
    }

    fn roundtrip<D: Digest>(group: Group, kind: KeyKind) {
        let salt = [5u8; 16];
        let server = Server::<D>::new(group.clone(), kind, b"user", b"4711", &salt, &[7u8; 32]);
        let mut client = Client::<D>::new(group, kind, b"user", b"4711", &[9u8; 32]).unwrap();
        let res = client.process(&salt, &server.public_key()).unwrap();
        let (m2, key) = server.verify(&client.public_key(), &res.proof).unwrap();
        assert_eq!(key, *res.session_key);
        assert!(client.verify_server_proof(&m2));
        assert!(!client.verify_server_proof(&[0; 20]));
    }

    #[test]
    fn debug_hides_the_session_key() {
        let r = ClientResult {
            proof: vec![1],
            session_key: Zeroizing::new(vec![0xAB, 0xCD]),
        };
        let text = format!("{r:?}");
        assert!(!text.contains("171") && !text.contains("205") && !text.contains("ab"));
        assert!(text.contains("hidden"));
    }

    #[test]
    fn legacy_variant_roundtrip() {
        roundtrip::<Sha1>(Group::rfc5054_2048(), KeyKind::Interleaved);
    }

    #[test]
    fn homekit_variant_roundtrip() {
        roundtrip::<Sha512>(Group::rfc3526_3072(), KeyKind::Plain);
    }

    #[test]
    fn wrong_pin_is_rejected_by_server() {
        let group = Group::rfc5054_2048();
        let salt = [5u8; 16];
        let server = Server::<Sha1>::new(
            group.clone(),
            KeyKind::Interleaved,
            b"u",
            b"1111",
            &salt,
            &[7u8; 32],
        );
        let mut client =
            Client::<Sha1>::new(group, KeyKind::Interleaved, b"u", b"2222", &[9u8; 32]).unwrap();
        let res = client.process(&salt, &server.public_key()).unwrap();
        assert!(server.verify(&client.public_key(), &res.proof).is_none());
    }

    #[test]
    fn hostile_server_values_are_errors_not_panics() {
        let group = Group::rfc5054_2048();
        let mut c =
            Client::<Sha1>::new(group.clone(), KeyKind::Interleaved, b"u", b"1", &[1; 32]).unwrap();
        assert!(c.process(&[1; 16], &[0; 256]).is_err()); // B = 0
        assert!(c.process(&[1; 16], &[]).is_err());
        assert!(c.process(&[1; 16], &vec![1; 1000]).is_err());
        assert!(c.process(&[], &[1; 256]).is_err());
        assert!(c.process(&vec![1; 1000], &[1; 256]).is_err());
        // B = N is zero mod N, and B above N is out of range too
        assert!(c.process(&[1; 16], &group.n.to_bytes_be()).is_err());
        let above = (&group.n + 5u8).to_bytes_be();
        assert!(c.process(&[1; 16], &above).is_err());
        // B = 1 and B = N - 1
        assert!(c.process(&[1; 16], &[1]).is_err());
        assert!(c.process(&[1; 16], &(&group.n - 1u8).to_bytes_be()).is_err());
        // B = N - 2 is an ordinary value
        assert!(c.process(&[1; 16], &(&group.n - 2u8).to_bytes_be()).is_ok());
        assert!(Client::<Sha1>::new(group, KeyKind::Plain, b"u", b"1", &[1; 4]).is_err());
    }
}
