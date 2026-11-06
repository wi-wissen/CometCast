// SPDX-License-Identifier: GPL-3.0-or-later
//! The render worker process of `cc-docs`: reads one request from stdin and writes one reply
//! to stdout. Not meant to be started by hand. See `docs/documents.md`.

fn main() -> std::process::ExitCode {
    let code = cc_docs::worker_main();
    std::process::ExitCode::from(u8::try_from(code).unwrap_or(1))
}
