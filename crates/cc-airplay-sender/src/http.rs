// SPDX-License-Identifier: GPL-3.0-or-later
//! A small HTTP/1.x implementation for both sides: the client for the receiver's control
//! port and the reading side of our own HLS server. Everything the peer sends is untrusted,
//! so every size is capped and every wait has a deadline.

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use crate::error::{Result, SenderError, clean};
use crate::framed::Framed;

/// Largest request or response head (start line and headers).
pub const MAX_HEAD_BYTES: usize = 16 * 1024;
/// Largest body we accept from a receiver (untrusted).
pub const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
/// Most header lines accepted.
pub const MAX_HEADERS: usize = 100;
/// Longest wait for a whole message.
pub const MESSAGE_TIMEOUT: Duration = Duration::from_secs(20);

/// An HTTP request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Request {
    /// Method, for example `POST`.
    pub method: String,
    /// Path with query, for example `/play`.
    pub path: String,
    /// Header fields in order.
    pub headers: Vec<(String, String)>,
    /// Body bytes.
    pub body: Vec<u8>,
}

/// An HTTP response.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Response {
    /// Status code.
    pub status: u16,
    /// Header fields in order.
    pub headers: Vec<(String, String)>,
    /// Body bytes.
    pub body: Vec<u8>,
}

fn find<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

impl Request {
    /// A request with no headers and no body.
    pub fn new(method: &str, path: &str) -> Self {
        Self {
            method: method.into(),
            path: path.into(),
            ..Self::default()
        }
    }

    /// Add a header (builder style).
    pub fn header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Set the body without touching the headers (builder style).
    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    /// Set the body and its content type (builder style).
    pub fn body(mut self, content_type: &str, body: Vec<u8>) -> Self {
        self.headers
            .push(("Content-Type".into(), content_type.into()));
        self.body = body;
        self
    }

    /// First header with this name (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&str> {
        find(&self.headers, name)
    }
}

impl Response {
    /// First header with this name (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&str> {
        find(&self.headers, name)
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        206 => "Partial Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        416 => "Range Not Satisfiable",
        453 => "Not Enough Bandwidth",
        470 => "Connection Authorization Required",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

/// An HTTP connection over any byte stream (a TCP stream, optionally with the encrypted
/// framing of [`crate::framed`]).
pub struct Conn<S> {
    io: Framed<S>,
    buf: Vec<u8>,
    closed: bool,
    /// An I/O or framing error happened: the stream position (and, when encrypted, the
    /// frame counters) can no longer be trusted, so the connection is never used again.
    broken: bool,
    /// Client side: send `RTSP/1.0` requests with a `CSeq` header instead of `HTTP/1.1`.
    /// Server side: the last request was RTSP, so answer in RTSP.
    rtsp: bool,
    cseq: u32,
    peer_cseq: Option<String>,
}

impl<S: Read + Write> Conn<S> {
    /// Wrap a stream. Timeouts of the stream itself must be set by the caller.
    pub fn new(stream: S) -> Self {
        Self {
            io: Framed::new(stream),
            buf: Vec::new(),
            closed: false,
            broken: false,
            rtsp: false,
            cseq: 0,
            peer_cseq: None,
        }
    }

    /// Client side: speak RTSP/1.0 (with `CSeq`) instead of HTTP/1.1. Some receivers, among
    /// them UxPlay, answer only that on their control port.
    pub fn set_rtsp(&mut self, rtsp: bool) {
        self.rtsp = rtsp;
    }

    /// The framing layer, read-only (to reach the socket).
    pub fn framed_ref(&self) -> &Framed<S> {
        &self.io
    }

    /// Whether this connection speaks RTSP/1.0.
    pub fn is_rtsp(&self) -> bool {
        self.rtsp
    }

    /// Switch the connection to the encrypted framing. Fails when plaintext bytes that
    /// followed the last message are still buffered: they would otherwise be taken for
    /// authenticated data on the encrypted channel (an on-path attacker could forge
    /// answers by appending bytes to the last plaintext response).
    pub fn enable_encryption(&mut self, cipher: crate::framed::Cipher) -> Result<()> {
        if !self.buf.is_empty() {
            return Err(SenderError::Protocol(
                "unexpected plaintext data after the last pairing answer".into(),
            ));
        }
        self.io.enable(cipher);
        Ok(())
    }

    /// Access the framing layer (to switch encryption on).
    pub fn framed_mut(&mut self) -> &mut Framed<S> {
        &mut self.io
    }

