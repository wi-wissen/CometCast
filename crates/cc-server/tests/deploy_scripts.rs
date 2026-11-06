// SPDX-License-Identifier: GPL-3.0-or-later
//! Runs the shell tests of the coturn render script (hostile .env values, secrets, ports,
//! addresses) in a temporary directory. Skipped when `sh` is not available (e.g. Windows).

use std::path::Path;
use std::process::Command;

#[test]
fn coturn_render_script_refuses_hostile_input() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deploy/coturn/test-render.sh");
    assert!(script.exists(), "{} is missing", script.display());
    let out = match Command::new("sh").arg(&script).output() {
        Ok(out) => out,
        Err(e) => {
            eprintln!("skipped: cannot run sh: {e}");
            return;
        }
    };
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "render.sh tests failed:\n{text}");
    assert!(text.contains("0 failed"), "{text}");
}
