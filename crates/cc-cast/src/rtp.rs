// SPDX-License-Identifier: GPL-3.0-or-later
//! Cast Streaming wire format: frame encryption (AES-128-CTR), RTP packetisation, the RTCP
//! sender report, and parsing of the receiver's RTCP (picture loss, cast feedback).
//!
//! Adapted from (read, not copied verbatim) Open Screen, commit
//! `8b62b21479f1adb7663f182d97962b960c5c64ef`, BSD-3-Clause, Copyright 2019 The Chromium
//! Authors (see THIRD_PARTY_NOTICES.md):
//! `cast/streaming/impl/frame_crypto.cc`, `rtp_packetizer.cc`, `rtp_defines.h`,
//! `sender_report_builder.cc`, `ntp_time.cc`, `compound_rtcp_parser.cc`, `rtcp_common.cc`.
//!
//! UNCERTAIN (not verified against a device):
//! * Open Screen uses BoringSSL's `AES_ctr128_encrypt`, whose source was not read. This code
//!   assumes it increments the whole 128-bit counter as a big-endian number (the usual CTR
//!   mode, [`ctr::Ctr128BE`]). Measurement point: counter overflow at block 2^64 does not
//!   matter for frames of a few MB, but the carry behaviour is untested.
//! * Only the cast feedback NACK list is used for retransmission; the CST2 frame ACK bit
//!   vector, receiver reference time and receiver frame logs are ignored.

use aes::Aes128;
use ctr::Ctr128BE;
use ctr::cipher::{KeyIvInit, StreamCipher};

/// `kMaxRtpPacketSizeForIpv4UdpOnEthernet` (1500 - 20 - 8).
pub const MAX_PACKET_SIZE_V4: usize = 1472;
/// `kMaxRtpPacketSizeForIpv6UdpOnEthernet` (1500 - 40 - 8).
pub const MAX_PACKET_SIZE_V6: usize = 1452;
/// Bytes of RTP + cast header with the reference frame id (`kBaseRtpHeaderSize`).
pub const BASE_HEADER: usize = 19;
/// Extra bytes of the adaptive latency extension (`kAdaptiveLatencyHeaderSize`).
pub const LATENCY_EXT: usize = 4;
/// `kAllPacketsLost` in a NACK.
pub const ALL_PACKETS_LOST: u16 = 0xffff;

/// Encrypt or decrypt (AES-CTR is symmetric) one frame payload.
///
/// The counter block is 16 zero bytes with the lower 32 bits of the frame id written big-endian
/// at offset 8, then XORed with the IV mask from the OFFER (`FrameCrypto::Crypt`).
pub fn crypt_frame(key: &[u8; 16], iv_mask: &[u8; 16], frame_id: u32, data: &mut [u8]) {
    let mut nonce = [0u8; 16];
    nonce[8..12].copy_from_slice(&frame_id.to_be_bytes());
    for (n, m) in nonce.iter_mut().zip(iv_mask) {
        *n ^= m;
    }
    let mut c = Ctr128BE::<Aes128>::new(key.into(), (&nonce).into());
    c.apply_keystream(data);
}

/// Per-frame metadata for packetisation.
#[derive(Debug, Clone, Copy)]
pub struct FrameMeta {
    /// Frame id (counts from 0).
    pub frame_id: u32,
    /// The frame this one depends on (its own id for key frames).
    pub referenced_frame_id: u32,
    /// 90 kHz timestamp.
    pub rtp_timestamp: u32,
    /// Key frame flag.
    pub key_frame: bool,
    /// New playout delay in ms to announce in the first packet (0 = none).
    pub new_playout_delay_ms: u16,
}

/// Cuts encrypted frames into RTP packets.
#[derive(Debug)]
pub struct RtpPacketizer {
    payload_type: u8,
    ssrc: u32,
    max_packet_size: usize,
    seq: u16,
}

impl RtpPacketizer {
    /// `max_packet_size` includes the headers.
    pub fn new(payload_type: u8, ssrc: u32, max_packet_size: usize, first_seq: u16) -> Self {
        assert!(max_packet_size > BASE_HEADER + LATENCY_EXT);
        Self {
            payload_type: payload_type & 0x7f,
            ssrc,
            max_packet_size,
            seq: first_seq,
        }
    }

    fn max_payload(&self) -> usize {
        self.max_packet_size - BASE_HEADER - LATENCY_EXT
    }

