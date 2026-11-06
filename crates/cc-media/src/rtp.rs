// SPDX-License-Identifier: GPL-3.0-or-later
//! Puts the RTP payloads of an H.264 stream (RFC 6184, packetization-mode 1) back together
//! into Annex-B access units. Used only by tests and the command line tool to hand received
//! video to [`crate::H264Decoder`]; the receiver apps decode with platform decoders.
//!
//! Handled payload types: single NAL unit (1..=23), STAP-A (24) and FU-A (28). Others (STAP-B,
//! MTAP, FU-B) are not produced by WebRTC senders and are skipped.
//!
//! If a packet is missing (sequence number gap), the access unit in progress is dropped, so a
//! half picture is never handed to the decoder.

/// A complete picture's worth of Annex-B data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessUnit {
    /// Annex-B bytes (start codes before every NAL unit).
    pub data: Vec<u8>,
    /// RTP timestamp shared by all packets of the unit.
    pub timestamp: u32,
}

/// See the module docs.
#[derive(Debug, Default)]
pub struct H264Assembler {
    buf: Vec<u8>,
    timestamp: Option<u32>,
    last_seq: Option<u16>,
    /// True while a FU-A fragment sequence is open.
    in_fragment: bool,
    /// True if the current access unit lost a packet.
    broken: bool,
    /// Access units dropped because of gaps.
    pub dropped_units: u64,
}

const START_CODE: [u8; 4] = [0, 0, 0, 1];

/// Largest access unit that is collected. A bigger one (packets of one timestamp without a
/// marker bit, or a hostile sender) is dropped and the assembler waits for the next picture.
/// UNCERTAIN: 8 MiB is a placeholder; a 4K IDR frame at a high bit rate is well below it.
pub const MAX_ACCESS_UNIT_BYTES: usize = 8 * 1024 * 1024;

impl H264Assembler {
    /// A new, empty assembler.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one RTP packet. Returns the finished access unit when the packet carries the
    /// marker bit and nothing was lost.
    pub fn push(
        &mut self,
        payload: &[u8],
        marker: bool,
        timestamp: u32,
        sequence: u16,
    ) -> Option<AccessUnit> {
        if let Some(prev) = self.last_seq
            && sequence != prev.wrapping_add(1)
        {
            self.mark_broken();
        }
        self.last_seq = Some(sequence);
        if self.timestamp.is_some_and(|t| t != timestamp) {
            // A new picture started without a marker on the previous one: that one is lost.
            if !self.buf.is_empty() {
                self.dropped_units += 1;
            }
            self.buf.clear();
            self.in_fragment = false;
            self.broken = false;
        }
        self.timestamp = Some(timestamp);

        if !self.broken {
            self.append(payload);
            if self.buf.len() > MAX_ACCESS_UNIT_BYTES {
                // Counts as a dropped unit, so the caller waits for an IDR frame.
                self.mark_broken();
            }
        }
        if marker {
            let done = !self.broken && !self.buf.is_empty() && !self.in_fragment;
            let out = done.then(|| AccessUnit {
                data: std::mem::take(&mut self.buf),
                timestamp,
            });
            if !done && !self.buf.is_empty() {
                self.dropped_units += 1;
            }
            self.buf.clear();
            self.in_fragment = false;
            self.broken = false;
            self.timestamp = None;
            return out;
        }
        None
    }

    fn mark_broken(&mut self) {
        if !self.broken && !self.buf.is_empty() {
            self.dropped_units += 1;
        }
        self.broken = true;
        self.buf.clear();
        self.in_fragment = false;
    }

