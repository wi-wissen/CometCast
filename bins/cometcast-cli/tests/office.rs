// SPDX-License-Identifier: GPL-3.0-or-later
//! `office convert` through the real program (and its hidden `office-worker` command), then
//! the result is opened with `docs info`, which is what the receiver's renderer does.
#![allow(missing_docs)]

#[path = "../../../crates/cc-office/tests/common/zipwriter.rs"]
mod zipwriter;

use std::path::PathBuf;
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cometcast-cli"))
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cc-cli-office-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn converts_docx_and_the_pdf_opens_in_the_receiver_renderer() {
    let d = scratch("ok");
    let input = d.join("Arbeitsblatt.docx");
    std::fs::write(&input, zipwriter::docx("Hello")).unwrap();
    let out = d.join("out.pdf");
    let r = cli()
        .args(["office", "convert"])
        .arg(&input)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        r.status.success(),
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    let pdf = std::fs::read(&out).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));

    // Never replaced without --force.
    let again = cli()
        .args(["office", "convert"])
        .arg(&input)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert!(!again.status.success());

    // The receiver's renderer (child process, limits) opens it.
    let info = cli().args(["docs", "info"]).arg(&out).output().unwrap();
    let text = String::from_utf8_lossy(&info.stdout).into_owned();
    assert!(
        info.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&info.stderr)
    );
    assert!(text.contains("pages"), "{text}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn hostile_and_wrong_files_fail_with_a_message_and_write_nothing() {
    let d = scratch("bad");
    let out = d.join("out.pdf");
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("corrupt.docx", zipwriter::docx("x")[..50].to_vec()),
        ("notzip.docx", b"hello".to_vec()),
        ("legacy.docx", {
            let mut v = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
            v.extend_from_slice(&[0u8; 64]);
            v
        }),
        (
            "traversal.docx",
            zipwriter::build(&[
                zipwriter::Entry::deflated("[Content_Types].xml", b"<Types/>"),
                zipwriter::Entry::deflated("word/document.xml", b"<w/>"),
                zipwriter::Entry::stored("../../evil", b"x"),
            ]),
        ),
        (
            "bomb.docx",
            zipwriter::build(&[
                zipwriter::Entry::deflated("[Content_Types].xml", b"<Types/>"),
                // 200 MiB of zeros: over the default 128 MiB entry limit, ~200 KiB packed.
                zipwriter::Entry::deflated("word/document.xml", &vec![0u8; 200 * 1024 * 1024]),
            ]),
        ),
        ("wrongext.pdf", zipwriter::docx("x")),
        ("opendocument.odt", zipwriter::docx("x")),
    ];
    for (name, bytes) in cases {
        let f = d.join(name);
        std::fs::write(&f, bytes).unwrap();
        let r = cli()
            .args(["office", "convert"])
            .arg(&f)
            .arg("--out")
            .arg(&out)
            .output()
            .unwrap();
        assert!(!r.status.success(), "{name} was accepted");
        let err = String::from_utf8_lossy(&r.stderr);
        assert!(err.contains("error:"), "{name}: {err}");
        assert!(!out.exists(), "{name} left an output");
    }
    let _ = std::fs::remove_dir_all(&d);
}
