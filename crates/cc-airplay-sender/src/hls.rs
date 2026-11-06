// SPDX-License-Identifier: GPL-3.0-or-later
//! A small HLS live server on plain HTTP (LAN only). It holds the newest few MPEG-TS
//! segments in memory and serves `/live.m3u8` and `/seg<N>.ts` (RFC 8216, version 3, no
//! encryption, no partial segments).
//!
//! Safety: the server answers only GET and HEAD, only to the addresses in the allow list
//! (the receiver), at most [`MAX_CONNECTIONS`] connections at a time, with timeouts and size
//! limits on everything it reads. Segments never leave memory (no files).
//!
//! UNCERTAIN: whether a given receiver (Apple TV, TV set) accepts plain `http://` media
//! links from a LAN address (App Transport Security applies to apps, not to AirPlay
//! receivers, as far as known, but this was not verified).

use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::error::{Result, SenderError};
use crate::http::Conn;

/// Most simultaneous connections.
pub const MAX_CONNECTIONS: usize = 16;
/// Largest single segment we keep.
pub const MAX_SEGMENT_BYTES: usize = 16 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

struct Segment {
    seq: u64,
    duration: f64,
    data: Arc<Vec<u8>>,
}

#[derive(Default)]
struct Inner {
    segments: VecDeque<Segment>,
    next_seq: u64,
    ended: bool,
}

/// The newest segments of a live stream.
pub struct SegmentStore {
    window: usize,
    inner: Mutex<Inner>,
}

