// SPDX-License-Identifier: GPL-3.0-or-later
//! One control connection to a receiver: connect, authenticate, then play a media link, poll
//! the playback state, scrub, stop and show a photo.
//!
//! The requests are the HTTP ones of the public (unofficial) AirPlay video description,
//! as pyatv's `airplayv1.py`/`player.py` (MIT) use them: `POST /play` with a binary plist
//! (`Content-Location`, `Start-Position`), `GET /playback-info`, `GET`/`POST /scrub`,
//! `POST /rate`, `POST /stop`, `PUT /photo`.
//!
//! UNCERTAIN: AirPlay 2 receivers (Apple TV with current tvOS, HomePod, many TVs) want the
//! RTSP-based media session (SETUP, event channel, timing, RECORD, then `/play`, see pyatv
//! `airplayv2.py`). That session is NOT implemented here. Whether such a receiver accepts
//! the plain HTTP requests below after pairing is unknown; no such receiver was available.

use std::net::{IpAddr, SocketAddr, TcpStream, UdpSocket};
use std::time::Duration;

use plist::{Dictionary, Value};

use crate::creds::Credentials;
use crate::cryptoutil::{hex, random};
use crate::discovery::{Device, feature};
use crate::error::{Result, SenderError, clean};
use crate::http::{Conn, Request, Response};
use crate::{hap, legacy, plistx};

/// Timeout for connecting.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Timeout for one read or write on the control connection.
pub const IO_TIMEOUT: Duration = Duration::from_secs(10);
/// Longest media URL we send.
pub const MAX_URL_LEN: usize = 2048;
/// Biggest photo we send.
pub const MAX_PHOTO_BYTES: usize = 20 * 1024 * 1024;

/// Timeout for the dialect probe.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Which request format the control port speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proto {
    /// Try HTTP/1.1, then RTSP/1.0.
    Auto,
    /// `HTTP/1.1` requests (what pyatv sends for pairing and `/play`).
    Http,
    /// `RTSP/1.0` requests with `CSeq` (what UxPlay answers; doubletake uses it for all).
    Rtsp,
}

/// What `authenticate` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    /// Nothing was sent (no credentials and the receiver does not advertise pairing).
    None,
    /// Legacy pair-verify with stored credentials.
    LegacyVerify,
    /// HomeKit-style pair-verify with stored credentials (encrypted channel on).
    HapVerify,
    /// Transient pairing (encrypted channel on).
    HapTransient,
}

/// A parsed `/playback-info` answer. All fields are what the receiver said.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaybackInfo {
    /// Length in seconds; only present when something is loaded.
    pub duration: Option<f64>,
    /// Position in seconds.
    pub position: Option<f64>,
    /// Playback rate (0 paused, 1 playing).
    pub rate: Option<f64>,
    /// Receiver says it can play.
    pub ready_to_play: Option<bool>,
    /// Error text if the receiver reported one (`code`, `domain`).
    pub error: Option<String>,
}

/// A control connection.
pub struct Session {
    conn: Conn<TcpStream>,
    device: Device,
    session_id: String,
    local_ip: IpAddr,
}

fn check_url(url: &str) -> Result<()> {
    if url.len() > MAX_URL_LEN || url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(SenderError::Input(
            "the URL is too long or has spaces".into(),
        ));
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(SenderError::Input(
            "only http:// and https:// links work".into(),
        ));
    }
    Ok(())
}

/// The local address that would be used to reach `remote` (no packet is sent).
pub fn local_ip_for(remote: IpAddr) -> Result<IpAddr> {
    let bind: SocketAddr = if remote.is_ipv4() {
        "0.0.0.0:0"
            .parse()
            .map_err(|_| SenderError::Io("bind".into()))?
    } else {
        "[::]:0"
            .parse()
            .map_err(|_| SenderError::Io("bind".into()))?
    };
    let sock = UdpSocket::bind(bind)?;
    sock.connect(SocketAddr::new(remote, 9))?;
    Ok(sock.local_addr()?.ip())
}

impl Session {
    /// Connect to the device's control port and find out which dialect it speaks
    /// ([`Proto::Auto`]).
    pub fn connect(device: &Device) -> Result<Self> {
        Self::connect_with(device, Proto::Auto)
    }

    /// Connect with a chosen dialect. `Auto` first tries `HTTP/1.1` (a probe `GET /info`
    /// with a short timeout), then `RTSP/1.0`: UxPlay 1.68 answers only the latter.
    pub fn connect_with(device: &Device, proto: Proto) -> Result<Self> {
        match proto {
            Proto::Http => Self::open(device, false, IO_TIMEOUT),
            Proto::Rtsp => Self::open(device, true, IO_TIMEOUT),
            Proto::Auto => {
                let first = Self::open(device, false, PROBE_TIMEOUT).and_then(|mut s| {
                    s.probe()?;
                    Ok(s)
                });
                match first {
                    Ok(s) => Ok(s),
                    Err(http_err) => {
                        let second = Self::open(device, true, PROBE_TIMEOUT).and_then(|mut s| {
                            s.probe()?;
                            Ok(s)
                        });
                        second.map_err(|rtsp_err| {
                            SenderError::Protocol(format!(
                                "the receiver answers neither HTTP/1.1 ({http_err}) nor RTSP/1.0 ({rtsp_err})"
                            ))
                        })
                    }
                }
            }
        }
    }