    /// Number of packets for a payload of `len` bytes (at least 1).
    pub fn packet_count(&self, len: usize) -> usize {
        len.div_ceil(self.max_payload()).max(1)
    }

    /// All packets of one (already encrypted) frame.
    ///
    /// Returns `None` if the frame would need more than 65534 packets.
    pub fn packetize(&mut self, meta: &FrameMeta, payload: &[u8]) -> Option<Vec<Vec<u8>>> {
        let n = self.packet_count(payload.len());
        if n >= usize::from(ALL_PACKETS_LOST) {
            return None;
        }
        let chunk = self.max_payload();
        let mut out = Vec::with_capacity(n);
        for pid in 0..n {
            let last = pid == n - 1;
            let start = pid * chunk;
            let end = if last { payload.len() } else { start + chunk };
            let latency = pid == 0 && meta.new_playout_delay_ms > 0;
            let mut p = Vec::with_capacity(BASE_HEADER + LATENCY_EXT + (end - start));
            // RTP header: version 2, no padding / extension / CSRC.
            p.push(0x80);
            p.push(if last { 0x80 } else { 0 } | self.payload_type);
            p.extend_from_slice(&self.seq.to_be_bytes());
            self.seq = self.seq.wrapping_add(1);
            p.extend_from_slice(&meta.rtp_timestamp.to_be_bytes());
            p.extend_from_slice(&self.ssrc.to_be_bytes());
            // Cast header: key frame bit, "reference frame id present" bit, extension count.
            p.push(if meta.key_frame { 0x80 } else { 0 } | 0x40 | u8::from(latency));
            p.push((meta.frame_id & 0xff) as u8);
            p.extend_from_slice(&(pid as u16).to_be_bytes());
            p.extend_from_slice(&((n - 1) as u16).to_be_bytes());
            p.push((meta.referenced_frame_id & 0xff) as u8);
            if latency {
                // Extension type 1 (adaptive latency) in the upper 6 bits, size 2 in the lower 10.
                p.extend_from_slice(&((1u16 << 10) | 2).to_be_bytes());
                p.extend_from_slice(&meta.new_playout_delay_ms.to_be_bytes());
            }
            p.extend_from_slice(&payload[start..end]);
            out.push(p);
        }
        Some(out)
    }
}

/// Seconds between 1900-01-01 and 1970-01-01.
const NTP_UNIX_OFFSET: u64 = 2_208_988_800;

/// NTP timestamp (32.32 fixed point) for a time since the Unix epoch.
pub fn ntp_timestamp(since_unix: std::time::Duration) -> u64 {
    let secs = since_unix.as_secs() + NTP_UNIX_OFFSET;
    let frac = (u64::from(since_unix.subsec_nanos()) << 32) / 1_000_000_000;
    (secs << 32) | frac
}

/// RTCP sender report (`kSenderReport` = 200) without report blocks.
pub fn sender_report(
    ssrc: u32,
    ntp: u64,
    rtp_timestamp: u32,
    packets: u32,
    octets: u32,
) -> Vec<u8> {
    let mut p = Vec::with_capacity(28);
    p.push(0x80); // version 2, no padding, report count 0
    p.push(200);
    p.extend_from_slice(&6u16.to_be_bytes()); // payload of 24 bytes = 6 words
    p.extend_from_slice(&ssrc.to_be_bytes());
    p.extend_from_slice(&ntp.to_be_bytes());
    p.extend_from_slice(&rtp_timestamp.to_be_bytes());
    p.extend_from_slice(&packets.to_be_bytes());
    p.extend_from_slice(&octets.to_be_bytes());
    p
}

/// One NACK entry of a cast feedback message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nack {
    /// Lower 8 bits of the frame id.
    pub frame_id8: u8,
    /// Missing packet id, or [`ALL_PACKETS_LOST`].
    pub packet_id: u16,
    /// Bit i set: packet `packet_id + 1 + i` is missing too.
    pub bit_vector: u8,
}

/// What the receiver told us in an RTCP packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtcpEvent {
    /// Picture loss indication: send a key frame.
    PictureLoss,
    /// Cast feedback (`CAST`): everything up to the checkpoint arrived, plus NACKs.
    Feedback {
        /// Lower 8 bits of the checkpoint frame id.
        checkpoint8: u8,
        /// Reported playout delay in ms.
        playout_delay_ms: u16,
        /// Packet-level NACKs.
        nacks: Vec<Nack>,
    },
    /// A receiver report (contents not evaluated).
    ReceiverReport,
}

