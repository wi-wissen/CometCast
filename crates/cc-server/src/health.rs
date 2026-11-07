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

/// The addresses to ask, in order, on the configured port. An IPv4 unspecified/loopback
/// listener: 127.0.0.1. `[::]` (maybe dual-stack, maybe IPv6 only, depending on the host's
/// `bindv6only`): 127.0.0.1 first, then `::1`. `[::1]`: `::1`. Any other address: itself.
fn targets(listen: SocketAddr) -> Vec<SocketAddr> {
    let port = listen.port();
    let v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let v6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port);
    match listen.ip() {
        IpAddr::V4(a) if a.is_unspecified() || a.is_loopback() => vec![v4],
        IpAddr::V6(a) if a.is_unspecified() => vec![v4, v6],
        IpAddr::V6(a) if a.is_loopback() => vec![v6],
        _ => vec![listen],
    }
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
    check_addrs(config, &targets(config.listen_addr)).await
}

/// Try each address in turn. Only a failed CONNECT moves on to the next one; once something
/// answers, its verdict (200 or not) is final.
async fn check_addrs(config: &ServerConfig, addrs: &[SocketAddr]) -> Result<(), String> {
    let mut errors = Vec::new();
    for &addr in addrs {
        let tcp = match tokio::time::timeout(
            Duration::from_secs(1),
            tokio::net::TcpStream::connect(addr),
        )
        .await
        {
            Ok(Ok(tcp)) => tcp,
            Ok(Err(e)) => {
                errors.push(format!("cannot connect to {addr}: {e}"));
                continue;
            }
            Err(_) => {
                errors.push(format!("cannot connect to {addr}: timeout"));
                continue;
            }
        };
        return ask(config, addr, tcp).await;
    }
    Err(errors.join("; "))
}

async fn ask(
    config: &ServerConfig,
    addr: SocketAddr,
    tcp: tokio::net::TcpStream,
) -> Result<(), String> {
    let work = async {
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

    fn sa(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn unspecified_listen_address_is_asked_on_loopback() {
        assert_eq!(targets(sa("0.0.0.0:8443")), vec![sa("127.0.0.1:8443")]);
        assert_eq!(targets(sa("127.0.0.1:8443")), vec![sa("127.0.0.1:8443")]);
        // [::]: IPv4 loopback first, then ::1.
        assert_eq!(
            targets(sa("[::]:8443")),
            vec![sa("127.0.0.1:8443"), sa("[::1]:8443")]
        );
        assert_eq!(targets(sa("[::1]:8443")), vec![sa("[::1]:8443")]);
        assert_eq!(targets(sa("192.0.2.1:5")), vec![sa("192.0.2.1:5")]);
        assert_eq!(targets(sa("[2001:db8::1]:5")), vec![sa("[2001:db8::1]:5")]);
    }

    /// A one-shot fake server that answers every connection with `answer`.
    async fn fake(answer: &'static str) -> SocketAddr {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                let mut buf = [0u8; 512];
                let _ = s.read(&mut buf).await;
                let _ = s.write_all(answer.as_bytes()).await;
            }
        });
        addr
    }

    #[tokio::test]
    async fn a_non_200_answer_is_unhealthy() {
        for answer in [
            "HTTP/1.1 503 Service Unavailable\r\n\r\n",
            "HTTP/1.1 404 Not Found\r\n\r\n",
            "garbage",
            "",
        ] {
            let addr = fake(answer).await;
            let cfg = ServerConfig {
                listen_addr: addr,
                ..ServerConfig::default()
            };
            let r = healthcheck(&cfg).await;
            assert!(r.is_err(), "answer {answer:?} counted as healthy: {r:?}");
        }
        let ok = fake("HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok").await;
        let cfg = ServerConfig {
            listen_addr: ok,
            ..ServerConfig::default()
        };
        assert_eq!(healthcheck(&cfg).await, Ok(()));
    }

    #[tokio::test]
    async fn a_refused_connect_moves_on_but_a_bad_answer_is_final() {
        // (No IPv6 needed: two IPv4 loopback ports stand in for 127.0.0.1 and ::1.)
        let closed = {
            let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            l.local_addr().unwrap()
        };
        let good = fake("HTTP/1.1 200 OK\r\n\r\n").await;
        let bad = fake("HTTP/1.1 500 Oops\r\n\r\n").await;
        let cfg = ServerConfig::default();
        assert_eq!(check_addrs(&cfg, &[closed, good]).await, Ok(()));
        // The first address answered with an error: no fallback to the healthy one.
        assert!(check_addrs(&cfg, &[bad, good]).await.is_err());
        let e = check_addrs(&cfg, &[closed, closed]).await.unwrap_err();
        assert!(e.contains("cannot connect"), "{e}");
    }

    #[tokio::test]
    async fn unspecified_ipv6_listener_falls_back_to_the_next_loopback_address() {
        // The server listens on ::1 only (as an IPv6-only `[::]` listener would be reachable
        // there); the configured listen address is `[::]:port`, so 127.0.0.1 is tried first,
        // refuses, and ::1 answers. Skipped where the host has no IPv6 loopback.
        let Ok(l) = tokio::net::TcpListener::bind("[::1]:0").await else {
            eprintln!("skipped: no IPv6 loopback");
            return;
        };
        let port = l.local_addr().unwrap().port();
        // Make sure nothing answers on 127.0.0.1:port (it is a different socket family).
        let cfg = ServerConfig {
            listen_addr: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port),
            ..ServerConfig::default()
        };
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                let mut buf = [0u8; 512];
                let _ = s.read(&mut buf).await;
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await;
            }
        });
        assert_eq!(healthcheck(&cfg).await, Ok(()));
        // Nothing listening at all: every attempt is reported.
        let down = ServerConfig {
            listen_addr: sa("[::]:1"),
            ..ServerConfig::default()
        };
        let e = healthcheck(&down).await.unwrap_err();
        assert!(e.contains("127.0.0.1:1") && e.contains("[::1]:1"), "{e}");
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
