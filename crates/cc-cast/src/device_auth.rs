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
//! * The chain check (step 3) verifies, for every link: the issuer signature, the validity
//!   dates and the roles. The device certificate (the leaf) must NOT be a CA and must carry a
//!   KeyUsage allowing `digitalSignature` (UNCERTAIN: real device leaves are assumed to carry
//!   that extension; a leaf without it is rejected). Every issuer must be a CA
//!   (`BasicConstraints` CA=true) and `pathLenConstraint` is honoured; intermediates must
//!   carry a KeyUsage with `keyCertSign` (a trust anchor without any KeyUsage is tolerated,
//!   one with a KeyUsage must have it). A non-CA certificate can therefore never sign another
//!   certificate of the chain.
//!   NOT checked (UNCERTAIN, Open Screen's validator does these): the Cast policy OIDs and the
//!   device policy of the certificate, CRL evaluation (the CRL in the response is ignored),
//!   name constraints, and unknown critical extensions.
//! * SHA-1: a response that asks for (or defaults to) SHA-1 is accepted, as Open Screen does
//!   (`enforce_sha256_checking` is false there). That happens only after the chain has been
//!   validated up to an anchor; the sender itself always asks for SHA-256.
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
    /// Do NOT trust the device: no challenge is sent (the device-auth exchange is skipped
    /// entirely) and nothing about the device is checked. Must be enabled explicitly
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

/// Role checks for a certificate used as an issuer. `cas_below` is the number of intermediate
/// CA certificates between it and the leaf (what `pathLenConstraint` limits). `strict_usage`:
/// a KeyUsage extension with `keyCertSign` is required (intermediates); for trust anchors an
/// absent KeyUsage is tolerated, a present one must still allow it.
fn check_issuer_role(
    cert: &X509Certificate<'_>,
    cas_below: usize,
    strict_usage: bool,
) -> Result<(), String> {
    let name = cert.subject();
    match cert.basic_constraints() {
        Ok(Some(bc)) if bc.value.ca => {
            if let Some(max) = bc.value.path_len_constraint
                && cas_below as u64 > u64::from(max)
            {
                return Err(format!("certificate {name} violates its pathLenConstraint"));
            }
        }
        _ => {
            return Err(format!(
                "certificate {name} is not a CA (BasicConstraints CA=true is required to sign)"
            ));
        }
    }
    match cert.key_usage() {
        Ok(Some(ku)) if ku.value.key_cert_sign() => Ok(()),
        Ok(None) if !strict_usage => Ok(()),
        _ => Err(format!(
            "certificate {name} may not sign certificates (KeyUsage keyCertSign is missing)"
        )),
    }
}

/// The device certificate must be an end-entity certificate that can sign data.
fn check_leaf_role(cert: &X509Certificate<'_>) -> Result<(), String> {
    let name = cert.subject();
    if let Ok(Some(bc)) = cert.basic_constraints()
        && bc.value.ca
    {
        return Err(format!("device certificate {name} must not be a CA"));
    }
    match cert.key_usage() {
        Ok(Some(ku)) if ku.value.digital_signature() => Ok(()),
        _ => Err(format!(
            "device certificate {name} does not allow digitalSignature"
        )),
    }
}

