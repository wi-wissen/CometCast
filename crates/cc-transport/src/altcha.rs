// SPDX-License-Identifier: GPL-3.0-or-later
//! ALTCHA proof-of-work for `create_room` on servers that require it (open mode).
//!
//! Flow (docs/server.md): `GET /altcha/challenge` gives a signed puzzle; the client searches
//! the counter whose PBKDF2 result starts with the puzzle's key prefix; the answer goes into
//! `create_room.altcha` as standard base64 of `{"challenge": ..., "solution": ...}`.
//! The search uses the official `altcha` crate (the same one the server verifies with).

use std::time::Duration;

use altcha::{Challenge, Payload, SolveChallengeOptions, solve_challenge};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

use crate::error::TransportError;
use crate::net::{HttpResponse, ServerAddr, TlsTrust, http_get};

/// Give up solving after this long. UNCERTAIN: placeholder; the server's default puzzle needs
/// about 750 PBKDF2 runs of 1000 iterations on average, but solving time on weak devices is
/// not measured (docs/measurement-points.md, point 8).
const SOLVE_TIMEOUT: Duration = Duration::from_secs(60);

/// Turn a non-200 answer of an HTTP endpoint into an error with the server's vocabulary
/// (`rate_limited`, `banned`, ...), so callers can treat it like a signalling `error`.
pub(crate) fn http_status_error(resp: &HttpResponse) -> TransportError {
    let (code, message) = match resp.status {
        429 => ("rate_limited", "too many requests, try again later"),
        // The server sends Retry-After with a ban and not with a plain "not allowed".
        403 if resp.retry_after.is_some() => ("banned", "this address is banned for a while"),
        403 => ("forbidden", "this address is not allowed on the server"),
        503 => ("server_full", "the server is full"),
        s => {
            return TransportError::Protocol(format!("unexpected HTTP status {s}"));
        }
    };
    TransportError::Server {
        code: code.into(),
        message: message.into(),
    }
}

/// Solve a challenge (the JSON body of `/altcha/challenge`) and return the text for
/// `create_room.altcha`. CPU-bound; async callers should use [`solve_off_thread`].
pub fn solve_challenge_json(body: &[u8]) -> Result<String, TransportError> {
    let challenge: Challenge = serde_json::from_slice(body)
        .map_err(|e| TransportError::Protocol(format!("bad ALTCHA challenge: {e}")))?;
    let mut opts = SolveChallengeOptions::new(&challenge);
    opts.timeout_ms = u64::try_from(SOLVE_TIMEOUT.as_millis()).unwrap_or(u64::MAX);
    let solution = solve_challenge(opts)
        .map_err(|e| TransportError::Invalid(format!("cannot solve ALTCHA challenge: {e}")))?
        .ok_or(TransportError::Timeout("solving the ALTCHA puzzle"))?;
    let payload = Payload {
        challenge,
        solution,
    };
    let json = serde_json::to_vec(&payload).map_err(|e| TransportError::Protocol(e.to_string()))?;
    Ok(BASE64.encode(json))
}

/// [`solve_challenge_json`] on a blocking thread, so the async runtime keeps running.
pub async fn solve_off_thread(body: Vec<u8>) -> Result<String, TransportError> {
    tokio::task::spawn_blocking(move || solve_challenge_json(&body))
        .await
        .map_err(|e| TransportError::Protocol(format!("solver task failed: {e}")))?
}

