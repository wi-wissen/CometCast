// SPDX-License-Identifier: GPL-3.0-or-later
//! Server addresses, TLS trust and a very small HTTP/1.1 GET client.
//!
//! The signalling server speaks WebSocket (`/ws`) and a few plain HTTP endpoints
//! (`/altcha/challenge`, `/ice-servers`, see docs/server.md). Native clients need both over
//! the same transport (plain TCP or TLS), so the TCP/TLS connection setup lives here and is
//! used by [`crate::SignalingClient`] and by [`http_get`].
//!
//! TLS uses rustls with the `ring` backend (the same as the server). Trust: the system
//! certificate store (via `rustls-native-certs`) plus optional extra certificates in PEM
//! form (for a self-signed development server).

use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use crate::error::TransportError;

/// UNCERTAIN: 10 s is a placeholder for one HTTP request (connect, TLS and answer).
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);
/// Largest HTTP answer we accept (the real answers are far below 4 KiB).
const MAX_HTTP_BYTES: usize = 256 * 1024;

/// Where the signalling server is, parsed from the text a user types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerAddr {
    /// `true` for `wss://` / `https://`.
    pub tls: bool,
    /// Host name or IP address (IPv6 without brackets).
    pub host: String,
    /// TCP port (80 or 443 when the text had none).
    pub port: u16,
    /// WebSocket path, `/ws` unless the text had another one.
    pub ws_path: String,
}

impl ServerAddr {
    /// Parse `host:port`, `host:port/ws`, `ws://...`, `wss://...`, `http://...`, `https://...`.
    /// Text without a scheme means plain `ws://`. An empty path becomes `/ws`.
    pub fn parse(server: &str) -> Result<Self, TransportError> {
        let server = server.trim();
        let (tls, rest) = if let Some(r) = server.strip_prefix("wss://") {
            (true, r)
        } else if let Some(r) = server.strip_prefix("https://") {
            (true, r)
        } else if let Some(r) = server.strip_prefix("ws://") {
            (false, r)
        } else if let Some(r) = server.strip_prefix("http://") {
            (false, r)
        } else if server.contains("://") {
            return Err(TransportError::Invalid(format!(
                "unknown URL scheme in {server:?}"
            )));
        } else {
            (false, server)
        };
        let (authority, path) = match rest.find('/') {
            Some(i) => rest.split_at(i),
            None => (rest, ""),
        };
        let (host, port) = split_host_port(authority, if tls { 443 } else { 80 })?;
        let ws_path = if path.is_empty() || path == "/" {
            "/ws".to_string()
        } else {
            path.to_string()
        };
        Ok(Self {
            tls,
            host,
            port,
            ws_path,
        })
    }

    /// `host:port` for the `Host` header and URLs (IPv6 in brackets).
    pub fn authority(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    /// The WebSocket URL, e.g. `wss://host:443/ws`.
    pub fn ws_url(&self) -> String {
        let scheme = if self.tls { "wss" } else { "ws" };
        format!("{scheme}://{}{}", self.authority(), self.ws_path)
    }
}

fn split_host_port(authority: &str, default_port: u16) -> Result<(String, u16), TransportError> {
    if authority.is_empty() {
        return Err(TransportError::Invalid("empty server address".into()));
    }
    let bad = || TransportError::Invalid(format!("bad server address {authority:?}"));
    if let Some(rest) = authority.strip_prefix('[') {
        // [v6] or [v6]:port
        let (host, after) = rest.split_once(']').ok_or_else(bad)?;
        let port = match after {
            "" => default_port,
            a => a
                .strip_prefix(':')
                .and_then(|p| p.parse().ok())
                .ok_or_else(bad)?,
        };
        return Ok((host.to_string(), port));
    }
    match authority.rsplit_once(':') {
        // More than one colon without brackets would be an unbracketed IPv6 address.
        Some((host, _)) if host.contains(':') => Err(bad()),
        Some((host, port)) if !host.is_empty() => {
            Ok((host.to_string(), port.parse().map_err(|_| bad())?))
        }
        Some(_) => Err(bad()),
        None => Ok((authority.to_string(), default_port)),
    }
}

/// Which certificates a TLS connection trusts: the system store plus extra ones.
#[derive(Debug, Clone, Default)]
pub struct TlsTrust {
    extra_roots: Vec<CertificateDer<'static>>,
}

impl TlsTrust {
    /// Trust only what the system store trusts.
    pub fn system() -> Self {
        Self::default()
    }

