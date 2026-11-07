// SPDX-License-Identifier: GPL-3.0-or-later
//! Connection check code (a short authentication string), the same as
//! `apps/web/src/core/sas.ts` and the Swift core.
//!
//! Both sides hash the two DTLS certificate fingerprints of the final offer and answer and show
//! the result; if a relay swapped the fingerprints during the key exchange the two codes
//! differ. It only helps when people really compare the codes (not enforced). A short code can
//! in principle be ground by an active attacker who has time to try many certificates, so it is
//! a check, not a proof. Test vectors: `tests/fixtures/sas.json` (shared with the web client).

use sha2::{Digest, Sha256};

const DOMAIN: &str = "cometcast/connection-check/v1";

/// The first `a=fingerprint:sha-256 XX:..` of an SDP, as lowercase hex with colons, or `None`.
/// Same rule as `sdpFingerprint` in the web client: the line must have exactly 32 hex pairs.
pub fn sdp_fingerprint(sdp: &str) -> Option<String> {
    // JavaScript's multiline `^`/`$` treat these four as line ends.
    for line in sdp.split(['\n', '\r', '\u{2028}', '\u{2029}']) {
        let Some(rest) = line.strip_prefix("a=fingerprint:sha-256") else {
            continue;
        };
        // One or more spaces or tabs, the fingerprint, optional trailing spaces or tabs.
        let rest_trimmed = rest.trim_start_matches([' ', '\t']);
        if rest_trimmed.len() == rest.len() {
            continue;
        }
        let fp = rest_trimmed.trim_end_matches([' ', '\t']);
        if is_fingerprint(fp) {
            return Some(fp.to_ascii_lowercase());
        }
        // The web regex looks for the first line that matches; keep looking.
    }
    None
}

fn is_fingerprint(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 32 * 3 - 1 {
        return false;
    }
    b.iter().enumerate().all(|(i, c)| {
        if i % 3 == 2 {
            *c == b':'
        } else {
            c.is_ascii_hexdigit()
        }
    })
}

/// Six digits (`"123 456"`) from the two fingerprints, independent of their order (they are
/// sorted), or `None` when an SDP has no usable fingerprint.
pub fn connection_code(local_sdp: &str, remote_sdp: &str) -> Option<String> {
    let a = sdp_fingerprint(local_sdp)?;
    let b = sdp_fingerprint(remote_sdp)?;
    let (x, y) = if a <= b { (a, b) } else { (b, a) };
    let digest = Sha256::digest(format!("{DOMAIN}\n{x}\n{y}").as_bytes());
    let n = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
    let d = format!("{:06}", n % 1_000_000);
    Some(format!("{} {}", &d[..3], &d[3..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_vectors_match_the_web_client() {
        let text = include_str!("../../../tests/fixtures/sas.json");
        let v: serde_json::Value = serde_json::from_str(text).unwrap();
        let cases = v["code"].as_array().unwrap();
        assert!(!cases.is_empty());
        for c in cases {
            let local = c["local_sdp"].as_str().unwrap();
            let remote = c["remote_sdp"].as_str().unwrap();
            let got = connection_code(local, remote);
            assert_eq!(got.as_deref(), c["expect"].as_str(), "case {}", c["name"]);
        }
        for c in v["fingerprint"].as_array().into_iter().flatten() {
            let got = sdp_fingerprint(c["sdp"].as_str().unwrap());
            assert_eq!(got.as_deref(), c["expect"].as_str(), "case {}", c["name"]);
        }
    }

    #[test]
    fn no_fingerprint_no_code() {
        assert_eq!(connection_code("v=0\r\n", "v=0\r\n"), None);
    }
}