    /// Any answer at all to `GET /info` proves the dialect; the status does not matter.
    fn probe(&mut self) -> Result<()> {
        self.conn.exchange(&self.media_request("GET", "/info"))?;
        self.set_timeouts(IO_TIMEOUT)
    }

    fn set_timeouts(&self, t: Duration) -> Result<()> {
        let s = self.conn.framed_ref().get_ref();
        s.set_read_timeout(Some(t))?;
        s.set_write_timeout(Some(t))?;
        Ok(())
    }

    fn open(device: &Device, rtsp: bool, timeout: Duration) -> Result<Self> {
        let addr = device
            .socket_addr()
            .ok_or_else(|| SenderError::Input("the device has no address".into()))?;
        let stream = match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
            Ok(s) => s,
            // Fallback for UxPlay-like receivers whose `_airplay._tcp` port is not the
            // listening one: the `_raop._tcp` port of the same device.
            Err(e) => match (e.kind(), device.raop_port, device.addrs.first()) {
                (std::io::ErrorKind::ConnectionRefused, Some(p), Some(ip)) => {
                    TcpStream::connect_timeout(&SocketAddr::new(*ip, p), CONNECT_TIMEOUT)?
                }
                _ => return Err(e.into()),
            },
        };
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        stream.set_nodelay(true)?;
        let local_ip = stream.local_addr()?.ip();
        let id: [u8; 16] = random();
        let mut conn = Conn::new(stream);
        conn.set_rtsp(rtsp);
        Ok(Self {
            conn,
            device: device.clone(),
            session_id: format!(
                "{}-{}-{}-{}-{}",
                hex(&id[0..4]),
                hex(&id[4..6]),
                hex(&id[6..8]),
                hex(&id[8..10]),
                hex(&id[10..16])
            ),
            local_ip,
        })
    }

    /// Which dialect this connection speaks.
    pub fn proto(&self) -> Proto {
        if self.conn.is_rtsp() {
            Proto::Rtsp
        } else {
            Proto::Http
        }
    }

    /// Our address on the connection to the receiver (where our HLS server must listen).
    pub fn local_ip(&self) -> IpAddr {
        self.local_ip
    }

    /// The device this session talks to.
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// The underlying HTTP connection, for the pairing functions.
    pub fn conn_mut(&mut self) -> &mut Conn<TcpStream> {
        &mut self.conn
    }

    fn media_request(&self, method: &str, path: &str) -> Request {
        Request::new(method, path)
            .header("User-Agent", "MediaControl/1.0")
            .header("X-Apple-Session-ID", self.session_id.clone())
    }

    fn ok(&mut self, req: Request) -> Result<Response> {
        let what = format!(
            "{} {}",
            req.method,
            req.path.split('?').next().unwrap_or("")
        );
        let resp = self.conn.exchange(&req)?;
        if resp.status != 200 {
            return Err(SenderError::Status {
                status: resp.status,
                what,
            });
        }
        Ok(resp)
    }

    /// `GET /server-info`: the receiver's own description as a property list.
    pub fn server_info(&mut self) -> Result<Dictionary> {
        let resp = self.ok(self.media_request("GET", "/server-info"))?;
        plistx::decode_dict(&resp.body)
    }

    /// Prove that we may talk to the receiver.
    ///
    /// With stored credentials: pair-verify of their kind. Without: transient pairing if the
    /// receiver advertises system or CoreUtils pairing, else nothing.
    pub fn authenticate(&mut self, creds: Option<&Credentials>) -> Result<AuthKind> {
        match creds {
            Some(c @ Credentials::Legacy { .. }) => {
                legacy::pair_verify(&mut self.conn, c)?;
                Ok(AuthKind::LegacyVerify)
            }
            Some(c @ Credentials::Hap { .. }) => {
                hap::pair_verify(&mut self.conn, c)?;
                Ok(AuthKind::HapVerify)
            }
            None => {
                if self.device.has_feature(feature::SYSTEM_PAIRING)
                    || self.device.has_feature(feature::CORE_UTILS_PAIRING)
                {
                    hap::pair_transient(&mut self.conn)?;
                    Ok(AuthKind::HapTransient)
                } else {
                    Ok(AuthKind::None)
                }
            }
        }
    }

    /// `POST /play`: ask the receiver to play a media link from the start.
    pub fn play(&mut self, url: &str) -> Result<()> {
        check_url(url)?;
        let mut d = Dictionary::new();
        d.insert("Content-Location".into(), Value::String(url.into()));
        d.insert("Start-Position".into(), Value::Real(0.0));
        d.insert(
            "X-Apple-Session-ID".into(),
            Value::String(self.session_id.clone()),
        );
        let req = self
            .media_request("POST", "/play")
            .body(plistx::BPLIST_TYPE, plistx::encode(d)?);
        self.ok(req)?;
        Ok(())
    }

    /// `GET /playback-info`.
    pub fn playback_info(&mut self) -> Result<PlaybackInfo> {
        let resp = self.ok(self.media_request("GET", "/playback-info"))?;
        if resp.body.is_empty() {
            return Ok(PlaybackInfo::default());
        }
        let d = plistx::decode_dict(&resp.body)?;
        let error = match d.get("error") {
            Some(Value::Dictionary(e)) => {
                let code = plistx::number(e, "code").map(|c| c as i64);
                let domain = match e.get("domain") {
                    Some(Value::String(s)) => clean(s, 64),
                    _ => "?".into(),
                };
                Some(format!("code {code:?} in {domain}"))
            }
            Some(_) => Some("unreadable error".into()),
            None => None,
        };
        Ok(PlaybackInfo {
            duration: plistx::number(&d, "duration"),
            position: plistx::number(&d, "position"),
            rate: plistx::number(&d, "rate"),
            ready_to_play: match d.get("readyToPlay") {
                Some(Value::Boolean(b)) => Some(*b),
                _ => None,
            },
            error,
        })
    }

    /// `POST /scrub?position=SECONDS`.
    pub fn scrub(&mut self, seconds: f64) -> Result<()> {
        if !(0.0..=1.0e7).contains(&seconds) {
            return Err(SenderError::Input("position out of range".into()));
        }
        self.ok(self.media_request("POST", &format!("/scrub?position={seconds:.3}")))?;
        Ok(())
    }

    /// `GET /scrub`: returns `(duration, position)` in seconds if the receiver says them.
    pub fn scrub_position(&mut self) -> Result<(Option<f64>, Option<f64>)> {
        let resp = self.ok(self.media_request("GET", "/scrub"))?;
        Ok(parse_scrub(&String::from_utf8_lossy(&resp.body)))
    }

    /// `POST /rate?value=R` (0 pauses, 1 plays).
    pub fn rate(&mut self, value: f64) -> Result<()> {
        if !value.is_finite() || !(0.0..=16.0).contains(&value) {
            return Err(SenderError::Input("rate out of range".into()));
        }
        self.ok(self.media_request("POST", &format!("/rate?value={value:.6}")))?;
        Ok(())
    }

    /// `POST /stop`.
    pub fn stop(&mut self) -> Result<()> {
        self.ok(self.media_request("POST", "/stop"))?;
        Ok(())
    }

    /// `PUT /photo` with a JPEG.
    pub fn photo(&mut self, jpeg: Vec<u8>) -> Result<()> {
        check_jpeg(&jpeg)?;
        let req = self
            .media_request("PUT", "/photo")
            .header("X-Apple-AssetKey", self.session_id.clone())
            .header("X-Apple-Transition", "None")
            .body("image/jpeg", jpeg);
        self.ok(req)?;
        Ok(())
    }
}