    /// Additionally trust the certificates in this PEM text (for example the public
    /// certificate of a self-signed development server). At least one is required.
    pub fn add_pem(&mut self, pem: &[u8]) -> Result<(), TransportError> {
        let mut found = 0;
        for cert in CertificateDer::pem_slice_iter(pem) {
            let cert = cert.map_err(|e| TransportError::Invalid(format!("bad PEM: {e}")))?;
            self.extra_roots.push(cert);
            found += 1;
        }
        if found == 0 {
            return Err(TransportError::Invalid(
                "no certificate found in the PEM data".into(),
            ));
        }
        Ok(())
    }

    fn client_config(&self) -> Result<Arc<rustls::ClientConfig>, TransportError> {
        let mut roots = rustls::RootCertStore::empty();
        // UNCERTAIN: a system store that cannot be read (or is empty, as on Android) is not
        // an error: extra certificates may still be enough. A failing handshake says so later.
        let native = rustls_native_certs::load_native_certs();
        let _ = roots.add_parsable_certificates(native.certs);
        for c in &self.extra_roots {
            roots
                .add(c.clone())
                .map_err(|e| TransportError::Invalid(format!("unusable certificate: {e}")))?;
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let cfg = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| TransportError::Connect(format!("TLS setup: {e}")))?
            .with_root_certificates(roots)
            .with_no_client_auth();
        Ok(Arc::new(cfg))
    }
}

/// A byte stream, plain or TLS.
pub(crate) trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}
pub(crate) type BoxedIo = Box<dyn Io>;

/// Open a TCP connection to the server and, for `wss://` / `https://`, run the TLS handshake.
pub(crate) async fn connect_io(
    addr: &ServerAddr,
    trust: &TlsTrust,
) -> Result<BoxedIo, TransportError> {
    let tcp = TcpStream::connect((addr.host.as_str(), addr.port))
        .await
        .map_err(|e| TransportError::Connect(format!("{}: {e}", addr.authority())))?;
    // Small messages; do not wait for Nagle. A failure here is harmless.
    let _ = tcp.set_nodelay(true);
    if !addr.tls {
        return Ok(Box::new(tcp));
    }
    let name = ServerName::try_from(addr.host.clone())
        .map_err(|e| TransportError::Invalid(format!("bad TLS server name: {e}")))?;
    let tls = TlsConnector::from(trust.client_config()?)
        .connect(name, tcp)
        .await
        .map_err(|e| TransportError::Connect(format!("TLS to {}: {e}", addr.authority())))?;
    Ok(Box::new(tls))
}

/// A parsed HTTP answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// Status code, e.g. 200.
    pub status: u16,
    /// `Retry-After` in seconds, if the header was present and numeric.
    pub retry_after: Option<u64>,
    /// Body bytes (after chunked decoding).
    pub body: Vec<u8>,
}

/// `GET path` on the server (plain or TLS). One request per connection.
pub async fn http_get(
    addr: &ServerAddr,
    path: &str,
    trust: &TlsTrust,
) -> Result<HttpResponse, TransportError> {
    let work = async {
        let mut io = connect_io(addr, trust).await?;
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: {}\r\nAccept: application/json\r\nUser-Agent: cometcast\r\nConnection: close\r\n\r\n",
            addr.authority()
        );
        io.write_all(req.as_bytes())
            .await
            .map_err(|e| TransportError::Connect(e.to_string()))?;
        io.flush()
            .await
            .map_err(|e| TransportError::Connect(e.to_string()))?;
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            if let Some(resp) = parse_response(&buf, false)? {
                return Ok(resp);
            }
            if buf.len() > MAX_HTTP_BYTES {
                return Err(TransportError::Protocol("HTTP answer too large".into()));
            }
            match io.read(&mut chunk).await {
                Ok(0) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                // A server may close TLS without close_notify; treat it as the end.
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(TransportError::Connect(e.to_string())),
            }
        }
        parse_response(&buf, true)?
            .ok_or_else(|| TransportError::Protocol("HTTP answer cut off".into()))
    };
    tokio::time::timeout(HTTP_TIMEOUT, work)
        .await
        .map_err(|_| TransportError::Timeout("HTTP request"))?
}