    fn append(&mut self, p: &[u8]) {
        let Some(&b0) = p.first() else { return };
        match b0 & 0x1f {
            1..=23 => {
                self.buf.extend_from_slice(&START_CODE);
                self.buf.extend_from_slice(p);
            }
            24 => {
                let mut i = 1;
                while i + 2 <= p.len() {
                    let size = usize::from(p[i]) << 8 | usize::from(p[i + 1]);
                    i += 2;
                    let Some(nal) = p.get(i..i + size) else {
                        self.broken = true;
                        return;
                    };
                    self.buf.extend_from_slice(&START_CODE);
                    self.buf.extend_from_slice(nal);
                    i += size;
                }
            }
            28 => {
                let Some(&fu) = p.get(1) else { return };
                let (start, end) = (fu & 0x80 != 0, fu & 0x40 != 0);
                if start {
                    self.buf.extend_from_slice(&START_CODE);
                    // Rebuild the NAL header from the FU indicator and the FU header.
                    self.buf.push((b0 & 0xe0) | (fu & 0x1f));
                    self.in_fragment = true;
                } else if !self.in_fragment {
                    // The start of this NAL unit was never seen.
                    self.broken = true;
                    return;
                }
                self.buf.extend_from_slice(&p[2..]);
                if end {
                    self.in_fragment = false;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_nal_and_stap_a_make_one_unit() {
        let mut a = H264Assembler::new();
        // STAP-A with SPS (2 bytes) and PPS (1 byte).
        let stap = [0x18, 0, 2, 0x67, 0xaa, 0, 1, 0x68];
        assert_eq!(a.push(&stap, false, 100, 1), None);
        let au = a.push(&[0x65, 1, 2, 3], true, 100, 2).unwrap();
        assert_eq!(
            au.data,
            [
                0, 0, 0, 1, 0x67, 0xaa, 0, 0, 0, 1, 0x68, 0, 0, 0, 1, 0x65, 1, 2, 3
            ]
        );
    }

    #[test]
    fn fu_a_fragments_are_joined() {
        let mut a = H264Assembler::new();
        // NAL 0x65 split in three: indicator 0x7c (ref_idc 3, type 28).
        assert_eq!(a.push(&[0x7c, 0x85, 1, 2], false, 5, 10), None);
        assert_eq!(a.push(&[0x7c, 0x05, 3], false, 5, 11), None);
        let au = a.push(&[0x7c, 0x45, 4], true, 5, 12).unwrap();
        assert_eq!(au.data, [0, 0, 0, 1, 0x65, 1, 2, 3, 4]);
    }

    #[test]
    fn a_lost_packet_drops_the_whole_unit() {
        let mut a = H264Assembler::new();
        assert_eq!(a.push(&[0x7c, 0x85, 1], false, 5, 10), None);
        // sequence 11 is missing
        assert_eq!(a.push(&[0x7c, 0x45, 4], true, 5, 12), None);
        assert_eq!(a.dropped_units, 1);
        // The next picture is fine again.
        let au = a.push(&[0x41, 9], true, 6, 13).unwrap();
        assert_eq!(au.data, [0, 0, 0, 1, 0x41, 9]);
    }

    #[test]
    fn a_missing_marker_loses_only_that_unit() {
        let mut a = H264Assembler::new();
        assert_eq!(a.push(&[0x41, 1], false, 1, 1), None);
        let au = a.push(&[0x41, 2], true, 2, 2).unwrap();
        assert_eq!(au.data, [0, 0, 0, 1, 0x41, 2]);
        assert_eq!(a.dropped_units, 1);
    }

    #[test]
    fn an_endless_unit_without_marker_is_capped() {
        let mut a = H264Assembler::new();
        let payload = vec![0x41u8; 1200];
        // 12k packets of 1200 bytes (14 MB) with one timestamp and never a marker bit.
        for seq in 0..12_000u16 {
            assert_eq!(a.push(&payload, false, 7, seq), None);
            assert!(a.buf.len() <= MAX_ACCESS_UNIT_BYTES + 2 * payload.len());
        }
        assert_eq!(a.dropped_units, 1, "the oversized unit counts once");
        assert!(a.buf.is_empty());
        // The marker packet of the broken unit yields nothing; the next picture is fine.
        assert_eq!(a.push(&[0x41, 1], true, 7, 12_000), None);
        let au = a.push(&[0x65, 2], true, 8, 12_001).unwrap();
        assert_eq!(au.data, [0, 0, 0, 1, 0x65, 2]);
    }
}
