// SPDX-License-Identifier: GPL-3.0-or-later
//! Sends encrypted, packetised H.264 frames over UDP and reads the receiver's RTCP on the same
//! socket (Open Screen multiplexes RTP and RTCP on one UDP port: the receiver sends its RTCP
//! to the address the RTP came from).
//!
//! Timing constants: sender report every 500 ms (`kRtcpReportInterval` in
//! `cast/streaming/public/constants.h`, Open Screen commit
//! `8b62b21479f1adb7663f182d97962b960c5c64ef`, BSD-3-Clause, Copyright 2019 The Chromium
//! Authors). The first sender report goes out before the first RTP packet, because the
//! receiver needs its lip-sync information (`SenderImpl::EnqueueFrame` comment).
//!
//! UNCERTAIN: the H.264 data is sent as Annex-B (start codes), exactly as `cc-media` produces
//! it. Whether Google's receiver wants Annex-B or length-prefixed (AVCC) is not known
//! (plan, spike 10). No congestion control, no kickstart packets, no re-sending of frames
//! older than the last 32.
//!
//! Retransmission is bounded, because the NACKs come from the network: NACK entries per
//! datagram are capped in [`rtp::parse_rtcp`]; a packet is not resent again within
//! [`MIN_RESEND_INTERVAL`]; and all resends together are limited by a token bucket
//! ([`RESEND_BYTES_PER_SEC`], [`RESEND_PACKETS_PER_SEC`]). Resends use a non-blocking send
//! (`try_send_to`): if the socket buffer is full, the resend is dropped instead of stalling the
//! capture / sender report / heartbeat loop. Only datagrams from the exact address the RTP goes
//! to are accepted (UNCERTAIN: a receiver that sends RTCP from another port than it listens on
//! would be ignored; Open Screen answers from the RTP port).

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::net::UdpSocket;

use crate::rtp::{
    self, ALL_PACKETS_LOST, FrameMeta, MAX_PACKET_SIZE_V4, MAX_PACKET_SIZE_V6, RtcpEvent,
    RtpPacketizer,
};
use crate::session::StreamParams;

/// `kRtcpReportInterval`.
pub const SENDER_REPORT_INTERVAL: Duration = Duration::from_millis(500);
/// How many recent frames are kept for retransmission.
const RESEND_CACHE_FRAMES: usize = 32;
/// A packet that was just resent is not resent again for this long.
pub const MIN_RESEND_INTERVAL: Duration = Duration::from_millis(20);
/// Sustained retransmission budget in bytes per second (about 8 Mbit/s).
pub const RESEND_BYTES_PER_SEC: u64 = 1_000_000;
/// Sustained retransmission budget in packets per second.
pub const RESEND_PACKETS_PER_SEC: u64 = 1_000;
/// Burst the budgets allow at once (fraction of a second).
const RESEND_BURST: Duration = Duration::from_millis(250);

/// Token bucket for retransmissions (bytes and packets).
#[derive(Debug)]
struct ResendLimiter {
    bytes: f64,
    packets: f64,
    last: Instant,
}

impl ResendLimiter {
    fn new(now: Instant) -> Self {
        Self {
            bytes: RESEND_BYTES_PER_SEC as f64 * RESEND_BURST.as_secs_f64(),
            packets: RESEND_PACKETS_PER_SEC as f64 * RESEND_BURST.as_secs_f64(),
            last: now,
        }
    }

    /// Take the budget for one packet of `len` bytes, if there is enough.
    fn allow(&mut self, now: Instant, len: usize) -> bool {
        let dt = now.saturating_duration_since(self.last).as_secs_f64();
        self.last = now;
        let burst = RESEND_BURST.as_secs_f64();
        self.bytes = (self.bytes + dt * RESEND_BYTES_PER_SEC as f64)
            .min(RESEND_BYTES_PER_SEC as f64 * burst);
        self.packets = (self.packets + dt * RESEND_PACKETS_PER_SEC as f64)
            .min(RESEND_PACKETS_PER_SEC as f64 * burst);
        if self.bytes >= len as f64 && self.packets >= 1.0 {
            self.bytes -= len as f64;
            self.packets -= 1.0;
            true
        } else {
            false
        }
    }
}

/// A sent frame kept for retransmission.
struct CachedFrame {
    id: u32,
    packets: Vec<Vec<u8>>,
    /// When each packet was last resent.
    resent_at: Vec<Option<Instant>>,
}

/// The later of two RTP timestamps, comparing modulo 2^32 (so a wrap-around does not make
/// the older value win).
fn later_rtp_ts(a: u32, b: u32) -> u32 {
    if (a.wrapping_sub(b) as i32) >= 0 { a } else { b }
}

