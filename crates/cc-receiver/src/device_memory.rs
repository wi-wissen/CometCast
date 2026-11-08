// SPDX-License-Identifier: GPL-3.0-or-later
//! The display's memory of who used a name (`docs/design/presenter-identity.md`).
//!
//! The display remembers pairs `(cleaned name, device id)` of every request it **allowed** and of
//! every presenter that joined in open mode with a valid proof. A request or open-mode join with
//! a valid proof whose name (compared without regard to case) is stored **only with other device
//! ids** gets a note: "this name was used here before by a different device". A name that is
//! stored with this device id, or never seen, gets none. Names that the server makes up
//! (`Guest N`) are never stored or compared. At most [`MAX_PAIRS`] pairs are kept; the least
//! recently used is dropped first (using a pair again, by allowing it again, makes it the newest).
//!
//! The memory is plain data: the app stores [`DeviceMemory::to_json`] with its settings and gives
//! it back through [`DeviceMemory::from_json`] (`ReceiverOptions::device_memory`); the core
//! reports every change as `ReceiverEvent::DeviceMemoryChanged`.

use serde_json::Value;

/// Most pairs kept.
pub const MAX_PAIRS: usize = 200;
/// Longest name kept, in characters (the server cuts names at 40).
const MAX_NAME_CHARS: usize = 40;

/// `true` for the label that the server gives somebody who sent no name: `Guest ` and digits.
/// Such a label says nothing about the person, so it is never stored or compared.
pub fn is_generated_name(name: &str) -> bool {
    name.strip_prefix("Guest ")
        .is_some_and(|d| !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit()))
}

/// `true` for a device id as made by `cc_transport::device::device_id`: 32 lower-case hex digits.
fn is_device_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn key_of(name: &str) -> String {
    name.trim().to_lowercase()
}

/// The pairs `(name, device id)`, the least recently used first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceMemory {
    pairs: Vec<(String, String)>,
}

impl DeviceMemory {
    /// An empty memory.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of pairs.
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// `true` when nothing is remembered.
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Read what [`DeviceMemory::to_json`] wrote. Lenient: anything that is not a list of
    /// `[name, id]` pairs of text (an empty or broken settings value, an entry with a bad id,
    /// a generated name) is skipped; only the newest [`MAX_PAIRS`] are kept.
    pub fn from_json(text: &str) -> Self {
        let mut mem = Self::new();
        if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(text) {
            for item in items {
                if let Value::Array(p) = item
                    && let [Value::String(name), Value::String(id)] = p.as_slice()
                {
                    mem.remember(name, id);
                }
            }
        }
        mem
    }

    /// The JSON text for the app's settings: `[["name","id"], ...]`, the least recently used first.
    pub fn to_json(&self) -> String {
        Value::Array(
            self.pairs
                .iter()
                .map(|(n, i)| {
                    Value::Array(vec![Value::String(n.clone()), Value::String(i.clone())])
                })
                .collect(),
        )
        .to_string()
    }

    /// The note: is `name` stored, but only with device ids other than `id`? Never for a
    /// generated name.
    pub fn used_by_other_device(&self, name: &str, id: &str) -> bool {
        if is_generated_name(name) {
            return false;
        }
        let key = key_of(name);
        let mut seen_name = false;
        for (n, i) in &self.pairs {
            if key_of(n) == key {
                if i == id {
                    return false;
                }
                seen_name = true;
            }
        }
        seen_name
    }