/// Longest chunk-size line (hex size plus extensions) we accept.
const MAX_CHUNK_LINE: usize = 1024;

/// Parse a (possibly partial) HTTP/1.1 answer. `Ok(None)` = need more bytes.
/// `eof` = the peer closed the connection, so a body without length ends here.
///
/// Strict on purpose: the bytes come from a server we do not trust. Duplicate or non-numeric
/// `Content-Length`, `Content-Length` together with `Transfer-Encoding`, an unknown transfer
/// coding, a body longer than declared and malformed header lines are errors. Never panics.
fn parse_response(buf: &[u8], eof: bool) -> Result<Option<HttpResponse>, TransportError> {
    let proto = |m: &str| TransportError::Protocol(m.to_string());
    let Some(head_end) = find(buf, b"\r\n\r\n") else {
        return Ok(None);
    };
    let head = std::str::from_utf8(&buf[..head_end]).map_err(|_| proto("HTTP head is not text"))?;
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let mut parts = status_line.split_whitespace();
    if !parts.next().is_some_and(|v| v.starts_with("HTTP/1.")) {
        return Err(TransportError::Protocol(format!(
            "bad status line {status_line:?}"
        )));
    }
    let status: u16 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| TransportError::Protocol(format!("bad status line {status_line:?}")))?;
    let mut content_length: Option<usize> = None;
    let mut transfer_encoding: Option<String> = None;
    let mut retry_after = None;
    for line in lines {
        let (k, v) = line
            .split_once(':')
            .ok_or_else(|| proto("malformed HTTP header line"))?;
        let v = v.trim();
        match k.trim().to_ascii_lowercase().as_str() {
            "content-length" => {
                if content_length.is_some() {
                    return Err(proto("duplicate Content-Length"));
                }
                if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(proto("invalid Content-Length"));
                }
                let n: usize = v.parse().map_err(|_| proto("invalid Content-Length"))?;
                if n > MAX_HTTP_BYTES {
                    return Err(proto("HTTP answer too large"));
                }
                content_length = Some(n);
            }
            "transfer-encoding" => {
                if transfer_encoding.is_some() {
                    return Err(proto("duplicate Transfer-Encoding"));
                }
                transfer_encoding = Some(v.to_ascii_lowercase());
            }
            "retry-after" => retry_after = v.parse().ok(),
            _ => {}
        }
    }
    let chunked = match transfer_encoding.as_deref() {
        None => false,
        Some("chunked") => true,
        Some(_) => return Err(proto("unsupported Transfer-Encoding")),
    };
    if chunked && content_length.is_some() {
        return Err(proto("both Transfer-Encoding and Content-Length"));
    }
    let rest = &buf[head_end + 4..];
    let body = if chunked {
        match decode_chunked(rest)? {
            Some(b) => b,
            None => return Ok(None),
        }
    } else if let Some(n) = content_length {
        if rest.len() < n {
            return Ok(None);
        }
        if rest.len() > n {
            return Err(proto("HTTP body longer than Content-Length"));
        }
        rest.to_vec()
    } else if eof || status == 204 || status == 304 || (100..200).contains(&status) {
        rest.to_vec()
    } else {
        return Ok(None);
    };
    Ok(Some(HttpResponse {
        status,
        retry_after,
        body,
    }))
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Decode a chunked body. `Ok(None)` = the last chunk has not arrived yet.
///
/// All arithmetic is checked and the decoded size is capped at [`MAX_HTTP_BYTES`], so no byte
/// sequence can overflow, allocate without bound or panic. Trailers are ignored.
fn decode_chunked(mut data: &[u8]) -> Result<Option<Vec<u8>>, TransportError> {
    let bad = || TransportError::Protocol("bad chunked body".into());
    let mut out: Vec<u8> = Vec::new();
    loop {
        let Some(line_end) = find(data, b"\r\n") else {
            if data.len() > MAX_CHUNK_LINE {
                return Err(bad());
            }
            return Ok(None);
        };
        if line_end > MAX_CHUNK_LINE {
            return Err(bad());
        }
        let size_text = std::str::from_utf8(&data[..line_end]).map_err(|_| bad())?;
        let size_text = size_text.split(';').next().unwrap_or_default().trim();
        // Only hex digits: `from_str_radix` would also take a leading `+`.
        if size_text.is_empty() || !size_text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(bad());
        }
        let size = usize::from_str_radix(size_text, 16).map_err(|_| bad())?;
        data = &data[line_end + 2..];
        if size == 0 {
            // Trailers are ignored; the final blank line may still be on its way.
            return Ok(Some(out));
        }
        if size > MAX_HTTP_BYTES || out.len().saturating_add(size) > MAX_HTTP_BYTES {
            return Err(TransportError::Protocol("HTTP answer too large".into()));
        }
        // `size` is at most MAX_HTTP_BYTES here, so `size + 2` cannot overflow.
        let need = size + 2;
        if data.len() < need {
            return Ok(None);
        }
        if &data[size..need] != b"\r\n" {
            return Err(bad());
        }
        out.extend_from_slice(&data[..size]);
        data = &data[need..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_addr_forms() {
        let p = |s: &str| ServerAddr::parse(s).unwrap();
        assert_eq!(p("127.0.0.1:8443").ws_url(), "ws://127.0.0.1:8443/ws");
        assert_eq!(p("example.org:1/").ws_url(), "ws://example.org:1/ws");
        assert_eq!(p("ws://x:1/other").ws_url(), "ws://x:1/other");
        assert_eq!(p("http://x:1").ws_url(), "ws://x:1/ws");
        assert_eq!(p("http://x").ws_url(), "ws://x:80/ws");
        assert_eq!(p("wss://x").ws_url(), "wss://x:443/ws");
        assert_eq!(p("https://x:8443").ws_url(), "wss://x:8443/ws");
        assert!(p("https://x:8443").tls);
        assert_eq!(p("[::1]:9").ws_url(), "ws://[::1]:9/ws");
        assert_eq!(p("[::1]:9").host, "::1");
        assert!(ServerAddr::parse("::1").is_err());
        assert!(ServerAddr::parse("ftp://x").is_err());
        assert!(ServerAddr::parse("").is_err());
        assert!(ServerAddr::parse("x:notaport").is_err());
        assert!(ServerAddr::parse("x:99999").is_err());
    }

    #[test]
    fn parses_content_length_and_partial_answers() {
        let full = b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\nretry-after: 7\r\n\r\nhello";
        let r = parse_response(full, false).unwrap().unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.retry_after, Some(7));
        assert_eq!(r.body, b"hello");
        assert!(
            parse_response(&full[..full.len() - 2], false)
                .unwrap()
                .is_none()
        );
        assert!(
            parse_response(b"HTTP/1.1 200 OK\r\ncon", false)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn parses_chunked_and_eof_bodies() {
        let c = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2;x=y\r\nde\r\n0\r\n\r\n";
        assert_eq!(parse_response(c, false).unwrap().unwrap().body, b"abcde");
        assert!(parse_response(&c[..c.len() - 8], false).unwrap().is_none());
        let e = b"HTTP/1.1 404 Not Found\r\n\r\nnope";
        assert!(parse_response(e, false).unwrap().is_none());
        let r = parse_response(e, true).unwrap().unwrap();
        assert_eq!((r.status, r.body.as_slice()), (404, &b"nope"[..]));
        assert!(parse_response(b"garbage\r\n\r\n", false).is_err());
        assert!(
            parse_response(
                b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\nzz\r\n",
                false
            )
            .is_err()
        );
    }

    #[test]
    fn chunked_never_panics_on_hostile_sizes() {
        for size in [
            "ffffffffffffffff",
            "fffffffffffffffe",
            "7fffffffffffffff",
            "8000000000000000",
            "ffffffffffffffffff",
            "40001",
            "+5",
            "-1",
            "0x5",
            "",
            " ",
            "zz",
        ] {
            let body = format!("{size}\r\nabc\r\n0\r\n\r\n");
            let r = decode_chunked(body.as_bytes());
            assert!(r.is_err(), "size {size:?} must be refused, got {r:?}");
        }
        // Huge size with an extension, hostile extensions and trailers.
        assert!(decode_chunked(b"ffffffffffffffff;ext=1\r\n").is_err());
        assert_eq!(
            decode_chunked(b"3;a=\"x;y\"\r\nabc\r\n0;z\r\nX: y\r\n\xff\xfe\r\n\r\n")
                .unwrap()
                .unwrap(),
            b"abc"
        );
        assert!(decode_chunked(&[b'a'; 5000]).is_err());
        let long_ext = format!("3;{}\r\nabc\r\n0\r\n\r\n", "e".repeat(2000));
        assert!(decode_chunked(long_ext.as_bytes()).is_err());
        // Missing CRLF after the data.
        assert!(decode_chunked(b"3\r\nabcXY0\r\n\r\n").is_err());
    }

    #[test]
    fn chunked_total_size_is_capped() {
        let mut body = Vec::new();
        let chunk = vec![b'a'; 100 * 1024];
        for _ in 0..4 {
            body.extend_from_slice(b"19000\r\n");
            body.extend_from_slice(&chunk);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(b"0\r\n\r\n");
        assert!(decode_chunked(&body).is_err());
    }

    #[test]
    fn response_fuzz_style_mutations_do_not_panic() {
        // Deterministic pseudo-random mutations and cuts of a valid answer: only the absence
        // of a panic matters.
        let valid = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n";
        let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..20_000 {
            let mut m = valid.to_vec();
            for _ in 0..3 {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                let i = (x as usize) % m.len();
                m[i] = (x >> 32) as u8;
            }
            let cut = ((x >> 8) as usize) % (m.len() + 1);
            let _ = parse_response(&m[..cut], false);
            let _ = parse_response(&m, true);
        }
    }

    #[test]
    fn strict_header_parsing() {
        let r = |s: &[u8]| parse_response(s, false);
        // Duplicate or invalid Content-Length.
        assert!(r(b"HTTP/1.1 200 OK\r\ncontent-length: 1\r\ncontent-length: 1\r\n\r\na").is_err());
        assert!(r(b"HTTP/1.1 200 OK\r\ncontent-length: 1\r\nContent-Length: 2\r\n\r\na").is_err());
        assert!(r(b"HTTP/1.1 200 OK\r\ncontent-length: abc\r\n\r\n").is_err());
        assert!(r(b"HTTP/1.1 200 OK\r\ncontent-length: -1\r\n\r\n").is_err());
        assert!(r(b"HTTP/1.1 200 OK\r\ncontent-length: +1\r\n\r\na").is_err());
        assert!(r(b"HTTP/1.1 200 OK\r\ncontent-length: \r\n\r\n").is_err());
        assert!(r(b"HTTP/1.1 200 OK\r\ncontent-length: 99999999999999999999\r\n\r\n").is_err());
        assert!(r(b"HTTP/1.1 200 OK\r\ncontent-length: 999999999\r\n\r\n").is_err());
        // Both framings.
        assert!(
            r(b"HTTP/1.1 200 OK\r\ncontent-length: 1\r\ntransfer-encoding: chunked\r\n\r\n1\r\na\r\n0\r\n\r\n")
                .is_err()
        );
        // Unknown coding, malformed line, wrong protocol.
        assert!(r(b"HTTP/1.1 200 OK\r\ntransfer-encoding: gzip, chunked\r\n\r\n").is_err());
        assert!(r(b"HTTP/1.1 200 OK\r\nno colon here\r\n\r\n").is_err());
        assert!(r(b"FTP/1.1 200 OK\r\n\r\n").is_err());
        // Body longer than declared.
        assert!(r(b"HTTP/1.1 200 OK\r\ncontent-length: 1\r\n\r\nab").is_err());
        // Exact length still works.
        assert_eq!(
            r(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nab")
                .unwrap()
                .unwrap()
                .body,
            b"ab"
        );
    }

    #[test]
    fn trust_rejects_pem_without_certificate() {
        let mut t = TlsTrust::system();
        assert!(t.add_pem(b"not a certificate").is_err());
    }
}
