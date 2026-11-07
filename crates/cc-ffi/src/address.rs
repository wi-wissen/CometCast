// SPDX-License-Identifier: GPL-3.0-or-later
//! `probe_server`: is this address a Comet Cast server? (spec 15.7, native-apps.md 21.6)
//!
//! `GET https://{server}/healthz` (docs/server.md), 5 s for everything, the certificate is
//! checked against the system store (no self-signed exception, no extra roots).

use std::time::Duration;

use cc_transport::{ServerAddr, TlsTrust};

use crate::api::ProbeResult;

/// The whole probe (connect, TLS, answer) must be over after this long.
pub(crate) const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Probe a parsed address. `Ok` = a Comet Cast server answered `200 ok`; `NotCometCast` = some
/// server answered, but not like one; `Unreachable` = no answer (no connection, TLS refused,
/// timeout).
pub(crate) async fn probe_addr(
    addr: &ServerAddr,
    trust: &TlsTrust,
    limit: Duration,
) -> ProbeResult {
    let call = cc_transport::net::http_get(addr, "/healthz", trust);
    match tokio::time::timeout(limit, call).await {
        Err(_) => ProbeResult::Unreachable,
        Ok(Ok(resp)) => {
            if resp.status == 200 && resp.body.trim_ascii() == b"ok" {
                ProbeResult::Ok
            } else {
                ProbeResult::NotCometCast
            }
        }
        // A broken HTTP answer is a server, but not ours.
        Ok(Err(cc_transport::TransportError::Protocol(_))) => ProbeResult::NotCometCast,
        Ok(Err(_)) => ProbeResult::Unreachable,
    }
}

/// Probe `server` (`host[:port]`) over HTTPS.
pub(crate) async fn probe(server: &str) -> ProbeResult {
    let t = server.trim();
    let t = t.split_once("://").map_or(t, |(_, r)| r);
    let t = t.trim_end_matches('/');
    let Ok(addr) = ServerAddr::parse(&format!("https://{t}")) else {
        return ProbeResult::Unreachable;
    };
    probe_addr(&addr, &TlsTrust::system(), PROBE_TIMEOUT).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// A plain-HTTP fake server that answers every request with `reply`.
    async fn fake(reply: &'static str) -> ServerAddr {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    let _ = s.read(&mut buf).await;
                    let _ = s.write_all(reply.as_bytes()).await;
                    let _ = s.shutdown().await;
                });
            }
        });
        ServerAddr::parse(&format!("http://127.0.0.1:{port}")).unwrap()
    }

    #[tokio::test]
    async fn healthz_ok_is_a_comet_cast_server() {
        let a = fake("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").await;
        let r = probe_addr(&a, &TlsTrust::system(), Duration::from_secs(5)).await;
        assert_eq!(r, ProbeResult::Ok);
    }

    #[tokio::test]
    async fn a_host_without_healthz_is_not_comet_cast() {
        let a = fake("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await;
        let r = probe_addr(&a, &TlsTrust::system(), Duration::from_secs(5)).await;
        assert_eq!(r, ProbeResult::NotCometCast);
        let a = fake("HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello").await;
        let r = probe_addr(&a, &TlsTrust::system(), Duration::from_secs(5)).await;
        assert_eq!(r, ProbeResult::NotCometCast);
    }

    #[tokio::test]
    async fn nothing_listening_is_unreachable() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        let a = ServerAddr::parse(&format!("http://127.0.0.1:{port}")).unwrap();
        let r = probe_addr(&a, &TlsTrust::system(), Duration::from_secs(5)).await;
        assert_eq!(r, ProbeResult::Unreachable);
    }

    #[tokio::test]
    async fn a_silent_server_runs_into_the_time_limit() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let _keep = tokio::spawn(async move {
            let (_s, _) = l.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(30)).await;
        });
        let a = ServerAddr::parse(&format!("http://127.0.0.1:{port}")).unwrap();
        let r = probe_addr(&a, &TlsTrust::system(), Duration::from_millis(200)).await;
        assert_eq!(r, ProbeResult::Unreachable);
    }

    #[test]
    fn the_limit_is_five_seconds() {
        assert_eq!(PROBE_TIMEOUT, Duration::from_secs(5));
    }
}
