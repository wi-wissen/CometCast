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

/// Counters, for the CLI and for tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StreamStats {
    /// Frames sent.
    pub frames: u64,
    /// RTP packets sent (retransmissions not counted).
    pub packets: u64,
    /// Retransmitted packets.
    pub resent_packets: u64,
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
    cache: VecDeque<(u32, Vec<Vec<u8>>)>,
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
            stats: StreamStats::default(),
        })
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
        self.cache.push_back((frame_id, packets));
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
                self.last_rtp_ts.max(ts)
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
        if from.ip() != self.remote.ip() {
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
                    self.resend(nacks).await?;
                }
                RtcpEvent::ReceiverReport => {}
            }
        }
        Ok(events)
    }

    async fn resend(&mut self, nacks: &[rtp::Nack]) -> std::io::Result<()> {
        for n in nacks {
            // The frame id on the wire is only 8 bits wide: pick the newest cached match.
            let Some((_, packets)) = self
                .cache
                .iter()
                .rev()
                .find(|(id, _)| (*id & 0xff) as u8 == n.frame_id8)
            else {
                continue;
            };
            let mut wanted: Vec<usize> = Vec::new();
            if n.packet_id == ALL_PACKETS_LOST {
                wanted.extend(0..packets.len());
            } else {
                wanted.push(usize::from(n.packet_id));
                for bit in 0..8 {
                    if n.bit_vector & (1 << bit) != 0 {
                        wanted.push(usize::from(n.packet_id) + 1 + bit);
                    }
                }
            }
            let to_send: Vec<Vec<u8>> = wanted
                .into_iter()
                .filter_map(|i| packets.get(i).cloned())
                .collect();
            for p in to_send {
                self.socket.send_to(&p, self.remote).await?;
                self.stats.resent_packets += 1;
            }
        }
        Ok(())
    }
}
