// SPDX-License-Identifier: GPL-3.0-or-later
//! A cap on accepted TCP connections, enforced before any byte is read.
//!
//! The limits in limits.rs start at the WebSocket upgrade. Without a cap at `accept`, a client
//! could open tens of thousands of sockets and keep each alive with a cheap request now and
//! then (`GET /healthz` just inside the header read timeout); they would be counted nowhere.
//!
//! [`LimitAcceptor`] is the innermost acceptor (before TLS): it counts every socket from
//! `accept` until the stream is dropped, whatever is spoken on it (HTTP, a TLS handshake, an
//! upgraded WebSocket). Over a cap the new socket is closed at once, without an answer.
//!
//! - whole server: `limits.max_sockets`
//! - per peer address (IPv4 address, IPv6 /64): `limits.max_sockets_per_ip`
//! - per IPv6 /48: `limits.max_sockets_per_ip * limits.wide_open_factor`
//!
//! The address is the one of the socket, never `X-Forwarded-For` (no request was read yet).
//! Peers in `network.trusted_proxies` are exempt from the per-address caps: every client
//! arrives from the proxy's address. Only the whole-server cap applies to them, so the proxy
//! should limit connections per client itself.

use std::collections::HashMap;
use std::future::{Ready, ready};
use std::io;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use axum_server::accept::Accept;
use ipnet::IpNet;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;

use crate::config::{LimitsConfig, NetworkConfig};
use crate::limits::{limit_key, wide_cap, wide_key};

/// Counts the open sockets of the server.
pub(crate) struct SocketLimiter {
    max: usize,
    max_per_ip: usize,
    max_per_wide: usize,
    /// Peers that are exempt from the per-address caps (canonical networks).
    trusted_proxies: Vec<IpNet>,
    open: AtomicUsize,
    /// Open sockets per address (`false`) and per IPv6 /48 (`true`).
    per_ip: Mutex<HashMap<(IpAddr, bool), usize>>,
}

/// Held by a socket while it is open; frees its place when dropped.
pub(crate) struct SocketPermit {
    limiter: Arc<SocketLimiter>,
    /// The keys this socket is counted under (none for a trusted proxy).
    keys: [Option<(IpAddr, bool)>; 2],
}

impl SocketLimiter {
    pub(crate) fn new(limits: &LimitsConfig, network: &NetworkConfig) -> Arc<Self> {
        Arc::new(Self {
            max: limits.max_sockets,
            max_per_ip: limits.max_sockets_per_ip,
            max_per_wide: wide_cap(limits.max_sockets_per_ip, limits.wide_open_factor),
            trusted_proxies: crate::net::canonical_network(network.clone()).trusted_proxies,
            open: AtomicUsize::new(0),
            per_ip: Mutex::new(HashMap::new()),
        })
    }

    /// A new socket from `peer`: `None` when a cap is reached.
    pub(crate) fn admit(self: &Arc<Self>, peer: IpAddr) -> Option<SocketPermit> {
        let peer = crate::net::canonical(peer);
        // Reserve a place in the whole-server count first; dropping the permit gives it back.
        let before = self.open.fetch_add(1, Ordering::Relaxed);
        let mut permit = SocketPermit {
            limiter: self.clone(),
            keys: [None, None],
        };
        if before >= self.max {
            return None;
        }
        if self.trusted_proxies.iter().any(|n| n.contains(&peer)) {
            return Some(permit);
        }
        let narrow = (limit_key(peer), false);
        let wide = wide_key(peer).map(|w| (w, true));
        let mut map = self.per_ip.lock().unwrap_or_else(|e| e.into_inner());
        let count = |k: &(IpAddr, bool)| map.get(k).copied().unwrap_or(0);
        if count(&narrow) >= self.max_per_ip
            || wide.as_ref().is_some_and(|w| count(w) >= self.max_per_wide)
        {
            // Release the lock before the permit is dropped (its `Drop` takes the lock).
            drop(map);
            return None;
        }
        permit.keys = [Some(narrow), wide];
        for key in permit.keys.into_iter().flatten() {
            *map.entry(key).or_insert(0) += 1;
        }
        drop(map);
        Some(permit)
    }

    /// Number of open sockets (used by tests).
    #[cfg(test)]
    fn open(&self) -> usize {
        self.open.load(Ordering::Relaxed)
    }
}

impl Drop for SocketPermit {
    fn drop(&mut self) {
        self.limiter.open.fetch_sub(1, Ordering::Relaxed);
        if self.keys[0].is_none() {
            return;
        }
        let mut map = self
            .limiter
            .per_ip
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for key in self.keys.into_iter().flatten() {
            if let Some(n) = map.get_mut(&key) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    map.remove(&key);
                }
            }
        }
    }
}

/// Acceptor that refuses sockets above the caps and counts the others while they live.
#[derive(Clone)]
pub(crate) struct LimitAcceptor {
    limiter: Arc<SocketLimiter>,
}