impl SegmentStore {
    /// Keep the newest `window` segments (at least 2).
    pub fn new(window: usize) -> Arc<Self> {
        Arc::new(Self {
            window: window.max(2),
            inner: Mutex::new(Inner::default()),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // A poisoned lock only means another thread panicked; the data is still usable.
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Add a finished segment of `duration` seconds.
    pub fn push(&self, duration: f64, data: Vec<u8>) -> Result<()> {
        if data.len() > MAX_SEGMENT_BYTES {
            return Err(SenderError::TooLarge("video segment"));
        }
        let duration = if duration.is_finite() {
            duration.clamp(0.1, 60.0)
        } else {
            1.0
        };
        let mut g = self.lock();
        let seq = g.next_seq;
        g.next_seq += 1;
        g.segments.push_back(Segment {
            seq,
            duration,
            data: Arc::new(data),
        });
        while g.segments.len() > self.window {
            g.segments.pop_front();
        }
        Ok(())
    }

    /// Mark the stream as finished (`#EXT-X-ENDLIST`).
    pub fn end(&self) {
        self.lock().ended = true;
    }

    /// Number of segments currently held.
    pub fn len(&self) -> usize {
        self.lock().segments.len()
    }

    /// Whether no segment is held.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Total number of segments ever added.
    pub fn total(&self) -> u64 {
        self.lock().next_seq
    }

    /// The live playlist.
    pub fn playlist(&self) -> String {
        let g = self.lock();
        let target = g
            .segments
            .iter()
            .map(|s| s.duration.ceil() as u64)
            .max()
            .unwrap_or(1)
            .max(1);
        let first = g.segments.front().map_or(0, |s| s.seq);
        let mut out = format!(
            "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:{target}\n#EXT-X-MEDIA-SEQUENCE:{first}\n"
        );
        for s in &g.segments {
            out.push_str(&format!("#EXTINF:{:.3},\nseg{}.ts\n", s.duration, s.seq));
        }
        if g.ended {
            out.push_str("#EXT-X-ENDLIST\n");
        }
        out
    }

    /// A segment by sequence number, if still held.
    pub fn segment(&self, seq: u64) -> Option<Arc<Vec<u8>>> {
        self.lock()
            .segments
            .iter()
            .find(|s| s.seq == seq)
            .map(|s| Arc::clone(&s.data))
    }
}

/// The running server.
pub struct HlsServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

fn normalise(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        v4 => v4,
    }
}

impl HlsServer {
    /// Listen on `bind` (port 0 picks a free port). With `allowed`, only those client
    /// addresses get answers; `None` allows every client (UNCERTAIN: not recommended).
    pub fn start(
        bind: SocketAddr,
        store: Arc<SegmentStore>,
        allowed: Option<Vec<IpAddr>>,
    ) -> Result<Self> {
        let listener = TcpListener::bind(bind)?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let active = Arc::new(AtomicUsize::new(0));
        let allowed: Option<Vec<IpAddr>> = allowed.map(|v| v.into_iter().map(normalise).collect());
        let stop2 = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("cc-airplay-hls".into())
            .spawn(move || {
                while !stop2.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, peer)) => {
                            let ok = allowed
                                .as_ref()
                                .is_none_or(|a| a.contains(&normalise(peer.ip())));
                            if !ok || active.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
                                drop(stream);
                                continue;
                            }
                            active.fetch_add(1, Ordering::Relaxed);
                            let store = Arc::clone(&store);
                            let active2 = Arc::clone(&active);
                            let stop3 = Arc::clone(&stop2);
                            let spawned = std::thread::Builder::new()
                                .name("cc-airplay-hls-conn".into())
                                .spawn(move || {
                                    let _ = serve_connection(stream, &store, &stop3);
                                    active2.fetch_sub(1, Ordering::Relaxed);
                                });
                            if spawned.is_err() {
                                active.fetch_sub(1, Ordering::Relaxed);
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(20));
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(100)),
                    }
                }
            })
            .map_err(|e| SenderError::Io(e.to_string()))?;
        Ok(Self {
            addr,
            stop,
            thread: Some(thread),
        })
    }

    /// The address the server listens on.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// The playlist URL as the receiver should fetch it, given our address towards it.
    pub fn playlist_url(&self, host: IpAddr) -> String {
        match host {
            IpAddr::V4(v4) => format!("http://{v4}:{}/live.m3u8", self.addr.port()),
            IpAddr::V6(v6) => format!("http://[{v6}]:{}/live.m3u8", self.addr.port()),
        }
    }

    /// Stop accepting and wait for the accept thread.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for HlsServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn serve_connection(stream: TcpStream, store: &SegmentStore, stop: &AtomicBool) -> Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(REQUEST_TIMEOUT))?;
    stream.set_write_timeout(Some(REQUEST_TIMEOUT))?;
    let mut conn = Conn::new(stream);
    {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let req = conn.read_request(1024, REQUEST_TIMEOUT)?;
        let head = req.method == "HEAD";
        if req.method != "GET" && !head {
            conn.write_response(
                405,
                &[
                    ("Allow", "GET, HEAD".into()),
                    ("Connection", "close".into()),
                ],
                b"",
            )?;
            return Ok(());
        }
        let path = req.path.split('?').next().unwrap_or("");
        let (mut status, ctype, cache, body): (u16, &str, &str, Arc<Vec<u8>>) =
            if path == "/live.m3u8" {
                (
                    200,
                    "application/vnd.apple.mpegurl",
                    "no-cache",
                    Arc::new(store.playlist().into_bytes()),
                )
            } else if let Some(seq) = path
                .strip_prefix("/seg")
                .and_then(|s| s.strip_suffix(".ts"))
                .and_then(|s| s.parse::<u64>().ok())
            {
                // The segment is shared with the store: no copy per request.
                match store.segment(seq) {
                    Some(data) => (200, "video/mp2t", "max-age=60", data),
                    None => (404, "text/plain", "no-cache", Arc::new(b"gone".to_vec())),
                }
            } else {
                (
                    404,
                    "text/plain",
                    "no-cache",
                    Arc::new(b"not found".to_vec()),
                )
            };
        // One `Range: bytes=...` on a segment is answered with 206 (receivers may ask).
        let mut extra: Vec<(&str, String)> = Vec::new();
        let mut slice: &[u8] = &body;
        if status == 200 && ctype == "video/mp2t" {
            extra.push(("Accept-Ranges", "bytes".into()));
            if let Some(range) = req.get("Range") {
                match parse_range(range, body.len()) {
                    Some(Ok((a, b))) => {
                        status = 206;
                        slice = &body[a..=b];
                        extra.push(("Content-Range", format!("bytes {a}-{b}/{}", body.len())));
                    }
                    Some(Err(())) => {
                        status = 416;
                        slice = b"";
                        extra.push(("Content-Range", format!("bytes */{}", body.len())));
                    }
                    None => {}
                }
            }
        }
        // `Connection: close`: one request per connection, so that a client cannot hold a
        // thread (and a segment) for long. No CORS header: nothing needs it.
        let mut headers = vec![
            ("Content-Type", ctype.to_owned()),
            ("Cache-Control", cache.to_owned()),
            ("Connection", "close".to_owned()),
        ];
        headers.extend(extra);
        conn.write_response_with(status, &headers, slice, !head)?;
    }
    Ok(())
}