/// Check that bytes look like a JPEG of acceptable size.
pub fn check_jpeg(data: &[u8]) -> Result<()> {
    if data.len() > MAX_PHOTO_BYTES {
        return Err(SenderError::TooLarge("photo"));
    }
    if data.len() < 4 || data[..3] != [0xFF, 0xD8, 0xFF] {
        return Err(SenderError::Input(
            "the photo must be a JPEG file (other formats are not converted)".into(),
        ));
    }
    Ok(())
}

/// Parse the text body of `GET /scrub` (`duration: 83.12` and `position: 14.4` lines).
pub fn parse_scrub(text: &str) -> (Option<f64>, Option<f64>) {
    let mut duration = None;
    let mut position = None;
    for line in text.lines().take(16) {
        if let Some((k, v)) = line.split_once(':') {
            let n = v.trim().parse::<f64>().ok().filter(|n| n.is_finite());
            match k.trim() {
                "duration" => duration = n,
                "position" => position = n,
                _ => {}
            }
        }
    }
    (duration, position)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_checks() {
        assert!(check_url("http://10.0.0.2:8080/live.m3u8").is_ok());
        assert!(check_url("https://example.org/a.mp4").is_ok());
        assert!(check_url("file:///etc/passwd").is_err());
        assert!(check_url("http://a/b c").is_err());
        assert!(check_url("http://a/\r\nX: y").is_err());
        assert!(check_url(&format!("http://a/{}", "x".repeat(MAX_URL_LEN))).is_err());
    }

    #[test]
    fn jpeg_checks() {
        assert!(check_jpeg(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0]).is_ok());
        assert!(check_jpeg(b"\x89PNG\r\n\x1a\n").is_err());
        assert!(check_jpeg(&[]).is_err());
    }

    #[test]
    fn scrub_text() {
        assert_eq!(
            parse_scrub("duration: 83.12\nposition: 14.4\n"),
            (Some(83.12), Some(14.4))
        );
        assert_eq!(parse_scrub("duration: nan\nposition: x"), (None, None));
        assert_eq!(parse_scrub(""), (None, None));
    }

    #[test]
    fn local_ip_for_loopback() {
        assert!(
            local_ip_for("127.0.0.1".parse().unwrap())
                .unwrap()
                .is_loopback()
        );
    }
}