/// Walk from the leaf up: each certificate must be signed by a later chain member or by an
/// anchor, all must be valid now, the leaf must be an end-entity certificate and every issuer
/// must be a CA (see [`check_issuer_role`]). `chain[0]` is the device certificate.
fn verify_chain(
    chain: &[X509Certificate<'_>],
    anchors: &[X509Certificate<'_>],
    now_unix: i64,
) -> Result<(), CastError> {
    let fail = |m: String| CastError::DeviceAuth(m);
    let at = ASN1Time::from_timestamp(now_unix).map_err(|_| fail("bad current time".into()))?;
    check_leaf_role(&chain[0]).map_err(fail)?;
    let mut idx = 0;
    loop {
        let cert = &chain[idx];
        if !cert.validity().is_valid_at(at) {
            return Err(fail(format!(
                "certificate {} is not valid now",
                cert.subject()
            )));
        }
        // Intermediates between the leaf and the issuer we are looking for.
        let cas_below = idx;
        let mut role_error = None;
        let anchor = anchors.iter().find(|a| {
            a.subject() == cert.issuer()
                && cert.verify_signature(Some(a.public_key())).is_ok()
                && match check_issuer_role(a, cas_below, false) {
                    Ok(()) => true,
                    Err(e) => {
                        role_error = Some(e);
                        false
                    }
                }
        });
        if let Some(a) = anchor {
            if !a.validity().is_valid_at(at) {
                return Err(fail("the trust anchor is not valid now".into()));
            }
            return Ok(());
        }
        let next = chain.iter().enumerate().skip(idx + 1).find(|(_, c)| {
            c.subject() == cert.issuer()
                && cert.verify_signature(Some(c.public_key())).is_ok()
                && match check_issuer_role(c, cas_below, true) {
                    Ok(()) => true,
                    Err(e) => {
                        role_error = Some(e);
                        false
                    }
                }
        });
        match next {
            Some((i, _)) => idx = i,
            None => {
                return Err(fail(role_error.unwrap_or_else(|| {
                    format!(
                        "certificate {} does not chain to a trust anchor",
                        cert.subject()
                    )
                })));
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

    // ---- chain role checks (certificates generated here; nothing from a product) ----

    use ::time::{Duration as TD, OffsetDateTime};
    use rcgen::{
        BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair,
        KeyUsagePurpose,
    };

    struct Node {
        params: CertificateParams,
        key: KeyPair,
        der: Vec<u8>,
    }

    /// `ca`: `None` = end entity. `signer`: `None` = self-signed.
    fn node(
        cn: &str,
        ca: Option<BasicConstraints>,
        usages: Vec<KeyUsagePurpose>,
        signer: Option<&Node>,
    ) -> Node {
        let now = OffsetDateTime::now_utc();
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, cn);
        params.distinguished_name = dn;
        params.is_ca = ca.map_or(IsCa::ExplicitNoCa, IsCa::Ca);
        params.key_usages = usages;
        params.not_before = now - TD::days(1);
        params.not_after = now + TD::days(30);
        let key = KeyPair::generate().unwrap();
        let der = match signer {
            None => params.self_signed(&key).unwrap().der().to_vec(),
            Some(s) => {
                let issuer = Issuer::new(s.params.clone(), &s.key);
                params.signed_by(&key, &issuer).unwrap().der().to_vec()
            }
        };
        Node { params, key, der }
    }

    fn ca(cn: &str, signer: Option<&Node>, bc: BasicConstraints) -> Node {
        node(cn, Some(bc), vec![KeyUsagePurpose::KeyCertSign], signer)
    }

    fn leaf(cn: &str, signer: &Node) -> Node {
        node(
            cn,
            None,
            vec![KeyUsagePurpose::DigitalSignature],
            Some(signer),
        )
    }

    /// Run `verify_chain` on `[chain...]` against `[anchor]`.
    fn run(chain: &[&Node], anchor: &Node) -> Result<(), String> {
        fn parse(n: &Node) -> X509Certificate<'_> {
            X509Certificate::from_der(&n.der).unwrap().1
        }
        let chain: Vec<_> = chain.iter().map(|n| parse(n)).collect();
        let anchors = vec![parse(anchor)];
        verify_chain(&chain, &anchors, OffsetDateTime::now_utc().unix_timestamp())
            .map_err(|e| e.to_string())
    }

    #[test]
    fn well_formed_chains_verify() {
        let root = ca("root", None, BasicConstraints::Unconstrained);
        let ica = ca("ica", Some(&root), BasicConstraints::Constrained(0));
        let dev = leaf("device", &ica);
        run(&[&dev, &ica], &root).unwrap();
        // Directly under the root as well.
        let dev2 = leaf("device2", &root);
        run(&[&dev2], &root).unwrap();
    }

    #[test]
    fn a_non_ca_device_leaf_cannot_act_as_issuer() {
        // The reviewer's forged chain: [forged, legit-leaf] where the legit leaf is not a CA.
        let root = ca("root", None, BasicConstraints::Unconstrained);
        let legit = leaf("legit device (not a CA)", &root);
        let forged = leaf("forged device", &legit);
        let e = run(&[&forged, &legit], &root).unwrap_err();
        assert!(e.contains("not a CA"), "{e}");
    }

    #[test]
    fn issuer_needs_ca_flag_and_key_cert_sign() {
        let root = ca("root", None, BasicConstraints::Unconstrained);
        // CA=true but no keyCertSign.
        let no_ku = node(
            "ica-no-ku",
            Some(BasicConstraints::Unconstrained),
            vec![KeyUsagePurpose::DigitalSignature],
            Some(&root),
        );
        let dev = leaf("device", &no_ku);
        let e = run(&[&dev, &no_ku], &root).unwrap_err();
        assert!(e.contains("keyCertSign"), "{e}");
        // keyCertSign but no BasicConstraints CA flag.
        let not_ca = node(
            "ica-not-ca",
            None,
            vec![
                KeyUsagePurpose::KeyCertSign,
                KeyUsagePurpose::DigitalSignature,
            ],
            Some(&root),
        );
        let dev = leaf("device", &not_ca);
        assert!(run(&[&dev, &not_ca], &root).is_err());
    }

    #[test]
    fn path_length_is_enforced() {
        let root = ca("root", None, BasicConstraints::Unconstrained);
        let ica1 = ca("ica1", Some(&root), BasicConstraints::Constrained(0));
        let ica2 = ca("ica2", Some(&ica1), BasicConstraints::Constrained(0));
        let dev = leaf("device", &ica2);
        let e = run(&[&dev, &ica2, &ica1], &root).unwrap_err();
        assert!(e.contains("pathLen"), "{e}");
        // The same shape is fine when ica1 allows one CA below.
        let ica1 = ca("ica1", Some(&root), BasicConstraints::Constrained(1));
        let ica2 = ca("ica2", Some(&ica1), BasicConstraints::Constrained(0));
        let dev = leaf("device", &ica2);
        run(&[&dev, &ica2, &ica1], &root).unwrap();
    }

    #[test]
    fn the_leaf_must_be_an_end_entity_that_can_sign_data() {
        let root = ca("root", None, BasicConstraints::Unconstrained);
        // A CA used as the device certificate.
        let as_leaf = ca("ca-as-leaf", Some(&root), BasicConstraints::Unconstrained);
        let e = run(&[&as_leaf], &root).unwrap_err();
        assert!(e.contains("must not be a CA"), "{e}");
        // A leaf without digitalSignature.
        let no_sig = node(
            "device",
            None,
            vec![KeyUsagePurpose::KeyEncipherment],
            Some(&root),
        );
        let e = run(&[&no_sig], &root).unwrap_err();
        assert!(e.contains("digitalSignature"), "{e}");
    }

    #[test]
    fn a_non_ca_anchor_is_refused() {
        let fake_root = node(
            "not-a-ca-root",
            None,
            vec![KeyUsagePurpose::DigitalSignature],
            None,
        );
        let dev = leaf("device", &fake_root);
        assert!(run(&[&dev], &fake_root).is_err());
    }

    #[test]
    fn empty_anchor_pem_is_an_error() {
        assert!(TrustAnchors::from_pem(b"nothing here").is_err());
    }
}
