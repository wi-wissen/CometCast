// SPDX-License-Identifier: GPL-3.0-or-later
//! Device authentication of the Cast receiver (namespace `urn:x-cast:com.google.cast.tp.deviceauth`).
//!
//! # What the protocol does (read in Open Screen, commit `8b62b21479f1adb7663f182d97962b960c5c64ef`)
//!
//! The sender sends a `DeviceAuthMessage` with an `AuthChallenge` (random 16 byte nonce, hash
//! SHA-256). The receiver answers with an `AuthResponse`: a signature over
//! `nonce || DER(TLS certificate of this connection)`, made with the key of its *device
//! certificate*, plus that certificate and its intermediates. A sender then has to check
//!
//! 1. the TLS certificate is currently valid and lives at most 4 more days,
//! 2. the nonce is echoed,
//! 3. the device certificate chain leads to a **trusted Cast root**,
//! 4. (optionally) no certificate is revoked according to the CRL in the response,
//! 5. the signature over `nonce || TLS certificate` verifies with the device certificate's key.
//!
//! (`cast/sender/channel/cast_auth_util.cc`, `cast/common/certificate/cast_cert_validator.cc`;
//! BSD-3-Clause, Copyright 2019 The Chromium Authors; see THIRD_PARTY_NOTICES.md.)
//!
//! # What this module does
//!
//! * Steps 1, 2 and 5 are implemented.
//! * Step 3 needs the Cast trust anchors. They are public data in the Open Screen tree
//!   (`cast/common/certificate/cast_root_ca_cert_der-inc.h`), but **this repository does not
//!   download or embed them** (project rule: no key or certificate material taken from a
//!   product). The caller must supply anchors ([`TrustAnchors`]); with none, the default
//!   policy [`DeviceAuth::Verify`] fails closed with an explanatory error.
//! * UNCERTAIN: the chain check here only verifies the issuer signatures and validity dates.
//!   Open Screen's validator additionally checks key usage, the Cast policy OIDs and the
//!   device policy of the certificate, and evaluates the CRL; none of that is implemented.
//! * [`DeviceAuth::InsecureSkip`] skips all of it. It exists so that a beta tester can try
//!   a real device without the anchors. Without device auth, anyone on the network who
//!   answers on port 8009 is accepted as the receiver (and gets the screen picture).

use ring::signature;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::pem::PemObject;
use x509_parser::prelude::*;

use crate::error::CastError;
use crate::proto::{
    AuthChallenge, CastMessage, DeviceAuthMessage, HashAlgorithm, PayloadType, SignatureAlgorithm,
};

/// Namespace of the device-auth exchange.
pub const AUTH_NAMESPACE: &str = "urn:x-cast:com.google.cast.tp.deviceauth";

/// Trusted root certificates (DER) for the device certificate chain.
#[derive(Debug, Clone, Default)]
pub struct TrustAnchors(pub Vec<Vec<u8>>);

impl TrustAnchors {
    /// Read all `CERTIFICATE` blocks of a PEM text.
    pub fn from_pem(pem: &[u8]) -> Result<Self, CastError> {
        let mut out = Vec::new();
        for item in CertificateDer::pem_slice_iter(pem) {
            let der = item.map_err(|e| CastError::DeviceAuth(format!("bad PEM: {e}")))?;
            out.push(der.as_ref().to_vec());
        }
        if out.is_empty() {
            return Err(CastError::DeviceAuth(
                "no certificate found in the PEM data".into(),
            ));
        }
        Ok(Self(out))
    }
}

/// What to do about device authentication.
#[derive(Debug, Clone)]
pub enum DeviceAuth {
    /// Check the receiver's device authentication against these anchors (default policy).
    Verify(TrustAnchors),
    /// Do NOT trust the device: the challenge is still sent and logged, but a failed or
    /// missing proof is ignored and the chain is not checked. Must be enabled explicitly
    /// (CLI: `--insecure-skip-device-auth`).
    InsecureSkip,
}

/// What the check found out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthOutcome {
    /// True if the chain and the signature were verified.
    pub verified: bool,
    /// Subject of the device certificate, when one could be parsed.
    pub device_subject: Option<String>,
}

