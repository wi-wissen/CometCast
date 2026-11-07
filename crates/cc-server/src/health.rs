// SPDX-License-Identifier: GPL-3.0-or-later
//! `cometcast-server --healthcheck`: asks the server that is configured in the same way
//! (same config file, environment) for `GET /healthz` on the loopback address and checks for
//! `200`. For a container health check: the distroless image has no shell and no curl.
//!
//! With TLS on, the certificate is NOT verified here: the check talks to this very machine
//! (127.0.0.1 / ::1) and only asks whether the server answers; no secret is sent.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::config::ServerConfig;

#[derive(Debug)]
struct AcceptAny(Arc<rustls::crypto::CryptoProvider>);

impl ServerCertVerifier for AcceptAny {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(m, c, d, &self.0.signature_verification_algorithms)
    }
    fn verify_tls13_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(m, c, d, &self.0.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// The loopback address to ask: the configured port on 127.0.0.1 (or ::1 for an IPv6 listener).
fn target(listen: SocketAddr) -> SocketAddr {
    let ip = match listen.ip() {
        IpAddr::V6(a) if a.is_unspecified() || a.is_loopback() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        IpAddr::V6(_) => return listen,
        IpAddr::V4(a) if a.is_unspecified() || a.is_loopback() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V4(_) => return listen,
    };
    SocketAddr::new(ip, listen.port())
}

const REQUEST: &[u8] = b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";

async fn exchange<S: AsyncReadExt + AsyncWriteExt + Unpin>(mut s: S) -> Result<(), String> {
    s.write_all(REQUEST)
        .await
        .map_err(|e| format!("write: {e}"))?;
    let mut buf = Vec::new();
    // Only the status line matters; a TLS close without close_notify is ignored.
    let _ = (&mut s).take(2048).read_to_end(&mut buf).await;
    let text = String::from_utf8_lossy(&buf);
    let status = text.lines().next().unwrap_or("");
    if status.starts_with("HTTP/1.1 200") {
        Ok(())
    } else {
        Err(format!("unexpected answer: {status:?}"))
    }
}

/// Ask the local server for `/healthz`. `Ok(())` only for an HTTP 200 answer.
pub async fn healthcheck(config: &ServerConfig) -> Result<(), String> {
    let addr = target(config.listen_addr);
    let work = async {
        let tcp = tokio::net::TcpStream::connect(addr)
            .await
            .map_err(|e| format!("cannot connect to {addr}: {e}"))?;
        if config.tls.enabled() {
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let tls = rustls::ClientConfig::builder_with_provider(provider.clone())
                .with_safe_default_protocol_versions()
                .map_err(|e| e.to_string())?
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(AcceptAny(provider)))
                .with_no_client_auth();
            let name = ServerName::try_from("localhost").map_err(|e| e.to_string())?;
            let s = tokio_rustls::TlsConnector::from(Arc::new(tls))
                .connect(name, tcp)
                .await
                .map_err(|e| format!("TLS handshake with {addr}: {e}"))?;
            exchange(s).await
        } else {
            exchange(tcp).await
        }
    };
    tokio::time::timeout(Duration::from_secs(4), work)
        .await
        .map_err(|_| format!("no answer from {addr} within 4 seconds"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unspecified_listen_address_is_asked_on_loopback() {
        assert_eq!(
            target("0.0.0.0:8443".parse().unwrap()),
            "127.0.0.1:8443".parse().unwrap()
        );
        assert_eq!(
            target("[::]:1".parse().unwrap()),
            "[::1]:1".parse().unwrap()
        );
        assert_eq!(
            target("192.0.2.1:5".parse().unwrap()),
            "192.0.2.1:5".parse().unwrap()
        );
    }

    async fn serve(config: ServerConfig) -> ServerConfig {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut config = config;
        config.listen_addr = listener.local_addr().unwrap();
        tokio::spawn(crate::run(listener, config.clone(), std::future::pending()));
        config
    }

    #[tokio::test]
    async fn healthcheck_works_over_http_and_https_and_fails_when_down() {
        let plain = serve(ServerConfig::default()).await;
        assert_eq!(healthcheck(&plain).await, Ok(()));

        let mut c = ServerConfig::default();
        c.tls.self_signed = true;
        let tls = serve(c).await;
        assert_eq!(healthcheck(&tls).await, Ok(()));

        // Nothing listens here any more: bind and drop to get a free port.
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let down = ServerConfig {
            listen_addr: l.local_addr().unwrap(),
            ..ServerConfig::default()
        };
        drop(l);
        assert!(healthcheck(&down).await.is_err());
    }
}
