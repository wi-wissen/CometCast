// SPDX-License-Identifier: GPL-3.0-or-later
//! Time limits for slow or silent HTTP clients (slowloris).
//!
//! hyper's HTTP/1 `header_read_timeout` only works with a timer, and axum 0.8 sets none, so
//! by default a client could hold a connection for ever by sending a request head very
//! slowly. Two things are done here:
//!
//! - [`tune`] gives hyper a timer and a header read timeout. hyper starts it each time it
//!   waits for a request head, so it also ends idle keep-alive connections.
//! - [`FirstRequestDeadline`] wraps every accepted stream: until the server has written its
//!   first byte (the first response, or `101 Switching Protocols` for a WebSocket), reads
//!   fail after [`HEADER_READ_TIMEOUT`]. This covers the time before hyper reads a head at
//!   all: hyper-util first waits for bytes to tell HTTP/1 from HTTP/2, without any timeout,
//!   so a client that connects and sends nothing would otherwise stay. With TLS the wrapper
//!   sits outside the TLS layer, after the handshake (which has its own 10 s limit in
//!   axum-server).
//!
//! An upgraded WebSocket is not affected: by then the server has written the 101 response,
//! and hyper no longer reads request heads on it.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use axum_server::accept::Accept;
use hyper_util::rt::TokioTimer;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::Sleep;

/// How long a client may take to send a complete request head (and, on a new connection,
/// its first byte). UNCERTAIN: 15 s is a placeholder; slow mobile networks are not measured.
pub(crate) const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(15);

/// Give hyper a timer and the header read timeout.
pub(crate) fn tune<A: axum_server::Address, Acc>(server: &mut axum_server::Server<A, Acc>) {
    server
        .http_builder()
        .http1()
        .timer(TokioTimer::new())
        .header_read_timeout(HEADER_READ_TIMEOUT);
}

/// Acceptor that wraps the streams of `inner` in a [`Deadline`].
#[derive(Clone, Debug)]
pub(crate) struct FirstRequestDeadline<A> {
    inner: A,
    limit: Duration,
}

impl<A> FirstRequestDeadline<A> {
    pub(crate) fn new(inner: A) -> Self {
        Self {
            inner,
            limit: HEADER_READ_TIMEOUT,
        }
    }
}

impl<A, I, S> Accept<I, S> for FirstRequestDeadline<A>
where
    A: Accept<I, S>,
{
    type Stream = Deadline<A::Stream>;
    type Service = A::Service;
    type Future = DeadlineFuture<A::Future>;

    fn accept(&self, stream: I, service: S) -> Self::Future {
        DeadlineFuture {
            inner: Box::pin(self.inner.accept(stream, service)),
            limit: self.limit,
        }
    }
}

/// Future of [`FirstRequestDeadline::accept`]: the inner accept, then the wrapper.
pub(crate) struct DeadlineFuture<F> {
    inner: Pin<Box<F>>,
    limit: Duration,
}

impl<F, St, Sv> Future for DeadlineFuture<F>
where
    F: Future<Output = io::Result<(St, Sv)>>,
{
    type Output = io::Result<(Deadline<St>, Sv)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let limit = self.limit;
        self.inner
            .as_mut()
            .poll(cx)
            .map_ok(|(stream, service)| (Deadline::new(stream, limit), service))
    }
}

/// A stream whose reads fail after a time limit until the first byte has been written.
pub(crate) struct Deadline<S> {
    inner: S,
    /// `None` once the server has written something.
    sleep: Option<Pin<Box<Sleep>>>,
}

impl<S> Deadline<S> {
    pub(crate) fn new(inner: S, limit: Duration) -> Self {
        Self {
            inner,
            sleep: Some(Box::pin(tokio::time::sleep(limit))),
        }
    }

    fn wrote(&mut self, n: usize) {
        if n > 0 {
            self.sleep = None;
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Deadline<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let res = Pin::new(&mut this.inner).poll_read(cx, buf);
        if res.is_pending()
            && let Some(sleep) = this.sleep.as_mut()
            && sleep.as_mut().poll(cx).is_ready()
        {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "no request in time",
            )));
        }
        res
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Deadline<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let res = Pin::new(&mut this.inner).poll_write(cx, buf);
        if let Poll::Ready(Ok(n)) = res {
            this.wrote(n);
        }
        res
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let res = Pin::new(&mut this.inner).poll_write_vectored(cx, bufs);
        if let Poll::Ready(Ok(n)) = res {
            this.wrote(n);
        }
        res
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test(start_paused = true)]
    async fn silent_client_times_out_before_the_first_response() {
        let (_client, server) = tokio::io::duplex(64);
        let mut s = Deadline::new(server, Duration::from_secs(15));
        let mut buf = [0u8; 8];
        let err = s.read(&mut buf).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }

    #[tokio::test(start_paused = true)]
    async fn after_the_first_write_reads_may_wait() {
        let (mut client, server) = tokio::io::duplex(64);
        let mut s = Deadline::new(server, Duration::from_secs(15));
        s.write_all(b"HTTP/1.1 101").await.unwrap();
        // Long after the limit, a read still waits for data and then gets it.
        let reader = tokio::spawn(async move {
            let mut buf = [0u8; 2];
            s.read_exact(&mut buf).await.map(|_| buf)
        });
        tokio::time::sleep(Duration::from_secs(60)).await;
        client.write_all(b"hi").await.unwrap();
        assert_eq!(&reader.await.unwrap().unwrap(), b"hi");
    }
}
