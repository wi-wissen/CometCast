// SPDX-License-Identifier: GPL-3.0-or-later
//! Conversion through the real worker process, with generated fixtures.
//!
//! The pure-Rust engine always runs. The LibreOffice tests are **skipped loudly** (a
//! `SKIPPED:` line on stderr, visible with `--nocapture`) where `soffice` is not installed;
//! set `COMETCAST_REQUIRE_OFFICE_ENGINE=1` to turn a skip into a failure (for CI that is
//! supposed to have it).
#![allow(missing_docs)]

mod common {
    pub mod zipwriter;
}
use common::zipwriter as zw;

use std::path::PathBuf;
use std::time::Duration;

use cc_office::{
    Concrete, Engine, Limits, OfficeError, OfficeFormat, Options, WorkerSpec, convert_bytes,
    convert_file, find_soffice,
};

fn worker() -> WorkerSpec {
    WorkerSpec::new(env!("CARGO_BIN_EXE_cc-office-worker"))
}

fn opts(engine: Engine) -> Options {
    Options {
        engine,
        worker: Some(worker()),
        limits: Limits {
            timeout: Duration::from_secs(90),
            ..Limits::default()
        },
        ..Options::default()
    }
}

fn skip_or_fail(what: &str) -> bool {
    if std::env::var_os("COMETCAST_REQUIRE_OFFICE_ENGINE").is_some_and(|v| v == "1") {
        panic!("{what} is required (COMETCAST_REQUIRE_OFFICE_ENGINE=1) but missing");
    }
    eprintln!("SKIPPED: {what} not available; this test did NOT run");
    true
}

fn assert_pdf(c: &cc_office::Converted) {
    assert!(c.pdf.starts_with(b"%PDF-"));
    assert!(c.pdf.len() > 500, "suspiciously small PDF: {}", c.pdf.len());
}

fn fixtures() -> Vec<(OfficeFormat, Vec<u8>)> {
    vec![
        (OfficeFormat::Docx, zw::docx("Hello classroom")),
        (OfficeFormat::Xlsx, zw::xlsx("Hello sheet")),
        (OfficeFormat::Pptx, zw::pptx("Hello slide")),
    ]
}

#[cfg(feature = "engine-rust")]
#[test]
fn rust_engine_converts_the_generated_fixtures() {
    for (f, bytes) in fixtures() {
        let r = convert_bytes(&bytes, f, &opts(Engine::Rust));
        let c = match r {
            Ok(c) => c,
            Err(e) => panic!("{f:?}: {e}"),
        };
        assert_eq!(c.engine, Concrete::Rust);
        assert_pdf(&c);
        eprintln!(
            "rust engine {f:?}: {} bytes in {:?}, protections {:?}, warnings {:?}",
            c.pdf.len(),
            c.elapsed,
            c.protections,
            c.warnings
        );
        // Resource limits are mandatory in the worker, so they must have been reported.
        #[cfg(unix)]
        for p in ["rlimit-cpu", "rlimit-core", "rlimit-fsize"] {
            assert!(c.protections.iter().any(|x| x == p), "{p} missing");
        }
    }
}

#[test]
fn soffice_converts_the_generated_fixtures() {
    if find_soffice(None).is_none() {
        skip_or_fail("LibreOffice (soffice)");
        return;
    }
    for (f, bytes) in fixtures() {
        let c = convert_bytes(&bytes, f, &opts(Engine::Soffice))
            .unwrap_or_else(|e| panic!("{f:?}: {e}"));
        assert_eq!(c.engine, Concrete::Soffice);
        assert_pdf(&c);
        eprintln!(
            "soffice {f:?}: {} bytes in {:?}, protections {:?}",
            c.pdf.len(),
            c.elapsed,
            c.protections
        );
    }
}

