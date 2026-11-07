// SPDX-License-Identifier: GPL-3.0-or-later
//! The display's store of hand-over tokens (native-apps.md 20.5, decision N32).
//!
//! The iOS broadcast extension has to join on its own connection, but the host approves a
//! person once. So the presenter's app hands its approval over: it makes a token (128 random
//! bits, base64url), sends it to the display on the content control channel and the display
//! holds it here. Whoever joins next **with this token and the same name** is let in without a
//! dialog. The token
//!
//! * lives for [`HANDOVER_TTL`] (60 s) and works once: taking it removes it (a replay is an
//!   ordinary join request),
//! * belongs to this display only (the store is part of one [`ReceiverCore`](crate::ReceiverCore);
//!   the server never sees whether a token is good, so a token of another room is simply
//!   unknown here),
//! * is bound to the presenter that asked for it: the name the display knows from `peer_joined`
//!   must be the name of the join (names are cleaned by the server, so both sides see the same
//!   text),
//! * is held at most once per presenter name and at most [`MAX_TOKENS`] in all, so a flood of
//!   `handover` messages cannot grow the store: a newer token of the same presenter replaces
//!   the older one, and a full store refuses the rest (no `handover_ok`).

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// How long a token is valid.
pub const HANDOVER_TTL: Duration = Duration::from_secs(60);

/// Most tokens held at once.
pub const MAX_TOKENS: usize = 4;

struct Entry {
    token: String,
    name: String,
    expires: Instant,
}

/// Single-use hand-over tokens of one display. Safe to share between tasks.
pub struct HandoverStore {
    ttl: Duration,
    entries: Mutex<Vec<Entry>>,
}

/// Compare without stopping at the first difference (the tokens are secrets for 60 s).
fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

impl HandoverStore {
    /// An empty store whose tokens live for `ttl`.
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: Mutex::new(Vec::new()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Entry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Hold `token` for the presenter `name`. A token the store holds already is refused (a
    /// token is never reused), and so is anything beyond [`MAX_TOKENS`]. A presenter's older
    /// token is replaced. Returns whether the token is held now.
    pub fn insert(&self, token: &str, name: &str, now: Instant) -> bool {
        let mut e = self.lock();
        e.retain(|x| x.expires > now);
        if e.iter().any(|x| same(&x.token, token)) {
            return false;
        }
        e.retain(|x| x.name != name);
        if e.len() >= MAX_TOKENS {
            return false;
        }
        e.push(Entry {
            token: token.to_owned(),
            name: name.to_owned(),
            expires: now + self.ttl,
        });
        true
    }

    /// Use `token` for a join of `name`: true exactly once, and only while the token is valid
    /// and was issued to this name. A token that does not fit (unknown, expired, another name)
    /// is left alone: it is not consumed by somebody else's attempt.
    pub fn take(&self, token: &str, name: &str, now: Instant) -> bool {
        let mut e = self.lock();
        e.retain(|x| x.expires > now);
        let Some(i) = e.iter().position(|x| same(&x.token, token)) else {
            return false;
        };
        if e[i].name != name {
            return false;
        }
        e.remove(i);
        true
    }

    /// Is a token of this presenter held (valid now)?
    pub fn pending_for(&self, name: &str, now: Instant) -> bool {
        self.lock()
            .iter()
            .any(|x| x.name == name && x.expires > now)
    }

    /// Drop the tokens that ran out; returns how many did.
    pub fn expire(&self, now: Instant) -> usize {
        let mut e = self.lock();
        let before = e.len();
        e.retain(|x| x.expires > now);
        before - e.len()
    }

    /// Forget everything.
    pub fn clear(&self) {
        self.lock().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (HandoverStore, Instant) {
        (HandoverStore::new(HANDOVER_TTL), Instant::now())
    }

    #[test]
    fn a_token_works_once() {
        let (s, t0) = store();
        assert!(s.insert("tok1", "Ana", t0));
        assert!(s.pending_for("Ana", t0));
        assert!(s.take("tok1", "Ana", t0));
        // Replay.
        assert!(!s.take("tok1", "Ana", t0));
        assert!(!s.pending_for("Ana", t0));
    }

    #[test]
    fn a_token_expires_after_the_ttl() {
        let (s, t0) = store();
        assert!(s.insert("tok1", "Ana", t0));
        assert!(s.pending_for("Ana", t0 + HANDOVER_TTL - Duration::from_millis(1)));
        let late = t0 + HANDOVER_TTL + Duration::from_millis(1);
        assert!(!s.pending_for("Ana", late));
        assert!(!s.take("tok1", "Ana", late));
        assert_eq!(s.expire(late), 1);
        assert_eq!(s.expire(late), 0);
    }

    #[test]
    fn a_token_is_bound_to_its_presenter_and_survives_a_wrong_attempt() {
        let (s, t0) = store();
        assert!(s.insert("tok1", "Ana", t0));
        assert!(!s.take("tok1", "Ben", t0));
        assert!(!s.take("other", "Ana", t0));
        assert!(!s.take("", "Ana", t0));
        assert!(s.take("tok1", "Ana", t0), "still there for the right name");
    }

    #[test]
    fn the_store_is_bounded_and_a_token_is_never_reused() {
        let (s, t0) = store();
        assert!(s.insert("tok1", "Ana", t0));
        // The same token again (even for another name) is refused.
        assert!(!s.insert("tok1", "Ben", t0));
        // A newer token of the same presenter replaces the older one.
        assert!(s.insert("tok2", "Ana", t0));
        assert!(!s.take("tok1", "Ana", t0));
        for (i, n) in ["B", "C", "D"].iter().enumerate() {
            assert!(s.insert(&format!("t{i}"), n, t0));
        }
        assert!(!s.insert("tx", "E", t0), "full");
        s.clear();
        assert!(s.insert("tx", "E", t0));
    }

    #[test]
    fn the_comparison_looks_at_the_whole_token() {
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("abc", "ab"));
    }
}