/// Parse a compound RTCP packet from the receiver for the stream `(receiver_ssrc, sender_ssrc)`.
/// Returns `None` if the data is malformed. Unknown packet types are skipped.
pub fn parse_rtcp(mut data: &[u8], receiver_ssrc: u32, sender_ssrc: u32) -> Option<Vec<RtcpEvent>> {
    let mut events = Vec::new();
    while !data.is_empty() {
        if data.len() < 4 || data[0] >> 5 != 0b100 {
            return None;
        }
        let count_or_subtype = data[0] & 0x1f;
        let ty = data[1];
        let len = usize::from(u16::from_be_bytes([data[2], data[3]])) * 4;
        if data.len() < 4 + len {
            return None;
        }
        let payload = &data[4..4 + len];
        data = &data[4 + len..];
        let u32_at = |b: &[u8], i: usize| u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        match (ty, count_or_subtype) {
            (201, _) => events.push(RtcpEvent::ReceiverReport),
            (206, 1) => {
                if payload.len() < 8 {
                    return None;
                }
                if u32_at(payload, 0) == receiver_ssrc && u32_at(payload, 4) == sender_ssrc {
                    events.push(RtcpEvent::PictureLoss);
                }
            }
            (204, 15) => {
                if payload.len() < 16 {
                    return None;
                }
                if u32_at(payload, 0) != receiver_ssrc || u32_at(payload, 4) != sender_ssrc {
                    continue; // another stream
                }
                if u32_at(payload, 8) != 0x4341_5354 {
                    return None; // not 'CAST'
                }
                let checkpoint8 = payload[12];
                let loss_fields = usize::from(payload[13]);
                let playout_delay_ms = u16::from_be_bytes([payload[14], payload[15]]);
                if payload.len() < 16 + 4 * loss_fields {
                    return None;
                }
                let nacks = (0..loss_fields)
                    .map(|i| {
                        let o = 16 + 4 * i;
                        Nack {
                            frame_id8: payload[o],
                            packet_id: u16::from_be_bytes([payload[o + 1], payload[o + 2]]),
                            bit_vector: payload[o + 3],
                        }
                    })
                    .collect();
                events.push(RtcpEvent::Feedback {
                    checkpoint8,
                    playout_delay_ms,
                    nacks,
                });
            }
            _ => {}
        }
    }
    Some(events)
}

/// Build a PLI (used by tests and the fake receiver): 12 bytes.
pub fn picture_loss_packet(receiver_ssrc: u32, sender_ssrc: u32) -> Vec<u8> {
    let mut p = vec![0x80 | 1, 206];
    p.extend_from_slice(&2u16.to_be_bytes());
    p.extend_from_slice(&receiver_ssrc.to_be_bytes());
    p.extend_from_slice(&sender_ssrc.to_be_bytes());
    p
}

/// Build a cast feedback packet (used by tests and the fake receiver).
pub fn feedback_packet(
    receiver_ssrc: u32,
    sender_ssrc: u32,
    checkpoint8: u8,
    nacks: &[Nack],
) -> Vec<u8> {
    let payload_len = 16 + 4 * nacks.len();
    let mut p = vec![0x80 | 15, 204];
    p.extend_from_slice(&((payload_len / 4) as u16).to_be_bytes());
    p.extend_from_slice(&receiver_ssrc.to_be_bytes());
    p.extend_from_slice(&sender_ssrc.to_be_bytes());
    p.extend_from_slice(&0x4341_5354u32.to_be_bytes());
    p.push(checkpoint8);
    p.push(nacks.len() as u8);
    p.extend_from_slice(&400u16.to_be_bytes());
    for n in nacks {
        p.push(n.frame_id8);
        p.extend_from_slice(&n.packet_id.to_be_bytes());
        p.push(n.bit_vector);
    }
    p
}

/// Parsed view of a Cast RTP packet (tests and the fake receiver).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRtp<'a> {
    /// Marker bit (last packet of the frame).
    pub marker: bool,
    /// Payload type.
    pub payload_type: u8,
    /// RTP sequence number.
    pub seq: u16,
    /// RTP timestamp.
    pub rtp_timestamp: u32,
    /// SSRC.
    pub ssrc: u32,
    /// Key frame bit.
    pub key_frame: bool,
    /// Lower 8 bits of the frame id.
    pub frame_id8: u8,
    /// Packet index in the frame.
    pub packet_id: u16,
    /// Index of the last packet of the frame.
    pub max_packet_id: u16,
    /// Lower 8 bits of the referenced frame id.
    pub referenced_frame_id8: Option<u8>,
    /// Adaptive latency in ms, if present.
    pub new_playout_delay_ms: Option<u16>,
    /// The (encrypted) payload.
    pub payload: &'a [u8],
}