/// A random 16 byte nonce (Open Screen: `kNonceSizeInBytes = 16`).
pub fn new_nonce() -> [u8; 16] {
    rand::random()
}

/// The challenge message (hash algorithm SHA-256; the signature algorithm is left at its
/// default, RSASSA-PKCS1-v1_5, as Open Screen does).
pub fn challenge_message(nonce: &[u8]) -> CastMessage {
    use prost::Message;
    let body = DeviceAuthMessage {
        challenge: Some(AuthChallenge {
            signature_algorithm: None,
            sender_nonce: Some(nonce.to_vec()),
            hash_algorithm: Some(HashAlgorithm::Sha256 as i32),
        }),
        response: None,
        error: None,
    };
    CastMessage::binary(
        "sender-0",
        "receiver-0",
        AUTH_NAMESPACE,
        body.encode_to_vec(),
    )
}

/// Check the receiver's reply to [`challenge_message`].
///
/// `peer_cert_der` is the certificate the receiver presented in the TLS handshake.
/// `now_unix` is the current time in seconds since 1970.
pub fn verify_response(
    reply: &CastMessage,
    nonce: &[u8],
    peer_cert_der: &[u8],
    anchors: &TrustAnchors,
    now_unix: i64,
) -> Result<AuthOutcome, CastError> {
    use prost::Message;
    let fail = |m: &str| CastError::DeviceAuth(m.to_owned());
    if reply.namespace != AUTH_NAMESPACE {
        return Err(fail("reply is not in the deviceauth namespace"));
    }
    if reply.payload_type != PayloadType::Binary as i32 {
        return Err(fail("reply has the wrong payload type"));
    }
    let payload = reply
        .payload_binary
        .as_deref()
        .ok_or_else(|| fail("reply has no payload"))?;
    let msg = DeviceAuthMessage::decode(payload)
        .map_err(|e| fail(&format!("cannot parse reply: {e}")))?;
    if let Some(e) = msg.error {
        return Err(CastError::DeviceAuth(format!(
            "receiver reported auth error type {}",
            e.error_type
        )));
    }
    let resp = msg.response.ok_or_else(|| fail("reply has no response"))?;

    // 1. The TLS certificate is used as a signed blob: its validity period is the lifetime of
    //    the signature (Open Screen, `VerifyTLSCertificateValidity`).
    let (_, tls_cert) = X509Certificate::from_der(peer_cert_der)
        .map_err(|_| fail("cannot parse the TLS certificate"))?;
    let not_before = tls_cert.validity().not_before.timestamp();
    let not_after = tls_cert.validity().not_after.timestamp();
    if now_unix < not_before {
        return Err(fail("TLS certificate is not valid yet"));
    }
    if not_after < now_unix {
        return Err(fail("TLS certificate has expired"));
    }
    if not_after > now_unix + 4 * 24 * 3600 {
        return Err(fail(
            "TLS certificate lifetime is too long (more than 4 days)",
        ));
    }

    // 2. Nonce.
    if resp.sender_nonce.as_deref() != Some(nonce) {
        return Err(fail("sender nonce mismatch"));
    }

    // 3. Chain.
    let mut chain_der: Vec<&[u8]> = vec![&resp.client_auth_certificate];
    chain_der.extend(resp.intermediate_certificate.iter().map(Vec::as_slice));
    let chain: Vec<X509Certificate<'_>> = chain_der
        .iter()
        .map(|d| X509Certificate::from_der(d).map(|(_, c)| c))
        .collect::<Result<_, _>>()
        .map_err(|_| fail("cannot parse the device certificate chain"))?;
    if anchors.0.is_empty() {
        return Err(fail(
            "no trust anchors were supplied, so the device certificate chain cannot be verified \
             (this program does not embed the Cast roots; see docs/cast.md). Supply them or use \
             --insecure-skip-device-auth",
        ));
    }
    let anchor_certs: Vec<X509Certificate<'_>> = anchors
        .0
        .iter()
        .filter_map(|d| X509Certificate::from_der(d).ok().map(|(_, c)| c))
        .collect();
    verify_chain(&chain, &anchor_certs, now_unix)?;

    // 5. Signature over nonce || TLS certificate.
    let mut signed = nonce.to_vec();
    signed.extend_from_slice(peer_cert_der);
    let hash = match resp.hash_algorithm.map_or(Some(HashAlgorithm::Sha1), |h| {
        HashAlgorithm::try_from(h).ok()
    }) {
        Some(h) => h,
        None => return Err(fail("unknown hash algorithm")),
    };
    let sig_alg = match resp
        .signature_algorithm
        .map_or(Some(SignatureAlgorithm::RsassaPkcs1v15), |a| {
            SignatureAlgorithm::try_from(a).ok()
        }) {
        Some(SignatureAlgorithm::RsassaPkcs1v15 | SignatureAlgorithm::Unspecified) => false,
        Some(SignatureAlgorithm::RsassaPss) => true,
        None => return Err(fail("unknown signature algorithm")),
    };
    let alg: &'static dyn signature::VerificationAlgorithm = match (sig_alg, hash) {
        (false, HashAlgorithm::Sha256) => &signature::RSA_PKCS1_2048_8192_SHA256,
        // Open Screen accepts SHA-1 responses here (`enforce_sha256_checking` is false).
        (false, HashAlgorithm::Sha1) => &signature::RSA_PKCS1_2048_8192_SHA1_FOR_LEGACY_USE_ONLY,
        // UNCERTAIN: PSS parameters (salt length) of real devices were not checked.
        (true, HashAlgorithm::Sha256) => &signature::RSA_PSS_2048_8192_SHA256,
        (true, HashAlgorithm::Sha1) => return Err(fail("RSASSA-PSS with SHA-1 is not supported")),
    };
    let key = signature::UnparsedPublicKey::new(
        alg,
        chain[0].public_key().subject_public_key.data.as_ref(),
    );
    key.verify(&signed, &resp.signature)
        .map_err(|_| fail("the signature over nonce and TLS certificate does not verify"))?;

    Ok(AuthOutcome {
        verified: true,
        device_subject: Some(chain[0].subject().to_string()),
    })
}

