// SPDX-License-Identifier: GPL-3.0-or-later
//! ALTCHA proof-of-work: the server hands out a signed puzzle (`GET /altcha/challenge`),
//! the client solves it (a few hundred thousand cheap hash steps, well under a second
//! on a normal device) and sends the answer with `create_room`.
//!
//! This uses the official `altcha` crate (version 1.1, MIT licence; it implements the
//! ALTCHA "Proof-of-Work v2" protocol). We use its "deterministic mode": the server
//! picks a secret counter, derives the answer once, and signs it. Checking a solution
//! is then one HMAC, so verifying costs the server almost nothing.
//!
//! Every puzzle can be used once (replay protection) and expires.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use altcha::{Challenge, CreateChallengeOptions, Payload, VerifySolutionOptions};
use axum::Extension;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use rand::Rng;

use crate::AppState;
use crate::config::AltchaConfig;
use crate::net::ClientIp;

/// A payload longer than this is refused without looking at it. (A real one is under 1 KiB.)
const MAX_PAYLOAD_BYTES: usize = 8 * 1024;

/// Why a solution was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PuzzleError {
    /// Needed but not sent.
    Missing,
    /// Sent but wrong, expired or already used.
    Invalid,
}

/// Creates and checks puzzles.
pub(crate) struct Puzzle {
    cfg: AltchaConfig,
    /// Signs the puzzle.
    secret: String,
    /// Signs the correct answer (so checking does not need to redo the work).
    key_secret: String,
    /// Puzzles that were already used, kept until they would have expired anyway.
    used: moka::sync::Cache<String, ()>,
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

impl Puzzle {
    pub(crate) fn new(cfg: &AltchaConfig) -> Self {
        let secret = match &cfg.secret {
            Some(s) => s.0.clone(),
            None => {
                // No secret configured: a random one that lives as long as the process.
                let mut bytes = [0u8; 32];
                rand::rng().fill(&mut bytes);
                bytes.iter().map(|b| format!("{b:02x}")).collect()
            }
        };
        let key_secret = format!("{secret}/key");
        Self {
            used: moka::sync::Cache::builder()
                // Slightly longer than the puzzle lifetime, so a used one cannot come back.
                .time_to_live(cfg.ttl + Duration::from_secs(60))
                // UNCERTAIN: 200 000 remembered solutions is a placeholder cap.
                .max_capacity(200_000)
                .build(),
            cfg: cfg.clone(),
            secret,
            key_secret,
        }
    }

    /// Make a new puzzle. `level` 0 is the base difficulty; each level doubles the work,
    /// up to `max_difficulty_factor`.
    pub(crate) fn challenge(&self, level: u32) -> Result<Challenge, altcha::Error> {
        let factor = 2u32
            .saturating_pow(level)
            .min(self.cfg.max_difficulty_factor.max(1));
        let max_counter = self.cfg.base_counter.saturating_mul(factor).max(2);
        // A secret counter between half and all of `max_counter`.
        let counter = rand::rng().random_range(max_counter / 2..=max_counter);
        let expires = unix_now() + i64::try_from(self.cfg.ttl.as_secs()).unwrap_or(i64::MAX / 2);
        altcha::create_challenge(CreateChallengeOptions {
            algorithm: "PBKDF2/SHA-256".into(),
            cost: self.cfg.cost,
            counter: Some(counter),
            expires_at: Some(expires),
            hmac_signature_secret: Some(self.secret.clone()),
            hmac_key_signature_secret: Some(self.key_secret.clone()),
            ..Default::default()
        })
    }

    /// Check what a client sent with `create_room`. With mode `optional` a missing
    /// payload is fine, but a wrong one is still refused.
    pub(crate) fn check(&self, payload: Option<&str>) -> Result<(), PuzzleError> {
        let Some(text) = payload else {
            return match self.cfg.mode {
                crate::config::AltchaMode::Always => Err(PuzzleError::Missing),
                crate::config::AltchaMode::Optional => Ok(()),
            };
        };
        self.verify(text)
    }

    fn verify(&self, text: &str) -> Result<(), PuzzleError> {
        if text.len() > MAX_PAYLOAD_BYTES {
            return Err(PuzzleError::Invalid);
        }
        let bytes = BASE64.decode(text).map_err(|_| PuzzleError::Invalid)?;
        let payload: Payload = serde_json::from_slice(&bytes).map_err(|_| PuzzleError::Invalid)?;
        // Our own puzzles always carry an expiry; one without is not ours.
        if payload
            .challenge
            .parameters
            .expires_at
            .is_none_or(|e| e == 0)
        {
            return Err(PuzzleError::Invalid);
        }
        let result = altcha::verify_solution(VerifySolutionOptions {
            hmac_key_signature_secret: Some(self.key_secret.clone()),
            ..VerifySolutionOptions::new(&payload.challenge, &payload.solution, self.secret.clone())
        })
        .map_err(|_| PuzzleError::Invalid)?;
        if !result.verified {
            return Err(PuzzleError::Invalid);
        }
        // Each solved puzzle counts once. The signature is unique per puzzle.
        let id = payload.challenge.signature.clone().unwrap_or_default();
        if !self.used.entry(id).or_insert(()).is_fresh() {
            return Err(PuzzleError::Invalid);
        }
        Ok(())
    }
}

/// How hard the next puzzle should be (0 = base). Grows with
/// server load, with bad events in the last minute, and for clients that recently failed.
/// UNCERTAIN: all thresholds are placeholders.
pub(crate) fn difficulty_level(
    rooms: usize,
    max_rooms: usize,
    recent_events: usize,
    client_failed_recently: bool,
) -> u32 {
    let load_pct = rooms.saturating_mul(100) / max_rooms.max(1);
    let load = match load_pct {
        0..50 => 0,
        50..75 => 1,
        75..90 => 2,
        _ => 3,
    };
    let abuse = match recent_events {
        0..10 => 0,
        10..50 => 1,
        50..200 => 2,
        _ => 3,
    };
    load.max(abuse) + u32::from(client_failed_recently)
}

/// `GET /altcha/challenge`
pub(crate) async fn challenge_handler(
    State(state): State<AppState>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
) -> Response {
    let now = Instant::now();
    if let Err(deny) = state.guard.check_http(ip, now) {
        return crate::deny_response(deny);
    }
    let rooms = state.rooms.lock().unwrap_or_else(|e| e.into_inner()).len();
    let level = difficulty_level(
        rooms,
        state.guard.max_rooms(),
        state.guard.recent_events(now),
        state.guard.has_recent_failures(ip, now),
    );
    match state.puzzle.challenge(level) {
        Ok(ch) => ([(header::CACHE_CONTROL, "no-store")], Json(ch)).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "cannot make a puzzle\n").into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AltchaMode, Mode};