/// Parse a packet made by [`RtpPacketizer`]. Handles the one extension this sender writes.
pub fn parse_rtp(p: &[u8]) -> Option<ParsedRtp<'_>> {
    if p.len() < 18 || p[0] != 0x80 {
        return None;
    }
    let has_ref = p[12] & 0x40 != 0;
    let ext_count = usize::from(p[12] & 0x3f);
    let mut off = 18;
    let referenced_frame_id8 = if has_ref {
        off += 1;
        Some(*p.get(18)?)
    } else {
        None
    };
    let mut delay = None;
    for _ in 0..ext_count {
        let head = u16::from_be_bytes([*p.get(off)?, *p.get(off + 1)?]);
        let size = usize::from(head & 0x3ff);
        if head >> 10 == 1 && size == 2 {
            delay = Some(u16::from_be_bytes([*p.get(off + 2)?, *p.get(off + 3)?]));
        }
        off += 2 + size;
    }
    Some(ParsedRtp {
        marker: p[1] & 0x80 != 0,
        payload_type: p[1] & 0x7f,
        seq: u16::from_be_bytes([p[2], p[3]]),
        rtp_timestamp: u32::from_be_bytes([p[4], p[5], p[6], p[7]]),
        ssrc: u32::from_be_bytes([p[8], p[9], p[10], p[11]]),
        key_frame: p[12] & 0x80 != 0,
        frame_id8: p[13],
        packet_id: u16::from_be_bytes([p[14], p[15]]),
        max_packet_id: u16::from_be_bytes([p[16], p[17]]),
        referenced_frame_id8,
        new_playout_delay_ms: delay,
        payload: p.get(off..)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crypt_is_symmetric_and_depends_on_frame_id() {
        let key = [3u8; 16];
        let mask = [9u8; 16];
        let plain: Vec<u8> = (0..100u8).collect();
        let mut a = plain.clone();
        crypt_frame(&key, &mask, 5, &mut a);
        assert_ne!(a, plain);
        let mut b = plain.clone();
        crypt_frame(&key, &mask, 6, &mut b);
        assert_ne!(a, b);
        crypt_frame(&key, &mask, 5, &mut a);
        assert_eq!(a, plain);
    }

    #[test]
    fn crypt_matches_independent_ctr_computation() {
        // Keystream block 0 is AES(key, nonce) with nonce = mask ^ (frame_id at bytes 8..12).
        use aes::cipher::{BlockEncrypt, KeyInit};
        let key = [0x11u8; 16];
        let mask = [0xa5u8; 16];
        let mut nonce = [0u8; 16];
        nonce[8..12].copy_from_slice(&0x0102_0304u32.to_be_bytes());
        for (n, m) in nonce.iter_mut().zip(&mask) {
            *n ^= m;
        }
        let mut block = aes::Block::from(nonce);
        Aes128::new((&key).into()).encrypt_block(&mut block);
        let mut data = [0u8; 16];
        crypt_frame(&key, &mask, 0x0102_0304, &mut data);
        assert_eq!(data.as_slice(), block.as_slice());
    }

    #[test]
    fn counter_carries_across_the_low_64_bits() {
        // Nonce = 0000000000000000 ffffffffffffffff, so block 1 uses counter 0000000000000001 0000000000000000.
        use aes::cipher::{BlockEncrypt, KeyInit};
        let key = [1u8; 16];
        let mut mask = [0u8; 16];
        mask[8..].fill(0xff);
        let mut two = [0u8; 32];
        crypt_frame(&key, &mask, 0, &mut two);
        let mut expect = aes::Block::from([0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0]);
        Aes128::new((&key).into()).encrypt_block(&mut expect);
        assert_eq!(&two[16..], expect.as_slice());
    }

    fn meta(frame_id: u32, key: bool, delay: u16) -> FrameMeta {
        FrameMeta {
            frame_id,
            referenced_frame_id: if key { frame_id } else { frame_id - 1 },
            rtp_timestamp: 90_000,
            key_frame: key,
            new_playout_delay_ms: delay,
        }
    }

    #[test]
    fn packetizer_splits_and_headers_round_trip() {
        let mut pk = RtpPacketizer::new(101, 60_000, 100, 65_534);
        let payload: Vec<u8> = (0..250u16).map(|i| i as u8).collect();
        let packets = pk.packetize(&meta(300, true, 0), &payload).unwrap();
        // max payload = 100 - 19 - 4 = 77 -> 4 packets.
        assert_eq!(packets.len(), 4);
        let mut joined = Vec::new();
        for (i, p) in packets.iter().enumerate() {
            assert!(p.len() <= 100);
            let r = parse_rtp(p).unwrap();
            assert_eq!(r.payload_type, 101);
            assert_eq!(r.ssrc, 60_000);
            assert_eq!(r.packet_id as usize, i);
            assert_eq!(r.max_packet_id, 3);
            assert_eq!(r.marker, i == 3);
            assert!(r.key_frame);
            assert_eq!(r.frame_id8, (300 & 0xff) as u8);
            assert_eq!(r.referenced_frame_id8, Some((300 & 0xff) as u8));
            assert_eq!(r.rtp_timestamp, 90_000);
            // Sequence numbers wrap: 65534, 65535, 0, 1.
            assert_eq!(r.seq, 65_534u16.wrapping_add(i as u16));
            joined.extend_from_slice(r.payload);
        }
        assert_eq!(joined, payload);
    }

    #[test]
    fn empty_payload_still_makes_one_packet_and_latency_extension_is_first_only() {
        let mut pk = RtpPacketizer::new(101, 1, 1472, 0);
        let packets = pk.packetize(&meta(1, false, 250), &[]).unwrap();
        assert_eq!(packets.len(), 1);
        let r = parse_rtp(&packets[0]).unwrap();
        assert!(!r.key_frame);
        assert_eq!(r.new_playout_delay_ms, Some(250));
        assert!(r.payload.is_empty());
        let mut pk = RtpPacketizer::new(101, 1, 100, 0);
        let packets = pk.packetize(&meta(2, false, 250), &[0u8; 200]).unwrap();
        assert_eq!(
            parse_rtp(&packets[0]).unwrap().new_playout_delay_ms,
            Some(250)
        );
        assert_eq!(parse_rtp(&packets[1]).unwrap().new_playout_delay_ms, None);
    }

    #[test]
    fn sender_report_layout() {
        let ntp = ntp_timestamp(std::time::Duration::new(1, 500_000_000));
        assert_eq!(ntp >> 32, 1 + NTP_UNIX_OFFSET);
        assert_eq!(ntp as u32, 1 << 31);
        let sr = sender_report(7, ntp, 1234, 10, 5000);
        assert_eq!(sr.len(), 28);
        assert_eq!(&sr[..4], &[0x80, 200, 0, 6]);
        assert_eq!(u32::from_be_bytes(sr[4..8].try_into().unwrap()), 7);
        assert_eq!(u32::from_be_bytes(sr[24..28].try_into().unwrap()), 5000);
    }

    #[test]
    fn parses_pli_and_feedback_in_a_compound_packet() {
        let mut data = vec![0x80, 201, 0, 1, 0, 0, 0, 9]; // receiver report, ssrc 9
        data.extend(picture_loss_packet(9, 7));
        let nacks = [
            Nack {
                frame_id8: 4,
                packet_id: ALL_PACKETS_LOST,
                bit_vector: 0,
            },
            Nack {
                frame_id8: 5,
                packet_id: 2,
                bit_vector: 0b101,
            },
        ];
        data.extend(feedback_packet(9, 7, 3, &nacks));
        let ev = parse_rtcp(&data, 9, 7).unwrap();
        assert_eq!(ev[0], RtcpEvent::ReceiverReport);
        assert_eq!(ev[1], RtcpEvent::PictureLoss);
        let RtcpEvent::Feedback {
            checkpoint8,
            nacks: got,
            ..
        } = &ev[2]
        else {
            panic!()
        };
        assert_eq!(*checkpoint8, 3);
        assert_eq!(got.as_slice(), nacks.as_slice());
        // Another stream's PLI is ignored.
        let ev = parse_rtcp(&picture_loss_packet(1, 2), 9, 7).unwrap();
        assert!(ev.is_empty());
    }

    #[test]
    fn rejects_malformed_rtcp() {
        assert!(parse_rtcp(&[0x00, 201, 0, 0], 1, 2).is_none());
        assert!(parse_rtcp(&[0x80, 201, 0, 9], 1, 2).is_none());
        assert!(parse_rtcp(&[0x80], 1, 2).is_none());
    }
}