    /// Remember that `id` was allowed under `name`. Returns `true` when the memory changed (a
    /// generated name, a bad id or an empty name changes nothing). A known pair becomes the
    /// newest; the oldest pairs beyond [`MAX_PAIRS`] are dropped.
    pub fn remember(&mut self, name: &str, id: &str) -> bool {
        let name: String = name.trim().chars().take(MAX_NAME_CHARS).collect();
        if name.is_empty() || is_generated_name(&name) || !is_device_id(id) {
            return false;
        }
        let key = key_of(&name);
        let pos = self
            .pairs
            .iter()
            .position(|(n, i)| i == id && key_of(n) == key);
        let before = self.pairs.clone();
        if let Some(p) = pos {
            // Keep the spelling of the latest use.
            self.pairs.remove(p);
        }
        self.pairs.push((name, id.to_owned()));
        if self.pairs.len() > MAX_PAIRS {
            let extra = self.pairs.len() - MAX_PAIRS;
            self.pairs.drain(..extra);
        }
        self.pairs != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0123456789abcdef0123456789abcdef";
    const B: &str = "fedcba9876543210fedcba9876543210";

    #[test]
    fn the_note_needs_the_name_with_another_device_only() {
        let mut m = DeviceMemory::new();
        assert!(!m.used_by_other_device("Julian's iPad", A), "never seen");
        assert!(m.remember("Julian's iPad", A));
        assert!(!m.used_by_other_device("Julian's iPad", A), "same device");
        assert!(m.used_by_other_device("Julian's iPad", B), "another device");
        assert!(
            m.used_by_other_device("JULIAN'S IPAD", B),
            "case does not matter"
        );
        assert!(!m.used_by_other_device("Anna's iPad", B), "another name");
        // Stored with this device as well as another one: no note.
        m.remember("Julian's iPad", B);
        assert!(!m.used_by_other_device("Julian's iPad", B));
        assert!(!m.used_by_other_device("Julian's iPad", A));
    }

    #[test]
    fn generated_names_are_never_stored_or_compared() {
        let mut m = DeviceMemory::new();
        assert!(!m.remember("Guest 3", A));
        assert!(m.is_empty());
        m.remember("Ana", A);
        assert!(!m.used_by_other_device("Guest 3", B));
        assert!(!m.remember("", A));
        assert!(!m.remember("   ", A));
        assert!(!m.remember("Ana", "not an id"));
        // A person who really calls the device "Guest" is a name like any other.
        assert!(m.remember("Guest", A));
        assert!(m.remember("Guest 3x", A));
    }

    #[test]
    fn at_most_200_pairs_and_the_least_recently_used_goes_first() {
        let mut m = DeviceMemory::new();
        for n in 0..MAX_PAIRS {
            assert!(m.remember(&format!("name {n}"), A));
        }
        assert_eq!(m.len(), MAX_PAIRS);
        // Use the oldest pair again: it becomes the newest.
        assert!(m.remember("name 0", A));
        assert!(m.remember("one more", A));
        assert_eq!(m.len(), MAX_PAIRS);
        assert!(m.used_by_other_device("name 0", B), "kept");
        assert!(
            !m.used_by_other_device("name 1", B),
            "dropped as the oldest"
        );
        assert!(m.used_by_other_device("one more", B));
    }

    #[test]
    fn remembering_a_known_pair_changes_nothing_that_matters() {
        let mut m = DeviceMemory::new();
        assert!(m.remember("Ana", A));
        assert!(!m.remember("Ana", A), "no change, nothing to store");
        assert_eq!(m.len(), 1);
        assert!(m.remember("Bea", A));
        // Using Ana again reorders, which is a change worth storing.
        assert!(m.remember("Ana", A));
    }

    #[test]
    fn json_round_trips_and_garbage_is_skipped() {
        let mut m = DeviceMemory::new();
        m.remember("Ana", A);
        m.remember("Bea", B);
        assert_eq!(DeviceMemory::from_json(&m.to_json()), m);
        for bad in [
            "",
            "null",
            "{}",
            "[1,2]",
            r#"[["Ana"]]"#,
            r#"[["Ana",5]]"#,
            "not json",
        ] {
            assert!(DeviceMemory::from_json(bad).is_empty(), "{bad}");
        }
        let mixed = format!(r#"[["Ana","{A}"],["Guest 2","{B}"],["x","bad"],["Bea","{B}"]]"#);
        assert_eq!(DeviceMemory::from_json(&mixed).len(), 2);
        // An oversized stored list keeps the newest 200.
        let many: Vec<_> = (0..250).map(|n| format!(r#"["n{n}","{A}"]"#)).collect();
        let m = DeviceMemory::from_json(&format!("[{}]", many.join(",")));
        assert_eq!(m.len(), MAX_PAIRS);
        assert!(m.used_by_other_device("n249", B) && !m.used_by_other_device("n49", B));
    }
}
