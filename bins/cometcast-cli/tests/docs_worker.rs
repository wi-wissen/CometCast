// SPDX-License-Identifier: GPL-3.0-or-later
//! `docs info` / `docs render` through the CLI's own hidden `docs-worker` command, with no
//! `COMETCAST_DOCS_WORKER`. Regression: the worker used to run inside the tokio runtime of
//! `main`, and its seccomp filter killed the runtime teardown (eventfd), so every call timed
//! out. See docs/documents.md.

use std::path::PathBuf;
use std::process::Command;

fn pdf() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/cc-docs/tests/data/minimal.pdf")
}

fn cli() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_cometcast-cli"));
    c.env_remove("COMETCAST_DOCS_WORKER");
    c
}

#[test]
fn info_uses_own_worker_and_reports_protections() {
    let out = cli().args(["docs", "info"]).arg(pdf()).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("pages: 1"), "{stdout}");
    let line = stdout
        .lines()
        .find(|l| l.starts_with("protections: "))
        .unwrap_or_else(|| panic!("no protections line: {stdout}"));
    assert_ne!(line, "protections: none", "worker sandbox not applied");
    #[cfg(target_os = "linux")]
    assert!(line.contains("seccomp"), "{line}");
}

#[test]
fn render_uses_own_worker() {
    let dir = std::env::temp_dir().join(format!("cc-cli-docs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("p.png");
    let out = cli()
        .args(["docs", "render"])
        .arg(pdf())
        .args(["--page", "1", "--width", "200", "--out"])
        .arg(&png)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bytes = std::fs::read(&png).unwrap();
    assert!(bytes.starts_with(b"\x89PNG"), "not a PNG");
    let _ = std::fs::remove_dir_all(&dir);
}