/// Walk from the leaf up: each certificate must be signed by a later chain member or by an
/// anchor, and all must be valid now.
fn verify_chain(
    chain: &[X509Certificate<'_>],
    anchors: &[X509Certificate<'_>],
    now_unix: i64,
) -> Result<(), CastError> {
    let fail = |m: String| CastError::DeviceAuth(m);
    let at = ASN1Time::from_timestamp(now_unix).map_err(|_| fail("bad current time".into()))?;
    let mut idx = 0;
    loop {
        let cert = &chain[idx];
        if !cert.validity().is_valid_at(at) {
            return Err(fail(format!(
                "certificate {} is not valid now",
                cert.subject()
            )));
        }
        if let Some(a) = anchors.iter().find(|a| {
            a.subject() == cert.issuer() && cert.verify_signature(Some(a.public_key())).is_ok()
        }) {
            if !a.validity().is_valid_at(at) {
                return Err(fail("the trust anchor is not valid now".into()));
            }
            return Ok(());
        }
        let next = chain.iter().enumerate().skip(idx + 1).find(|(_, c)| {
            c.subject() == cert.issuer() && cert.verify_signature(Some(c.public_key())).is_ok()
        });
        match next {
            Some((i, _)) => idx = i,
            None => {
                return Err(fail(format!(
                    "certificate {} does not chain to a trust anchor",
                    cert.subject()
                )));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_asks_for_sha256_and_carries_the_nonce() {
        use prost::Message;
        let m = challenge_message(&[7; 16]);
        assert_eq!(m.namespace, AUTH_NAMESPACE);
        assert_eq!(m.payload_type, PayloadType::Binary as i32);
        let d = DeviceAuthMessage::decode(m.payload_binary.unwrap().as_slice()).unwrap();
        let c = d.challenge.unwrap();
        assert_eq!(c.sender_nonce.unwrap(), vec![7; 16]);
        assert_eq!(c.hash_algorithm, Some(HashAlgorithm::Sha256 as i32));
    }

    #[test]
    fn empty_anchor_pem_is_an_error() {
        assert!(TrustAnchors::from_pem(b"nothing here").is_err());
    }
}