/// Parse a single `bytes=` range for a body of `len` bytes. `None`: ignore the header (not
/// a byte range, or several ranges: the whole body is sent); `Some(Err)`: not satisfiable.
fn parse_range(value: &str, len: usize) -> Option<std::result::Result<(usize, usize), ()>> {
    let spec = value.trim().strip_prefix("bytes=")?;
    if spec.contains(',') || len == 0 {
        return None;
    }
    let (from, to) = spec.split_once('-')?;
    let (from, to) = (from.trim(), to.trim());
    let (a, b) = if from.is_empty() {
        // Suffix: the last N bytes.
        let n: usize = to.parse().ok()?;
        if n == 0 {
            return Some(Err(()));
        }
        (len.saturating_sub(n), len - 1)
    } else {
        let a: usize = from.parse().ok()?;
        let b: usize = if to.is_empty() {
            len - 1
        } else {
            to.parse::<usize>().ok()?.min(len - 1)
        };
        (a, b)
    };
    if a >= len || a > b {
        return Some(Err(()));
    }
    Some(Ok((a, b)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::Request;
    use std::io::{Read, Write};

    fn get(addr: SocketAddr, path: &str) -> (u16, Vec<u8>) {
        let s = TcpStream::connect(addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut c = Conn::new(s);
        let r = c.exchange(&Request::new("GET", path)).unwrap();
        (r.status, r.body)
    }

    #[test]
    fn playlist_window_and_numbering() {
        let store = SegmentStore::new(3);
        assert!(store.playlist().contains("MEDIA-SEQUENCE:0"));
        for i in 0..5u8 {
            store.push(1.0, vec![i; 10]).unwrap();
        }
        let p = store.playlist();
        assert!(p.contains("#EXT-X-MEDIA-SEQUENCE:2"), "{p}");
        assert!(p.contains("seg4.ts") && !p.contains("seg1.ts"));
        assert!(p.contains("#EXT-X-TARGETDURATION:1"));
        assert!(!p.contains("ENDLIST"));
        store.end();
        assert!(store.playlist().contains("#EXT-X-ENDLIST"));
        assert!(store.segment(1).is_none());
        assert_eq!(store.segment(4).unwrap().as_slice(), &[4u8; 10]);
        assert!(store.push(1.0, vec![0; MAX_SEGMENT_BYTES + 1]).is_err());
        assert_eq!(store.total(), 5);
    }

    #[test]
    fn serves_playlist_and_segments_and_rejects_others() {
        let store = SegmentStore::new(4);
        store.push(1.2, vec![7; 1000]).unwrap();
        let mut server = HlsServer::start(
            "127.0.0.1:0".parse().unwrap(),
            Arc::clone(&store),
            Some(vec!["127.0.0.1".parse().unwrap()]),
        )
        .unwrap();
        let addr = server.local_addr();
        let (st, body) = get(addr, "/live.m3u8");
        assert_eq!(st, 200);
        assert!(String::from_utf8(body).unwrap().contains("#EXTINF:1.200,"));
        let (st, body) = get(addr, "/seg0.ts");
        assert_eq!((st, body.len()), (200, 1000));
        assert_eq!(get(addr, "/seg9.ts").0, 404);
        assert_eq!(get(addr, "/etc/passwd").0, 404);
        assert_eq!(get(addr, "/seg-1.ts").0, 404);

        // Wrong method.
        let s = TcpStream::connect(addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut c = Conn::new(s);
        let r = c.exchange(&Request::new("POST", "/live.m3u8")).unwrap();
        assert_eq!(r.status, 405);

        // Garbage and oversize input must not hang or crash the server.
        let mut s = TcpStream::connect(addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s.write_all(&vec![b'A'; 40_000]).unwrap();
        let mut sink = Vec::new();
        let _ = s.read_to_end(&mut sink);
        assert_eq!(get(addr, "/live.m3u8").0, 200);
        server.stop();
    }

    #[test]
    fn range_requests_close_and_no_cors() {
        let store = SegmentStore::new(4);
        store.push(1.0, (0..100u8).collect::<Vec<u8>>()).unwrap();
        let mut server = HlsServer::start(
            "127.0.0.1:0".parse().unwrap(),
            Arc::clone(&store),
            Some(vec!["127.0.0.1".parse().unwrap()]),
        )
        .unwrap();
        let addr = server.local_addr();
        let fetch = |range: Option<&str>| {
            let s = TcpStream::connect(addr).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut c = Conn::new(s);
            let mut req = Request::new("GET", "/seg0.ts");
            if let Some(r) = range {
                req = req.header("Range", r);
            }
            c.exchange(&req).unwrap()
        };
        let r = fetch(None);
        assert_eq!((r.status, r.body.len()), (200, 100));
        assert_eq!(r.get("Connection"), Some("close"));
        assert_eq!(r.get("Access-Control-Allow-Origin"), None);
        let r = fetch(Some("bytes=10-19"));
        assert_eq!(r.status, 206);
        assert_eq!(r.body, (10..20u8).collect::<Vec<u8>>());
        assert_eq!(r.get("Content-Range"), Some("bytes 10-19/100"));
        let r = fetch(Some("bytes=-5"));
        assert_eq!((r.status, r.body), (206, (95..100u8).collect::<Vec<u8>>()));
        assert_eq!(fetch(Some("bytes=100-")).status, 416);
        assert_eq!(fetch(Some("bytes=5-2")).status, 416);
        // Several ranges, or garbage: the whole segment.
        assert_eq!(fetch(Some("bytes=0-1,5-6")).body.len(), 100);
        assert_eq!(fetch(Some("lines=1-2")).body.len(), 100);
        server.stop();
    }

    #[test]
    fn range_parsing_edges() {
        assert_eq!(parse_range("bytes=0-0", 10), Some(Ok((0, 0))));
        assert_eq!(parse_range("bytes=5-", 10), Some(Ok((5, 9))));
        assert_eq!(parse_range("bytes=5-999", 10), Some(Ok((5, 9))));
        assert_eq!(parse_range("bytes=-999", 10), Some(Ok((0, 9))));
        assert_eq!(parse_range("bytes=-0", 10), Some(Err(())));
        assert_eq!(parse_range("bytes=10-", 10), Some(Err(())));
        assert_eq!(parse_range("bytes=a-b", 10), None);
        assert_eq!(parse_range("bytes=0-1", 0), None);
    }

    #[test]
    fn clients_not_on_the_list_get_nothing() {
        let store = SegmentStore::new(2);
        let mut server = HlsServer::start(
            "127.0.0.1:0".parse().unwrap(),
            store,
            Some(vec!["10.9.9.9".parse().unwrap()]),
        )
        .unwrap();
        let s = TcpStream::connect(server.local_addr()).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut c = Conn::new(s);
        assert!(c.exchange(&Request::new("GET", "/live.m3u8")).is_err());
        server.stop();
    }

    #[test]
    fn url_shapes() {
        let store = SegmentStore::new(2);
        let mut server = HlsServer::start("127.0.0.1:0".parse().unwrap(), store, None).unwrap();
        let port = server.local_addr().port();
        assert_eq!(
            server.playlist_url("192.168.1.5".parse().unwrap()),
            format!("http://192.168.1.5:{port}/live.m3u8")
        );
        assert!(
            server
                .playlist_url("fe80::1".parse().unwrap())
                .contains("[fe80::1]")
        );
        server.stop();
    }
}