    /// Whether the peer announced `Connection: close` or the stream ended.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    fn fill(&mut self, deadline: Instant) -> Result<usize> {
        if Instant::now() > deadline {
            return Err(SenderError::Timeout("waiting for the peer"));
        }
        let mut chunk = [0u8; 8192];
        let n = self.io.read(&mut chunk)?;
        if n == 0 {
            self.closed = true;
            return Err(SenderError::Io("connection closed by the peer".into()));
        }
        self.buf.extend_from_slice(&chunk[..n]);
        Ok(n)
    }

    /// Read a head up to and including the empty line; returns the lines.
    fn read_head(&mut self, deadline: Instant) -> Result<Vec<String>> {
        loop {
            if let Some(pos) = find_subslice(&self.buf, b"\r\n\r\n") {
                let head: Vec<u8> = self.buf.drain(..pos + 4).collect();
                if head.len() > MAX_HEAD_BYTES {
                    return Err(SenderError::TooLarge("HTTP head"));
                }
                let text = String::from_utf8_lossy(&head[..pos]).into_owned();
                return Ok(text.split("\r\n").map(str::to_owned).collect());
            }
            if self.buf.len() > MAX_HEAD_BYTES {
                return Err(SenderError::TooLarge("HTTP head"));
            }
            self.fill(deadline)?;
        }
    }

    fn read_exact_body(&mut self, len: usize, deadline: Instant) -> Result<Vec<u8>> {
        while self.buf.len() < len {
            self.fill(deadline)?;
        }
        Ok(self.buf.drain(..len).collect())
    }

    fn read_line(&mut self, deadline: Instant) -> Result<String> {
        loop {
            if let Some(pos) = find_subslice(&self.buf, b"\r\n") {
                let line: Vec<u8> = self.buf.drain(..pos + 2).collect();
                return Ok(String::from_utf8_lossy(&line[..pos]).into_owned());
            }
            if self.buf.len() > 1024 {
                return Err(SenderError::TooLarge("chunk header"));
            }
            self.fill(deadline)?;
        }
    }

    fn read_chunked(&mut self, max: usize, deadline: Instant) -> Result<Vec<u8>> {
        let mut body = Vec::new();
        loop {
            let line = self.read_line(deadline)?;
            let size_text = line.split(';').next().unwrap_or("").trim();
            let size = usize::from_str_radix(size_text, 16)
                .map_err(|_| SenderError::Protocol("bad chunk size".into()))?;
            if size == 0 {
                // Trailer lines until the empty line.
                for _ in 0..MAX_HEADERS {
                    if self.read_line(deadline)?.is_empty() {
                        return Ok(body);
                    }
                }
                return Err(SenderError::TooLarge("chunk trailer"));
            }
            if body.len().saturating_add(size) > max {
                return Err(SenderError::TooLarge("HTTP body"));
            }
            body.extend(self.read_exact_body(size, deadline)?);
            if !self.read_line(deadline)?.is_empty() {
                return Err(SenderError::Protocol("missing chunk end".into()));
            }
        }
    }

    fn read_headers(lines: &[String]) -> Result<Vec<(String, String)>> {
        if lines.len() > MAX_HEADERS + 1 {
            return Err(SenderError::TooLarge("number of headers"));
        }
        let mut out = Vec::new();
        for line in lines.iter().skip(1) {
            let Some((k, v)) = line.split_once(':') else {
                return Err(SenderError::Protocol("header line without colon".into()));
            };
            out.push((k.trim().to_owned(), v.trim().to_owned()));
        }
        Ok(out)
    }

    fn content_length(headers: &[(String, String)], max: usize) -> Result<Option<usize>> {
        match find(headers, "content-length") {
            None => Ok(None),
            Some(v) => {
                let n: usize = v
                    .trim()
                    .parse()
                    .map_err(|_| SenderError::Protocol("bad Content-Length".into()))?;
                if n > max {
                    return Err(SenderError::TooLarge("HTTP body"));
                }
                Ok(Some(n))
            }
        }
    }

    fn check_usable(&self) -> Result<()> {
        if self.broken {
            Err(SenderError::Io(
                "the connection was dropped after an earlier error; reconnect".into(),
            ))
        } else {
            Ok(())
        }
    }

    /// Record that an error left the stream in an unknown state. Bad input from our own
    /// caller does not touch the stream.
    fn track<T>(&mut self, r: Result<T>) -> Result<T> {
        if matches!(&r, Err(e) if !matches!(e, SenderError::Input(_))) {
            self.broken = true;
        }
        r
    }