impl LimitAcceptor {
    pub(crate) fn new(limiter: Arc<SocketLimiter>) -> Self {
        Self { limiter }
    }
}

impl<S> Accept<TcpStream, S> for LimitAcceptor {
    type Stream = Counted<TcpStream>;
    type Service = S;
    type Future = Ready<io::Result<(Self::Stream, Self::Service)>>;

    fn accept(&self, stream: TcpStream, service: S) -> Self::Future {
        // An error ends the connection task of axum-server: the socket is dropped (closed).
        let permit = stream.peer_addr().and_then(|peer| {
            self.limiter
                .admit(peer.ip())
                .ok_or_else(|| io::Error::other("too many connections"))
        });
        ready(permit.map(|permit| {
            (
                Counted {
                    inner: stream,
                    _permit: permit,
                },
                service,
            )
        }))
    }
}

/// A stream that holds its place in the socket count. The place is freed when the stream is
/// dropped, which for a WebSocket is when the upgraded connection ends.
pub(crate) struct Counted<S> {
    // Field order matters: the socket is closed before its place is given back.
    inner: S,
    _permit: SocketPermit,
}

impl<S: AsyncRead + Unpin> AsyncRead for Counted<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Counted<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Mode;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn limiter(max: usize, per_ip: usize, factor: u32, proxies: &[&str]) -> Arc<SocketLimiter> {
        let limits = LimitsConfig {
            max_sockets: max,
            max_sockets_per_ip: per_ip,
            wide_open_factor: factor,
            ..LimitsConfig::for_mode(Mode::Internal)
        };
        let network = NetworkConfig {
            trusted_proxies: proxies.iter().map(|s| s.parse().unwrap()).collect(),
            ..NetworkConfig::default()
        };
        SocketLimiter::new(&limits, &network)
    }

    #[test]
    fn sockets_are_capped_per_address_and_freed() {
        let l = limiter(100, 2, 4, &[]);
        let a = l.admit(ip("192.0.2.1")).unwrap();
        let _b = l.admit(ip("192.0.2.1")).unwrap();
        assert!(l.admit(ip("192.0.2.1")).is_none());
        // The IPv4-mapped form is the same client.
        assert!(l.admit(ip("::ffff:192.0.2.1")).is_none());
        assert_eq!(l.open(), 2, "a refused socket takes no place");
        // Another address is fine; a whole IPv6 /64 counts as one.
        let _c = l.admit(ip("192.0.2.2")).unwrap();
        let _d = l.admit(ip("2001:db8::1")).unwrap();
        let _e = l.admit(ip("2001:db8::2")).unwrap();
        assert!(l.admit(ip("2001:db8::3")).is_none());
        drop(a);
        assert!(l.admit(ip("192.0.2.1")).is_some());
    }

    #[test]
    fn whole_server_cap_and_cleanup() {
        let l = limiter(3, 100, 4, &[]);
        let held: Vec<_> = (1..=3)
            .map(|i| l.admit(ip(&format!("192.0.2.{i}"))).unwrap())
            .collect();
        assert!(l.admit(ip("192.0.2.9")).is_none());
        assert_eq!(l.open(), 3);
        drop(held);
        assert_eq!(l.open(), 0);
        assert!(l.per_ip.lock().unwrap().is_empty());
        assert!(l.admit(ip("192.0.2.9")).is_some());
    }

    #[test]
    fn an_ipv6_slash_48_is_capped_together() {
        // 2 per /64, factor 2: 4 per /48.
        let l = limiter(100, 2, 2, &[]);
        let held: Vec<_> = (0..4)
            .map(|i| l.admit(ip(&format!("2001:db8:1:{i}::1"))).unwrap())
            .collect();
        assert!(l.admit(ip("2001:db8:1:ffff::1")).is_none());
        assert!(l.admit(ip("2001:db8:2::1")).is_some());
        drop(held);
        assert!(l.admit(ip("2001:db8:1:ffff::1")).is_some());
    }

    #[test]
    fn trusted_proxies_only_obey_the_whole_server_cap() {
        let l = limiter(5, 1, 4, &["10.0.0.0/8", "::ffff:127.0.0.1/128"]);
        let mut held: Vec<_> = (0..4).map(|_| l.admit(ip("10.1.2.3")).unwrap()).collect();
        held.push(l.admit(ip("127.0.0.1")).unwrap());
        assert!(l.admit(ip("10.1.2.3")).is_none(), "whole server cap");
        assert!(l.per_ip.lock().unwrap().is_empty());
        drop(held);
        assert_eq!(l.open(), 0);
        // Everybody else has the per-address cap.
        let _a = l.admit(ip("192.0.2.1")).unwrap();
        assert!(l.admit(ip("192.0.2.1")).is_none());
    }
}