/// Counters, for the CLI and for tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StreamStats {
    /// Frames sent.
    pub frames: u64,
    /// RTP packets sent (retransmissions not counted).
    pub packets: u64,
    /// Retransmitted packets.
    pub resent_packets: u64,
    /// Retransmissions that were left out because of the rate limit or the de-duplication.
    pub resend_suppressed: u64,
    /// Picture loss indications received.
    pub picture_loss: u64,
    /// RTCP feedback messages received.
    pub feedback: u64,
}

/// The sending side of one video stream.
pub struct VideoStreamer {
    socket: UdpSocket,
    remote: SocketAddr,
    params: StreamParams,
    packetizer: RtpPacketizer,
    next_frame_id: u32,
    last_key_frame_id: u32,
    base_rtp_ts: u32,
    last_rtp_ts: u32,
    packets_sent: u32,
    octets_sent: u32,
    started: Instant,
    last_report: Option<Instant>,
    cache: VecDeque<CachedFrame>,
    limiter: ResendLimiter,
    stats: StreamStats,
}

impl VideoStreamer {
    /// Open a socket and prepare to stream to `remote`.
    pub async fn new(remote: SocketAddr, params: StreamParams) -> std::io::Result<Self> {
        let bind: SocketAddr = if remote.is_ipv4() {
            "0.0.0.0:0".parse()
        } else {
            "[::]:0".parse()
        }
        .map_err(|e| std::io::Error::other(format!("{e}")))?;
        let socket = UdpSocket::bind(bind).await?;
        let max = if remote.is_ipv4() {
            MAX_PACKET_SIZE_V4
        } else {
            MAX_PACKET_SIZE_V6
        };
        Ok(Self {
            socket,
            remote,
            packetizer: RtpPacketizer::new(
                params.payload_type,
                params.sender_ssrc,
                max,
                rand::random(),
            ),
            params,
            next_frame_id: 0,
            last_key_frame_id: 0,
            base_rtp_ts: rand::random(),
            last_rtp_ts: 0,
            packets_sent: 0,
            octets_sent: 0,
            started: Instant::now(),
            last_report: None,
            cache: VecDeque::new(),
            limiter: ResendLimiter::new(Instant::now()),
            stats: StreamStats::default(),
        })
    }

    /// Local address of the UDP socket (tests, diagnostics).
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Counters so far.
    pub fn stats(&self) -> StreamStats {
        self.stats
    }

    /// Encrypt, packetise and send one H.264 access unit. `pts` is the time since the stream
    /// started (used for the 90 kHz RTP timestamp).
    pub async fn send_frame(
        &mut self,
        annexb: &[u8],
        key_frame: bool,
        pts: Duration,
    ) -> std::io::Result<()> {
        let frame_id = self.next_frame_id;
        self.next_frame_id = self.next_frame_id.wrapping_add(1);
        if key_frame {
            self.last_key_frame_id = frame_id;
        }
        // A delta frame references the previous frame (UNCERTAIN: what the receiver expects
        // for reference ids of non-key frames was not verified).
        let referenced = if key_frame || frame_id == 0 {
            frame_id
        } else {
            frame_id.wrapping_sub(1)
        };
        let rtp_timestamp = self
            .base_rtp_ts
            .wrapping_add((pts.as_micros() * 90_000 / 1_000_000) as u32);
        self.last_rtp_ts = rtp_timestamp;
        let meta = FrameMeta {
            frame_id,
            referenced_frame_id: referenced,
            rtp_timestamp,
            key_frame,
            new_playout_delay_ms: 0,
        };
        let mut payload = annexb.to_vec();
        rtp::crypt_frame(
            &self.params.aes_key,
            &self.params.aes_iv_mask,
            frame_id,
            &mut payload,
        );
        let packets = self
            .packetizer
            .packetize(&meta, &payload)
            .ok_or_else(|| std::io::Error::other("frame needs too many packets"))?;
        // Lip-sync information first (the very first report goes out before the first packet).
        self.sender_report_if_due().await?;
        for p in &packets {
            self.socket.send_to(p, self.remote).await?;
            self.packets_sent = self.packets_sent.wrapping_add(1);
            self.octets_sent = self.octets_sent.wrapping_add(p.len() as u32);
            self.stats.packets += 1;
        }
        self.stats.frames += 1;
        self.cache.push_back(CachedFrame {
            id: frame_id,
            resent_at: vec![None; packets.len()],
            packets,
        });
        while self.cache.len() > RESEND_CACHE_FRAMES {
            self.cache.pop_front();
        }
        Ok(())
    }

