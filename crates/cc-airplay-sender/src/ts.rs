// SPDX-License-Identifier: GPL-3.0-or-later
//! A minimal MPEG-2 transport stream writer for one H.264 video stream (ISO/IEC 13818-1,
//! the container HLS has always accepted). Written from the public format description;
//! no third-party code was used.
//!
//! Layout: PID 0 carries the PAT, PID 0x1000 the PMT, PID 0x100 the video (stream type
//! 0x1B) which is also the PCR PID. Every access unit becomes one PES packet with a PTS and
//! an access unit delimiter in front. Segments start with PAT and PMT.
//!
//! UNCERTAIN: whether Apple's strictest HLS validator accepts the stream (for example it
//! wants audio-less streams marked in some way) was not checked; ffprobe/ffmpeg decode it.

/// Size of a transport stream packet.
pub const PACKET: usize = 188;
const PID_PAT: u16 = 0;
const PID_PMT: u16 = 0x1000;
const PID_VIDEO: u16 = 0x100;
/// Access unit delimiter NAL (primary picture type: any slice type).
const AUD: [u8; 6] = [0, 0, 0, 1, 0x09, 0xF0];

fn crc32_mpeg(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04C1_1DB7
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// Writer state: the continuity counters.
#[derive(Debug, Default)]
pub struct TsMuxer {
    cc_pat: u8,
    cc_pmt: u8,
    cc_video: u8,
}

impl TsMuxer {
    /// A new writer.
    pub fn new() -> Self {
        Self::default()
    }

    fn section_packet(pid: u16, cc: &mut u8, section: &[u8], out: &mut Vec<u8>) {
        let mut pkt = [0xFFu8; PACKET];
        pkt[0] = 0x47;
        pkt[1] = 0x40 | (pid >> 8) as u8;
        pkt[2] = pid as u8;
        pkt[3] = 0x10 | (*cc & 0x0F);
        *cc = (*cc + 1) & 0x0F;
        pkt[4] = 0; // pointer field
        pkt[5..5 + section.len()].copy_from_slice(section);
        out.extend_from_slice(&pkt);
    }

    /// Append PAT and PMT.
    pub fn write_tables(&mut self, out: &mut Vec<u8>) {
        let mut pat = vec![0x00, 0xB0, 0x0D, 0x00, 0x01, 0xC1, 0x00, 0x00, 0x00, 0x01];
        pat.push(0xE0 | (PID_PMT >> 8) as u8);
        pat.push(PID_PMT as u8);
        let crc = crc32_mpeg(&pat);
        pat.extend_from_slice(&crc.to_be_bytes());
        Self::section_packet(PID_PAT, &mut self.cc_pat, &pat, out);

        let mut pmt = vec![0x02, 0xB0, 0x12, 0x00, 0x01, 0xC1, 0x00, 0x00];
        pmt.push(0xE0 | (PID_VIDEO >> 8) as u8); // PCR PID
        pmt.push(PID_VIDEO as u8);
        pmt.extend_from_slice(&[0xF0, 0x00]); // no program descriptors
        pmt.push(0x1B); // H.264
        pmt.push(0xE0 | (PID_VIDEO >> 8) as u8);
        pmt.push(PID_VIDEO as u8);
        pmt.extend_from_slice(&[0xF0, 0x00]); // no stream descriptors
        let crc = crc32_mpeg(&pmt);
        pmt.extend_from_slice(&crc.to_be_bytes());
        Self::section_packet(PID_PMT, &mut self.cc_pmt, &pmt, out);
    }

    /// Append one access unit (Annex-B bytes) with its presentation time in 90 kHz ticks.
    /// `keyframe` marks the random access point in the adaptation field.
    pub fn write_frame(&mut self, out: &mut Vec<u8>, annexb: &[u8], pts: u64, keyframe: bool) {
        let pts = pts & 0x1_FFFF_FFFF;
        let mut pes = Vec::with_capacity(annexb.len() + 32);
        pes.extend_from_slice(&[0, 0, 1, 0xE0, 0, 0, 0x84, 0x80, 5]);
        pes.extend_from_slice(&pts_field(0x20, pts));
        pes.extend_from_slice(&AUD);
        pes.extend_from_slice(annexb);

        let pcr = pts.saturating_sub(9_000); // PCR 0.1 s before the presentation time
        let mut offset = 0;
        let mut first = true;
        while offset < pes.len() {
            let remaining = pes.len() - offset;
            let mut pkt = [0xFFu8; PACKET];
            pkt[0] = 0x47;
            pkt[1] = (if first { 0x40 } else { 0 }) | (PID_VIDEO >> 8) as u8;
            pkt[2] = PID_VIDEO as u8;
            let capacity = if first { PACKET - 4 - 8 } else { PACKET - 4 };
            let (af, payload_len) = if remaining >= capacity {
                (first, capacity)
            } else {
                (true, remaining)
            };
            let mut pos = 4;
            if af {
                let af_total = PACKET - 4 - payload_len; // includes the length byte
                pkt[4] = (af_total - 1) as u8;
                if af_total > 1 {
                    pkt[5] =
                        (if first { 0x10 } else { 0 }) | if first && keyframe { 0x40 } else { 0 };
                    let mut at = 6;
                    if first {
                        // PCR: 33 bits base, 6 reserved bits, 9 bits extension.
                        pkt[at] = (pcr >> 25) as u8;
                        pkt[at + 1] = (pcr >> 17) as u8;
                        pkt[at + 2] = (pcr >> 9) as u8;
                        pkt[at + 3] = (pcr >> 1) as u8;
                        pkt[at + 4] = (((pcr & 1) as u8) << 7) | 0x7E;
                        pkt[at + 5] = 0;
                        at += 6;
                    }
                    for b in &mut pkt[at..4 + af_total] {
                        *b = 0xFF;
                    }
                }
                pos = 4 + af_total;
            }
            pkt[3] = (if af { 0x30 } else { 0x10 }) | (self.cc_video & 0x0F);
            self.cc_video = (self.cc_video + 1) & 0x0F;
            pkt[pos..pos + payload_len].copy_from_slice(&pes[offset..offset + payload_len]);
            out.extend_from_slice(&pkt);
            offset += payload_len;
            first = false;
        }
    }
}

/// The 5-byte PTS field of a PES header; `marker` is 0x20 for "PTS only".
fn pts_field(marker: u8, pts: u64) -> [u8; 5] {
    [
        marker | (((pts >> 30) as u8 & 0x07) << 1) | 1,
        (pts >> 22) as u8,
        (((pts >> 15) as u8 & 0x7F) << 1) | 1,
        (pts >> 7) as u8,
        (((pts as u8) & 0x7F) << 1) | 1,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collect the PES payload of the video PID and check the packet structure.
    fn demux(ts: &[u8]) -> Vec<u8> {
        assert_eq!(ts.len() % PACKET, 0);
        let mut payload = Vec::new();
        let mut expected_cc: Option<u8> = None;
        for p in ts.chunks(PACKET) {
            assert_eq!(p[0], 0x47);
            let pid = (u16::from(p[1] & 0x1F) << 8) | u16::from(p[2]);
            let afc = (p[3] >> 4) & 3;
            let mut pos = 4;
            if afc & 2 != 0 {
                pos += 1 + p[4] as usize;
            }
            assert!(pos <= PACKET);
            if pid == PID_VIDEO {
                let cc = p[3] & 0x0F;
                if let Some(e) = expected_cc {
                    assert_eq!(cc, e, "continuity counter");
                }
                expected_cc = Some((cc + 1) & 0x0F);
                if afc & 1 != 0 {
                    payload.extend_from_slice(&p[pos..]);
                }
            }
        }
        payload
    }

    #[test]
    fn tables_have_valid_crc() {
        let mut m = TsMuxer::new();
        let mut out = Vec::new();
        m.write_tables(&mut out);
        assert_eq!(out.len(), 2 * PACKET);
        let pat = &out[5..5 + 16];
        assert_eq!(crc32_mpeg(pat), 0, "PAT CRC over the whole section is zero");
        let pmt = &out[PACKET + 5..PACKET + 5 + 21];
        assert_eq!(crc32_mpeg(pmt), 0, "PMT CRC over the whole section is zero");
    }

    #[test]
    fn frames_of_every_size_roundtrip() {
        let mut m = TsMuxer::new();
        for size in [
            0usize, 1, 100, 160, 175, 176, 177, 183, 184, 185, 187, 188, 189, 1000, 50_000,
        ] {
            let au: Vec<u8> = (0..size).map(|i| (i % 253) as u8).collect();
            let mut out = Vec::new();
            m.write_frame(&mut out, &au, 90_000 * 7 + size as u64, size % 2 == 0);
            // Each frame starts a new PES: restart the continuity check per frame.
            let payload = demux_from_cc(&out);
            assert_eq!(&payload[..4], &[0, 0, 1, 0xE0]);
            assert_eq!(&payload[9 + 5..9 + 5 + 6], &AUD);
            assert_eq!(&payload[9 + 5 + 6..], au.as_slice());
        }
    }

    fn demux_from_cc(ts: &[u8]) -> Vec<u8> {
        // The muxer keeps counting across frames, so only check inside this buffer.
        demux(ts)
    }

    #[test]
    fn pts_field_encodes_known_value() {
        // 90000 ticks = 1 s. Known layout: 0010 PTS32-30 1 | PTS29-22 | PTS21-15 1 | PTS14-7 | PTS6-0 1
        assert_eq!(pts_field(0x20, 90_000), [0x21, 0x00, 0x05, 0xBF, 0x21]);
    }
}
