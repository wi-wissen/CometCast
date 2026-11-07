// SPDX-License-Identifier: GPL-3.0-or-later
//! `cometcast-cli codec` as a program. The download itself is not tried here (no network in
//! tests); what is checked is the dispatch, the consent rule and the lookup.

use std::process::{Command, Output};

fn cli(args: &[&str], envs: &[(&str, &std::path::Path)]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_cometcast-cli"));
    c.args(args)
        .env_remove("COMETCAST_OPENH264_LIB")
        .env_remove("COMETCAST_CODEC_DIR")
        .env_remove("COMETCAST_CODEC_AUTO");
    for (k, v) in envs {
        c.env(k, v);
    }
    c.output().unwrap()
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn help_and_bad_arguments() {
    let o = cli(&["codec", "--help"], &[]);
    assert!(o.status.success());
    assert!(text(&o).contains("codec install"));
    let o = cli(&["codec", "frobnicate"], &[]);
    assert_eq!(o.status.code(), Some(2));
}

/// Without the release feature the command exists but says why it does nothing.
#[cfg(not(feature = "h264-libloading"))]
#[test]
fn source_builds_explain_themselves() {
    let o = cli(&["codec", "status"], &[]);
    assert_eq!(o.status.code(), Some(2));
    assert!(text(&o).contains("does not download a codec"));
}

#[cfg(feature = "h264-libloading")]
mod release {
    use super::*;

    #[test]
    fn status_missing_then_ready_with_the_env_file() {
        let t = tempfile::tempdir().unwrap();
        let data = t.path().join("data");
        let o = cli(&["codec", "status"], &[("XDG_DATA_HOME", &data)]);
        assert_eq!(o.status.code(), Some(1), "{}", text(&o));
        assert!(text(&o).contains("missing"), "{}", text(&o));
        let lib = t.path().join("libfake.so");
        std::fs::write(&lib, b"x").unwrap();
        let o = cli(
            &["codec", "status"],
            &[("XDG_DATA_HOME", &data), ("COMETCAST_OPENH264_LIB", &lib)],
        );
        assert!(o.status.success(), "{}", text(&o));
        assert!(text(&o).contains("ready:"), "{}", text(&o));
        // A path with no file behind it is not "ready".
        let o = cli(
            &["codec", "status"],
            &[
                ("XDG_DATA_HOME", &data),
                ("COMETCAST_OPENH264_LIB", &t.path().join("nope.so")),
            ],
        );
        assert_eq!(o.status.code(), Some(1), "{}", text(&o));
        assert!(text(&o).contains("does not exist"), "{}", text(&o));
    }

    #[test]
    fn install_without_agreement_does_not_download() {
        let t = tempfile::tempdir().unwrap();
        let o = cli(
            &["codec", "install"],
            &[("XDG_DATA_HOME", &t.path().join("data"))],
        );
        assert_eq!(o.status.code(), Some(2), "{}", text(&o));
        let out = text(&o);
        assert!(
            out.contains("ciscobinary.openh264.org") && out.contains("--yes"),
            "{out}"
        );
        // Nothing was created.
        assert!(!t.path().join("data").exists());
    }

    #[test]
    fn install_with_yes_uses_what_is_already_there() {
        let t = tempfile::tempdir().unwrap();
        let lib = t.path().join("libfake.so");
        std::fs::write(&lib, b"x").unwrap();
        let o = cli(
            &["codec", "install", "--yes"],
            &[
                ("XDG_DATA_HOME", &t.path().join("data")),
                ("COMETCAST_OPENH264_LIB", &lib),
            ],
        );
        assert!(o.status.success(), "{}", text(&o));
        assert!(text(&o).contains("ready:"));
    }
}
