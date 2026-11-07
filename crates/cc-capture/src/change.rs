// SPDX-License-Identifier: GPL-3.0-or-later
//! Cheap change detection for whole pictures: a 64-bit hash of the pixel bytes is compared with
//! the hash of the previous picture. Pictures whose hash is equal are treated as unchanged and
//! are not encoded (docs/measurement-points.md, A10).
//!
//! UNCERTAIN: a 64-bit non-cryptographic hash can collide, so a change could in theory be
//! missed; the chance for random content is about 2^-64 per comparison. Nobody attacks this
//! (the sender sees its own screen), but a missed change would stay on screen until the next
//! change; the static refresh does not repair it because it re-encodes the same stored picture.

/// Remembers the hash of the last picture.
#[derive(Debug, Default, Clone)]
pub struct ChangeDetector {
    last: Option<(u64, u32, u32)>,
}

impl ChangeDetector {
    /// A detector that has seen nothing; the first picture always counts as changed.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a picture. Returns `true` if it differs from the previous one (or is the first,
    /// or has another size).
    pub fn update(&mut self, width: u32, height: u32, bytes: &[u8]) -> bool {
        let now = (hash_bytes(bytes), width, height);
        let changed = self.last != Some(now);
        self.last = Some(now);
        changed
    }

    /// Forget the last picture, so the next one counts as changed.
    pub fn reset(&mut self) {
        self.last = None;
    }
}

/// 64-bit hash of a byte slice: four independent lanes over 8-byte words, so the CPU can work
/// on them in parallel. Not a standard algorithm; only meant to tell pictures apart.
pub fn hash_bytes(bytes: &[u8]) -> u64 {
    const K: [u64; 4] = [
        0x9E37_79B9_7F4A_7C15,
        0xC2B2_AE3D_27D4_EB4F,
        0x1656_67B1_9E37_79F9,
        0x85EB_CA77_C2B2_AE63,
    ];
    let mut lanes: [u64; 4] = [1, 2, 3, 4];
    let (chunks, remainder) = bytes.as_chunks::<32>();
    for c in chunks {
        for (i, lane) in lanes.iter_mut().enumerate() {
            let mut w = [0u8; 8];
            w.copy_from_slice(&c[i * 8..i * 8 + 8]);
            *lane = (*lane ^ u64::from_le_bytes(w))
                .wrapping_mul(K[i])
                .rotate_left(29);
        }
    }
    let mut h = bytes.len() as u64;
    for &b in remainder {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3);
    }
    for (l, k) in lanes.iter().zip(K) {
        h = (h ^ l).wrapping_mul(k).rotate_left(31);
    }
    h ^ (h >> 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_picture_is_a_change_and_equal_pictures_are_not() {
        let mut d = ChangeDetector::new();
        let a = vec![7u8; 64 * 64 * 4];
        assert!(d.update(64, 64, &a));
        assert!(!d.update(64, 64, &a));
        assert!(!d.update(64, 64, &a.clone()));
    }

    #[test]
    fn a_single_changed_byte_anywhere_is_seen() {
        let mut d = ChangeDetector::new();
        let base = vec![0u8; 100 * 37 * 4 + 5];
        d.update(100, 37, &base);
        for pos in [0, 1, 31, 32, 33, 4000, base.len() - 1] {
            let mut p = base.clone();
            p[pos] = 1;
            assert!(d.update(100, 37, &p), "change at {pos} missed");
            assert!(d.update(100, 37, &base), "change back at {pos} missed");
        }
    }

    #[test]
    fn a_new_size_counts_as_a_change() {
        let mut d = ChangeDetector::new();
        let a = vec![0u8; 16 * 16 * 4];
        d.update(16, 16, &a);
        assert!(d.update(8, 32, &a));
        d.reset();
        assert!(d.update(8, 32, &a));
    }
}