/// Fetch a puzzle from the server and solve it. The result goes into `create_room.altcha`.
pub async fn fetch_and_solve(
    addr: &ServerAddr,
    trust: &TlsTrust,
) -> Result<String, TransportError> {
    let resp = http_get(addr, "/altcha/challenge", trust).await?;
    if resp.status != 200 {
        return Err(http_status_error(&resp));
    }
    solve_off_thread(resp.body).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use altcha::{CreateChallengeOptions, VerifySolutionOptions, create_challenge};

    const SECRET: &str = "unit-test-secret-0123456789";

    fn make(counter: u32) -> Challenge {
        create_challenge(CreateChallengeOptions {
            counter: Some(counter),
            cost: 50,
            hmac_signature_secret: Some(SECRET.into()),
            hmac_key_signature_secret: Some(format!("{SECRET}/key")),
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn solves_and_the_server_side_check_accepts() {
        let ch = make(321);
        let text = solve_challenge_json(&serde_json::to_vec(&ch).unwrap()).unwrap();
        let payload: Payload = serde_json::from_slice(&BASE64.decode(text).unwrap()).unwrap();
        assert_eq!(payload.solution.counter, 321);
        let v = altcha::verify_solution(VerifySolutionOptions {
            hmac_key_signature_secret: Some(format!("{SECRET}/key")),
            ..VerifySolutionOptions::new(&payload.challenge, &payload.solution, SECRET)
        })
        .unwrap();
        assert!(v.verified);
    }

    /// The same challenge the web app's unit test solves (apps/web/tests/altcha.test.ts):
    /// both implementations must find the same counter and derived key.
    #[test]
    fn known_challenge_fixture() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            challenge: Challenge,
            secret: String,
            key_secret: String,
            counter: u32,
            #[serde(rename = "derivedKey")]
            derived_key: String,
        }
        let f: Fixture = serde_json::from_str(include_str!("../testdata/altcha-known.json"))
            .expect("fixture parses");
        let text = solve_challenge_json(&serde_json::to_vec(&f.challenge).unwrap()).unwrap();
        let payload: Payload = serde_json::from_slice(&BASE64.decode(text).unwrap()).unwrap();
        assert_eq!(payload.solution.counter, f.counter);
        assert_eq!(payload.solution.derived_key, f.derived_key);
        let v = altcha::verify_solution(VerifySolutionOptions {
            hmac_key_signature_secret: Some(f.key_secret),
            ..VerifySolutionOptions::new(&payload.challenge, &payload.solution, f.secret)
        })
        .unwrap();
        assert!(v.verified);
    }

    /// Regenerates `testdata/altcha-known.json` (random nonce and salt, so only run it when the
    /// fixture must change): `cargo test -p cc-transport -- --ignored --nocapture print_fixture`.
    #[test]
    #[ignore = "prints a new fixture"]
    fn print_fixture() {
        let secret = "fixture-secret-0123456789";
        let key_secret = format!("{secret}/key");
        let counter = 777;
        let challenge = create_challenge(CreateChallengeOptions {
            counter: Some(counter),
            cost: 1000,
            expires_at: Some(4_102_444_800), // 2100-01-01
            hmac_signature_secret: Some(secret.into()),
            hmac_key_signature_secret: Some(key_secret.clone()),
            ..Default::default()
        })
        .unwrap();
        let text = solve_challenge_json(&serde_json::to_vec(&challenge).unwrap()).unwrap();
        let payload: Payload = serde_json::from_slice(&BASE64.decode(text).unwrap()).unwrap();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "secret": secret,
                "key_secret": key_secret,
                "counter": payload.solution.counter,
                "derivedKey": payload.solution.derived_key,
                "challenge": challenge,
            }))
            .unwrap()
        );
    }

    #[test]
    fn rejects_garbage_challenge() {
        assert!(matches!(
            solve_challenge_json(b"not json"),
            Err(TransportError::Protocol(_))
        ));
    }

    #[test]
    fn maps_http_statuses() {
        let r = |status, retry_after| HttpResponse {
            status,
            retry_after,
            body: vec![],
        };
        let code = |e: TransportError| match e {
            TransportError::Server { code, .. } => code,
            other => format!("{other}"),
        };
        assert_eq!(code(http_status_error(&r(429, Some(3)))), "rate_limited");
        assert_eq!(code(http_status_error(&r(403, Some(60)))), "banned");
        assert_eq!(code(http_status_error(&r(403, None))), "forbidden");
        assert_eq!(code(http_status_error(&r(503, None))), "server_full");
        assert!(code(http_status_error(&r(500, None))).contains("500"));
    }
}