    fn cfg(mode: AltchaMode) -> AltchaConfig {
        AltchaConfig {
            mode,
            // Tiny puzzle so tests are fast.
            cost: 10,
            base_counter: 20,
            ..AltchaConfig::for_mode(Mode::Internal)
        }
    }

    /// What a client does: solve and wrap as base64 JSON.
    fn solve(ch: &Challenge) -> String {
        let solution = altcha::solve_challenge(altcha::SolveChallengeOptions::new(ch))
            .unwrap()
            .unwrap();
        let payload = Payload {
            challenge: ch.clone(),
            solution,
        };
        BASE64.encode(serde_json::to_vec(&payload).unwrap())
    }

    #[test]
    fn solved_puzzle_is_accepted_once() {
        let p = Puzzle::new(&cfg(AltchaMode::Always));
        let ch = p.challenge(0).unwrap();
        let answer = solve(&ch);
        assert_eq!(p.check(Some(&answer)), Ok(()));
        // The same answer again is a replay.
        assert_eq!(p.check(Some(&answer)), Err(PuzzleError::Invalid));
    }

    #[test]
    fn missing_payload_depends_on_mode() {
        let always = Puzzle::new(&cfg(AltchaMode::Always));
        assert_eq!(always.check(None), Err(PuzzleError::Missing));
        let optional = Puzzle::new(&cfg(AltchaMode::Optional));
        assert_eq!(optional.check(None), Ok(()));
        // Optional still refuses a wrong payload.
        assert_eq!(optional.check(Some("AAAA")), Err(PuzzleError::Invalid));
    }

    #[test]
    fn garbage_tampering_and_foreign_puzzles_are_refused() {
        let p = Puzzle::new(&cfg(AltchaMode::Always));
        assert_eq!(p.check(Some("not base64 !!")), Err(PuzzleError::Invalid));
        assert_eq!(
            p.check(Some(&BASE64.encode(b"{\"a\":1}"))),
            Err(PuzzleError::Invalid)
        );
        assert_eq!(
            p.check(Some(&"A".repeat(MAX_PAYLOAD_BYTES + 1))),
            Err(PuzzleError::Invalid)
        );
        // A puzzle signed by another server (other secret).
        let other = Puzzle::new(&cfg(AltchaMode::Always));
        let foreign = solve(&other.challenge(0).unwrap());
        assert_eq!(p.check(Some(&foreign)), Err(PuzzleError::Invalid));
        // A changed solution.
        let ch = p.challenge(0).unwrap();
        let mut sol = altcha::solve_challenge(altcha::SolveChallengeOptions::new(&ch))
            .unwrap()
            .unwrap();
        sol.derived_key = "00".repeat(32);
        let bad = BASE64.encode(
            serde_json::to_vec(&Payload {
                challenge: ch,
                solution: sol,
            })
            .unwrap(),
        );
        assert_eq!(p.check(Some(&bad)), Err(PuzzleError::Invalid));
    }

    #[test]
    fn expired_puzzle_is_refused() {
        let mut c = cfg(AltchaMode::Always);
        c.ttl = Duration::from_secs(0);
        let p = Puzzle::new(&c);
        let ch = p.challenge(0).unwrap();
        // ttl 0 means expires_at == now; make sure we are past it.
        std::thread::sleep(Duration::from_millis(1100));
        assert_eq!(p.check(Some(&solve(&ch))), Err(PuzzleError::Invalid));
    }

    #[test]
    fn difficulty_grows_with_load_and_abuse_and_is_capped() {
        assert_eq!(difficulty_level(0, 100, 0, false), 0);
        assert_eq!(difficulty_level(60, 100, 0, false), 1);
        assert_eq!(difficulty_level(80, 100, 0, false), 2);
        assert_eq!(difficulty_level(95, 100, 0, false), 3);
        assert_eq!(difficulty_level(0, 100, 20, false), 1);
        assert_eq!(difficulty_level(0, 100, 500, false), 3);
        assert_eq!(difficulty_level(0, 100, 0, true), 1);
        assert_eq!(difficulty_level(95, 100, 500, true), 4);

        let c = AltchaConfig {
            base_counter: 100,
            max_difficulty_factor: 4,
            ..cfg(AltchaMode::Always)
        };
        let p = Puzzle::new(&c);
        // Level 3 would be factor 8, but the cap is 4: the search is at most 400 long.
        for _ in 0..20 {
            let ch = p.challenge(3).unwrap();
            let sol = altcha::solve_challenge(altcha::SolveChallengeOptions::new(&ch))
                .unwrap()
                .unwrap();
            assert!(sol.counter <= 400, "counter {}", sol.counter);
        }
    }
}
