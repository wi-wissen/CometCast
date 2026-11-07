// SPDX-License-Identifier: GPL-3.0-or-later
//! The conversion worker process of `cc-office`. Not meant to be started by hand.
//! See `docs/documents.md`.

fn main() -> std::process::ExitCode {
    let code = cc_office::worker_main();
    std::process::ExitCode::from(u8::try_from(code).unwrap_or(1))
}