    /// Send a sender report if none was sent yet or the last one is 500 ms old.
    pub async fn sender_report_if_due(&mut self) -> std::io::Result<()> {
        let now = Instant::now();
        let due = self
            .last_report
            .is_none_or(|t| now.duration_since(t) >= SENDER_REPORT_INTERVAL);
        if !due {
            return Ok(());
        }
        self.last_report = Some(now);
        // The RTP timestamp that corresponds to "now" on the stream's timeline.
        let elapsed = self.started.elapsed();
        let ts = self
            .base_rtp_ts
            .wrapping_add((elapsed.as_micros() * 90_000 / 1_000_000) as u32);
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let sr = rtp::sender_report(
            self.params.sender_ssrc,
            rtp::ntp_timestamp(wall),
            if self.stats.frames == 0 {
                ts
            } else {
                later_rtp_ts(self.last_rtp_ts, ts)
            },
            self.packets_sent,
            self.octets_sent,
        );
        self.socket.send_to(&sr, self.remote).await?;
        Ok(())
    }

    /// Wait for one datagram from the receiver and handle it. Returns the events found
    /// (key frame requests are counted in the stats and also returned).
    pub async fn recv_rtcp(&mut self) -> std::io::Result<Vec<RtcpEvent>> {
        let mut buf = [0u8; 2048];
        let (n, from) = self.socket.recv_from(&mut buf).await?;
        if from != self.remote {
            return Ok(Vec::new());
        }
        let Some(events) = rtp::parse_rtcp(
            &buf[..n],
            self.params.receiver_ssrc,
            self.params.sender_ssrc,
        ) else {
            return Ok(Vec::new());
        };
        for e in &events {
            match e {
                RtcpEvent::PictureLoss => self.stats.picture_loss += 1,
                RtcpEvent::Feedback { nacks, .. } => {
                    self.stats.feedback += 1;
                    self.resend(nacks, Instant::now())?;
                }
                RtcpEvent::ReceiverReport => {}
            }
        }
        Ok(events)
    }

    /// Serve NACKs within the limits described in the module documentation. Never waits.
    fn resend(&mut self, nacks: &[rtp::Nack], now: Instant) -> std::io::Result<()> {
        for n in nacks {
            // The frame id on the wire is only 8 bits wide: pick the newest cached match.
            let Some(frame) = self
                .cache
                .iter_mut()
                .rev()
                .find(|f| (f.id & 0xff) as u8 == n.frame_id8)
            else {
                continue;
            };
            let mut wanted: Vec<usize> = Vec::new();
            if n.packet_id == ALL_PACKETS_LOST {
                wanted.extend(0..frame.packets.len());
            } else {
                wanted.push(usize::from(n.packet_id));
                for bit in 0..8 {
                    if n.bit_vector & (1 << bit) != 0 {
                        wanted.push(usize::from(n.packet_id) + 1 + bit);
                    }
                }
            }
            for i in wanted {
                let Some(p) = frame.packets.get(i) else {
                    continue;
                };
                let recent = frame.resent_at[i]
                    .is_some_and(|t| now.saturating_duration_since(t) < MIN_RESEND_INTERVAL);
                if recent || !self.limiter.allow(now, p.len()) {
                    self.stats.resend_suppressed += 1;
                    continue;
                }
                match self.socket.try_send_to(p, self.remote) {
                    Ok(_) => {
                        frame.resent_at[i] = Some(now);
                        self.stats.resent_packets += 1;
                    }
                    // Socket buffer full: drop this resend, do not stall the loop.
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        self.stats.resend_suppressed += 1;
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limiter_allows_a_burst_then_refills_with_time() {
        let t0 = Instant::now();
        let mut l = ResendLimiter::new(t0);
        let mut allowed = 0;
        for _ in 0..10_000 {
            if l.allow(t0, 1400) {
                allowed += 1;
            }
        }
        // Burst: 250 ms of the budget (packet limit 250 packets, byte limit 250000 / 1400).
        assert!((100..=250).contains(&allowed), "{allowed}");
        assert!(!l.allow(t0, 1400));
        // One second later the bucket is full again, but never beyond the burst size.
        let t1 = t0 + Duration::from_secs(1);
        let mut again = 0;
        for _ in 0..10_000 {
            if l.allow(t1, 1400) {
                again += 1;
            }
        }
        assert_eq!(again, allowed);
    }

    #[test]
    fn rtp_timestamp_comparison_survives_wrap_around() {
        assert_eq!(later_rtp_ts(10, 20), 20);
        assert_eq!(later_rtp_ts(20, 10), 20);
        // 5 is "after" u32::MAX - 5 once the counter wrapped.
        assert_eq!(later_rtp_ts(u32::MAX - 5, 5), 5);
        assert_eq!(later_rtp_ts(5, u32::MAX - 5), 5);
    }
}