#[test]
fn convert_file_reads_from_disk_and_checks_the_extension() {
    let dir = std::env::temp_dir().join(format!("cc-office-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p: PathBuf = dir.join("Klassenarbeit with spaces.DOCX");
    std::fs::write(&p, zw::docx("x")).unwrap();
    // A .docx that is really an xlsx is refused before any engine runs.
    let wrong = dir.join("wrong.docx");
    std::fs::write(&wrong, zw::xlsx("x")).unwrap();
    assert!(matches!(
        convert_file(&wrong, &opts(Engine::Auto)),
        Err(OfficeError::BadContainer(_))
    ));
    let have_engine = find_soffice(None).is_some() || cc_office::rust_engine_compiled();
    if have_engine {
        let c = convert_file(&p, &opts(Engine::Auto)).expect("convert");
        assert_pdf(&c);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_worker_is_an_error_not_a_panic() {
    let o = Options {
        engine: Engine::Auto,
        worker: Some(WorkerSpec::new("/nonexistent/cc-office-worker")),
        ..Options::default()
    };
    if find_soffice(None).is_none() && !cc_office::rust_engine_compiled() {
        return;
    }
    assert!(matches!(
        convert_bytes(&zw::docx("x"), OfficeFormat::Docx, &o),
        Err(OfficeError::WorkerUnavailable(_))
    ));
}

#[cfg(unix)]
#[test]
fn a_hanging_worker_is_killed_at_the_deadline_with_its_children() {
    // A fake "worker" that ignores its arguments, starts a grandchild and sleeps.
    let dir = std::env::temp_dir().join(format!("cc-office-hang-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("hang.sh");
    std::fs::write(&script, "#!/bin/sh\nsleep 600 &\nsleep 600\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let o = Options {
        engine: Engine::Rust,
        worker: Some(WorkerSpec::new(&script)),
        limits: Limits {
            timeout: Duration::from_secs(1),
            ..Limits::default()
        },
        ..Options::default()
    };
    if !cc_office::rust_engine_compiled() {
        return;
    }
    let t = std::time::Instant::now();
    let r = convert_bytes(&zw::docx("x"), OfficeFormat::Docx, &o);
    assert!(matches!(r, Err(OfficeError::Timeout(_))), "{r:?}");
    assert!(
        t.elapsed() < Duration::from_secs(10),
        "took {:?}",
        t.elapsed()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn a_worker_that_writes_a_fake_or_huge_result_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    if !cc_office::rust_engine_compiled() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("cc-office-fake-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // The fake worker finds its work dir in the `--work` argument (after the 8th word).
    let mk = |name: &str, body: &str| {
        let s = dir.join(name);
        let script = format!(
            "#!/bin/sh\nwhile [ $# -gt 0 ]; do [ \"$1\" = \"--work\" ] && W=\"$2\"; shift; done\n{body}\n"
        );
        std::fs::write(&s, script).unwrap();
        std::fs::set_permissions(&s, std::fs::Permissions::from_mode(0o755)).unwrap();
        s
    };
    let run = |s: &PathBuf, cap: u64| {
        let o = Options {
            engine: Engine::Rust,
            worker: Some(WorkerSpec::new(s)),
            limits: Limits {
                max_output_bytes: cap,
                ..Limits::default()
            },
            ..Options::default()
        };
        convert_bytes(&zw::docx("x"), OfficeFormat::Docx, &o)
    };
    let not_pdf = mk("notpdf.sh", "echo hello > \"$W/out/input.pdf\"");
    assert!(matches!(
        run(&not_pdf, 1 << 20),
        Err(OfficeError::BadOutput(_))
    ));
    let nothing = mk("nothing.sh", "true");
    assert!(matches!(
        run(&nothing, 1 << 20),
        Err(OfficeError::BadOutput(_))
    ));
    let big = mk(
        "big.sh",
        "printf '%%PDF-1.7\\n' > \"$W/out/input.pdf\"; head -c 5000 /dev/zero >> \"$W/out/input.pdf\"; printf '%%%%EOF' >> \"$W/out/input.pdf\"",
    );
    assert!(matches!(run(&big, 1000), Err(OfficeError::BadOutput(m)) if m.contains("larger")));
    let failing = mk("fail.sh", "echo boom >&2; exit 3");
    assert!(
        matches!(run(&failing, 1 << 20), Err(OfficeError::ConversionFailed(m)) if m.contains("boom"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hostile_files_never_reach_an_engine() {
    // Even with a worker that does not exist: the container check fails first, so the error
    // is about the container and not about the worker.
    let o = Options {
        engine: Engine::Auto,
        worker: Some(WorkerSpec::new("/nonexistent/cc-office-worker")),
        limits: Limits {
            max_entry_bytes: 4 * 1024 * 1024,
            ..Limits::default()
        },
        ..Options::default()
    };
    let bomb = zw::build(&[
        zw::Entry::deflated("[Content_Types].xml", b"<Types/>"),
        zw::Entry::deflated("word/document.xml", &vec![0u8; 6 * 1024 * 1024]),
    ]);
    assert!(matches!(
        convert_bytes(&bomb, OfficeFormat::Docx, &o),
        Err(OfficeError::TooMuchData(_))
    ));
    let traversal = zw::build(&[
        zw::Entry::deflated("[Content_Types].xml", b"<Types/>"),
        zw::Entry::deflated("word/document.xml", b"<w/>"),
        zw::Entry::stored("../../etc/cron.d/x", b"x"),
    ]);
    assert!(matches!(
        convert_bytes(&traversal, OfficeFormat::Docx, &o),
        Err(OfficeError::BadContainer(_))
    ));
    let corrupt = &zw::docx("x")[..40];
    assert!(matches!(
        convert_bytes(corrupt, OfficeFormat::Docx, &o),
        Err(OfficeError::BadContainer(_))
    ));
}
