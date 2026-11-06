// SPDX-License-Identifier: GPL-3.0-or-later
//! TLS to a Cast receiver, which presents a self-signed certificate.
//!
//! The usual certificate-chain check is replaced by a verifier that accepts any certificate
//! and remembers it. This is only acceptable together with the device-auth step
//! ([`crate::device_auth`]): the receiver then has to sign `nonce || this certificate` with
//! its device key. The TLS handshake signature itself is still verified against the key in
//! the presented certificate (so the peer proves it owns that certificate's key).
//!
//! With [`crate::device_auth::DeviceAuth::InsecureSkip`] nothing authenticates the peer.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

use crate::error::CastError;

/// Accepts any server certificate, records it, still checks handshake signatures.
#[derive(Debug)]
struct RecordingVerifier {
    algs: WebPkiSupportedAlgorithms,
    seen: Mutex<Option<Vec<u8>>>,
}

impl ServerCertVerifier for RecordingVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if let Ok(mut g) = self.seen.lock() {
            *g = Some(end_entity.as_ref().to_vec());
        }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}

/// A TLS stream plus the DER of the certificate the peer presented.
pub struct CastTls {
    /// The encrypted stream.
    pub stream: TlsStream<TcpStream>,
    /// The peer's end-entity certificate (DER).
    pub peer_cert: Vec<u8>,
}

/// Connect and handshake. The caller must run device auth before trusting the peer.
pub async fn connect(addr: SocketAddr) -> Result<CastTls, CastError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Arc::new(RecordingVerifier {
        algs: provider.signature_verification_algorithms,
        seen: Mutex::new(None),
    });
    let config =
        rustls::ClientConfig::builder_with_provider(Arc::clone(&provider) as Arc<CryptoProvider>)
            .with_safe_default_protocol_versions()
            .map_err(|e| CastError::Protocol(format!("TLS setup: {e}")))?
            .dangerous()
            .with_custom_certificate_verifier(verifier.clone())
            .with_no_client_auth();
    let tcp = TcpStream::connect(addr).await?;
    let _ = tcp.set_nodelay(true);
    let name = ServerName::IpAddress(addr.ip().into());
    let stream = TlsConnector::from(Arc::new(config))
        .connect(name, tcp)
        .await?;
    let peer_cert = verifier
        .seen
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .ok_or_else(|| CastError::Protocol("the receiver presented no certificate".into()))?;
    Ok(CastTls { stream, peer_cert })
}