    /// Send a request. After any error other than bad input the connection is discarded: a
    /// partly written encrypted frame would desynchronise the counters.
    pub fn send_request(&mut self, req: &Request) -> Result<()> {
        self.check_usable()?;
        let r = self.send_request_inner(req);
        self.track(r)
    }

    fn send_request_inner(&mut self, req: &Request) -> Result<()> {
        if req.method.contains(|c: char| c.is_whitespace())
            || req
                .path
                .contains(|c: char| c.is_whitespace() || c.is_control())
        {
            return Err(SenderError::Input("whitespace in method or path".into()));
        }
        let version = if self.rtsp { "RTSP/1.0" } else { "HTTP/1.1" };
        let mut head = format!("{} {} {version}\r\n", req.method, req.path);
        if self.rtsp && find(&req.headers, "cseq").is_none() {
            self.cseq += 1;
            head.push_str(&format!("CSeq: {}\r\n", self.cseq));
        }
        for (k, v) in &req.headers {
            if k.contains([':', '\r', '\n']) || v.contains(['\r', '\n']) {
                return Err(SenderError::Input("bad header".into()));
            }
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        if find(&req.headers, "content-length").is_none() {
            head.push_str(&format!("Content-Length: {}\r\n", req.body.len()));
        }
        head.push_str("\r\n");
        let mut wire = head.into_bytes();
        wire.extend_from_slice(&req.body);
        self.io.write_all(&wire)?;
        self.io.flush()?;
        Ok(())
    }

    /// Read the answer to a request. `head_only` is true for HEAD requests.
    pub fn read_response(&mut self, head_only: bool) -> Result<Response> {
        self.check_usable()?;
        let r = self.read_response_inner(head_only);
        self.track(r)
    }

    fn read_response_inner(&mut self, head_only: bool) -> Result<Response> {
        let deadline = Instant::now() + MESSAGE_TIMEOUT;
        for _ in 0..4 {
            let lines = self.read_head(deadline)?;
            let start = lines.first().map(String::as_str).unwrap_or("");
            let mut parts = start.splitn(3, ' ');
            let version = parts.next().unwrap_or("");
            if !(version == "HTTP/1.1" || version == "HTTP/1.0" || version == "RTSP/1.0") {
                return Err(SenderError::Protocol(format!(
                    "unexpected start line {:?}",
                    clean(start, 40)
                )));
            }
            let status: u16 = parts
                .next()
                .and_then(|s| s.parse().ok())
                .filter(|s| (100..600).contains(s))
                .ok_or_else(|| SenderError::Protocol("bad status code".into()))?;
            let headers = Self::read_headers(&lines)?;
            if (100..200).contains(&status) {
                continue;
            }
            if find(&headers, "connection").is_some_and(|v| v.eq_ignore_ascii_case("close")) {
                self.closed = true;
            }
            let no_body = head_only || status == 204 || status == 304;
            let body = if no_body {
                Vec::new()
            } else if find(&headers, "transfer-encoding")
                .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"))
            {
                self.read_chunked(MAX_BODY_BYTES, deadline)?
            } else if let Some(n) = Self::content_length(&headers, MAX_BODY_BYTES)? {
                self.read_exact_body(n, deadline)?
            } else if version != "HTTP/1.0" {
                // HTTP/1.1 and RTSP without a length: an empty body. (Strictly, HTTP/1.1
                // would read to the end of the connection, but UxPlay's RTSP answers such
                // as `pair-pin-start` have no length and keep the connection open.)
                Vec::new()
            } else {
                // HTTP/1.0 without a length: the body runs to the end of the connection.
                loop {
                    if self.buf.len() > MAX_BODY_BYTES {
                        return Err(SenderError::TooLarge("HTTP body"));
                    }
                    match self.fill(deadline) {
                        Ok(_) => {}
                        Err(_) if self.closed => break,
                        Err(e) => return Err(e),
                    }
                }
                std::mem::take(&mut self.buf)
            };
            return Ok(Response {
                status,
                headers,
                body,
            });
        }
        Err(SenderError::Protocol("too many interim responses".into()))
    }

    /// Send a request and read the answer.
    pub fn exchange(&mut self, req: &Request) -> Result<Response> {
        self.send_request(req)?;
        self.read_response(req.method == "HEAD")
    }

    /// Server side: read one request. `max_body` caps the body.
    pub fn read_request(&mut self, max_body: usize, timeout: Duration) -> Result<Request> {
        self.check_usable()?;
        let r = self.read_request_inner(max_body, timeout);
        self.track(r)
    }

    fn read_request_inner(&mut self, max_body: usize, timeout: Duration) -> Result<Request> {
        let deadline = Instant::now() + timeout;
        let lines = self.read_head(deadline)?;
        let start = lines.first().map(String::as_str).unwrap_or("");
        let mut parts = start.split(' ');
        let (method, path, version) = (
            parts.next().unwrap_or(""),
            parts.next().unwrap_or(""),
            parts.next().unwrap_or(""),
        );
        if method.is_empty()
            || path.is_empty()
            || !(version.starts_with("HTTP/1.") || version == "RTSP/1.0")
            || parts.next().is_some()
        {
            return Err(SenderError::Protocol("bad request line".into()));
        }
        let headers = Self::read_headers(&lines)?;
        self.rtsp = version == "RTSP/1.0";
        self.peer_cseq = find(&headers, "cseq").map(|v| v.chars().take(16).collect());
        let body = match Self::content_length(&headers, max_body)? {
            Some(n) => self.read_exact_body(n, deadline)?,
            None => Vec::new(),
        };
        if find(&headers, "connection").is_some_and(|v| v.eq_ignore_ascii_case("close")) {
            self.closed = true;
        }
        Ok(Request {
            method: method.into(),
            path: path.into(),
            headers,
            body,
        })
    }

    /// Server side: write a response.
    pub fn write_response(
        &mut self,
        status: u16,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<()> {
        self.write_response_with(status, headers, body, true)
    }

    /// Server side: like [`Conn::write_response`]; with `send_body` false only the head goes
    /// out (answer to HEAD), `Content-Length` still names the body length.
    pub fn write_response_with(
        &mut self,
        status: u16,
        headers: &[(&str, String)],
        body: &[u8],
        send_body: bool,
    ) -> Result<()> {
        let version = if self.rtsp { "RTSP/1.0" } else { "HTTP/1.1" };
        let mut head = format!("{version} {status} {}\r\n", reason(status));
        if let (true, Some(c)) = (self.rtsp, &self.peer_cseq) {
            head.push_str(&format!("CSeq: {c}\r\n"));
        }
        for (k, v) in headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        head.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
        // A big body is written from the caller's slice, not copied behind the head (a
        // segment can be megabytes and several clients may fetch at once). A small one goes
        // out in one write so that the head does not wait for the body (Nagle).
        if send_body && body.len() <= 16 * 1024 {
            let mut wire = head.into_bytes();
            wire.extend_from_slice(body);
            self.io.write_all(&wire)?;
        } else {
            self.io.write_all(head.as_bytes())?;
            if send_body {
                self.io.write_all(body)?;
            }
        }
        self.io.flush()?;
        Ok(())
    }
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Reads from a fixed buffer, discards writes.
    struct Fixed(Cursor<Vec<u8>>);
    impl Read for Fixed {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            self.0.read(b)
        }
    }
    impl Write for Fixed {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn conn(data: &[u8]) -> Conn<Fixed> {
        Conn::new(Fixed(Cursor::new(data.to_vec())))
    }

    #[test]
    fn a_connection_is_discarded_after_an_io_error() {
        let mut c = conn(b"");
        let first = c.exchange(&Request::new("GET", "/a")).unwrap_err();
        assert!(first.to_string().contains("closed"), "{first}");
        let second = c.exchange(&Request::new("GET", "/a")).unwrap_err();
        assert!(second.to_string().contains("reconnect"), "{second}");
        // Bad input from our own caller does not poison the connection.
        let mut c = conn(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        assert!(c.send_request(&Request::new("GET", "/a b")).is_err());
        assert_eq!(c.exchange(&Request::new("GET", "/a")).unwrap().status, 200);
    }

    #[test]
    fn encryption_is_refused_while_plaintext_is_buffered() {
        use crate::framed::Cipher;
        let cipher = || Cipher::new(&[1; 32], &[2; 32]);
        // Bytes that follow the last plaintext answer must not become "encrypted" input.
        let mut c = conn(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\nFORGED");
        c.read_response(false).unwrap();
        assert!(c.enable_encryption(cipher()).is_err());
        assert!(!c.framed_ref().is_encrypted());
        // Nothing left over: fine.
        let mut c = conn(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        c.read_response(false).unwrap();
        c.enable_encryption(cipher()).unwrap();
        assert!(c.framed_ref().is_encrypted());
    }

    #[test]
    fn parses_content_length_and_chunked() {
        let mut c = conn(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabcHTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nhi\r\n3;x=1\r\nyou\r\n0\r\n\r\n");
        assert_eq!(c.read_response(false).unwrap().body, b"abc");
        assert_eq!(c.read_response(false).unwrap().body, b"hiyou");
    }

    #[test]
    fn skips_interim_responses() {
        let mut c = conn(b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 204 No Content\r\n\r\n");
        assert_eq!(c.read_response(false).unwrap().status, 204);
    }

    #[test]
    fn hostile_responses_are_errors() {
        let huge = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY_BYTES + 1
        );
        assert!(matches!(
            conn(huge.as_bytes()).read_response(false),
            Err(SenderError::TooLarge(_))
        ));
        assert!(
            conn(b"SIP/2.0 200 OK\r\n\r\n")
                .read_response(false)
                .is_err()
        );
        assert!(conn(b"HTTP/1.1 abc\r\n\r\n").read_response(false).is_err());
        assert!(
            conn(b"HTTP/1.1 200 OK\r\nbroken\r\n\r\n")
                .read_response(false)
                .is_err()
        );
        assert!(
            conn(b"HTTP/1.1 200 OK\r\nContent-Length: -1\r\n\r\n")
                .read_response(false)
                .is_err()
        );
        assert!(
            conn(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nab")
                .read_response(false)
                .is_err()
        );
        assert!(
            conn(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n")
                .read_response(false)
                .is_err()
        );
        let long_head = format!(
            "HTTP/1.1 200 OK\r\nX: {}\r\n\r\n",
            "a".repeat(MAX_HEAD_BYTES)
        );
        assert!(matches!(
            conn(long_head.as_bytes()).read_response(false),
            Err(SenderError::TooLarge(_))
        ));
        let mut many = String::from("HTTP/1.1 200 OK\r\n");
        for i in 0..200 {
            many.push_str(&format!("H{i}: v\r\n"));
        }
        many.push_str("\r\n");
        assert!(conn(many.as_bytes()).read_response(false).is_err());
        assert!(conn(b"").read_response(false).is_err());
    }

    #[test]
    fn rtsp_mode_adds_cseq_and_answers_in_kind() {
        let mut c = conn(b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nContent-Length: 0\r\n\r\n");
        c.set_rtsp(true);
        assert_eq!(
            c.exchange(&Request::new("GET", "/info")).unwrap().status,
            200
        );
        let mut s = conn(b"POST /x RTSP/1.0\r\nCSeq: 7\r\nContent-Length: 0\r\n\r\n");
        let r = s.read_request(10, Duration::from_secs(1)).unwrap();
        assert_eq!(r.path, "/x");
        assert!(s.is_rtsp());
    }

    #[test]
    fn no_length_reads_to_end_only_for_http_1_0() {
        let mut c = conn(b"HTTP/1.0 200 OK\r\n\r\nhello");
        assert_eq!(c.read_response(false).unwrap().body, b"hello");
        let mut c = conn(b"RTSP/1.0 200 OK\r\nCSeq: 2\r\n\r\nHTTP/1.1 200 OK\r\n\r\n");
        assert!(c.read_response(false).unwrap().body.is_empty());
        assert!(c.read_response(false).unwrap().body.is_empty());
    }

    #[test]
    fn request_reading_checks_everything() {
        let mut c = conn(b"POST /x HTTP/1.1\r\nContent-Length: 2\r\n\r\nhi");
        let r = c.read_request(10, Duration::from_secs(1)).unwrap();
        assert_eq!(
            (r.method.as_str(), r.path.as_str(), r.body.as_slice()),
            ("POST", "/x", &b"hi"[..])
        );
        assert!(
            conn(b"GARBAGE\r\n\r\n")
                .read_request(10, Duration::from_secs(1))
                .is_err()
        );
        assert!(
            conn(b"POST /x HTTP/1.1\r\nContent-Length: 20\r\n\r\n")
                .read_request(10, Duration::from_secs(1))
                .is_err()
        );
    }

    #[test]
    fn sending_rejects_header_injection() {
        let mut c = conn(b"");
        assert!(c.send_request(&Request::new("GET", "/a b")).is_err());
        assert!(
            c.send_request(&Request::new("GET", "/").header("X", "a\r\nY: b"))
                .is_err()
        );
        assert!(
            c.send_request(&Request::new("GET", "/").header("X", "ok"))
                .is_ok()
        );
    }
}
