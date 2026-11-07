// SPDX-License-Identifier: GPL-3.0-or-later
//! Office files (`.docx .pptx .xlsx`) on the sending side: converted to a PDF by `cc-office`
//! before anything is connected, so the receiver only ever gets a PDF (docs/documents.md,
//! "Office documents").
//!
//! The conversion runs the worker program `cc-office-worker` as a resource-limited child
//! process (it is **not** a sandbox, see docs/documents.md). Where the worker is, in this
//! order: the explicit path of [`set_worker_override`]; a file `cc-office-worker` next to the
//! running executable (where `flutter build linux` puts it); the Cargo test layout (one
//! directory up from `deps`). There is no environment variable and no `PATH` search.
//!
//! The app always asks for the pure-Rust engine ([`Engine::Rust`]): it never starts LibreOffice
//! behind the person's back, because LibreOffice can read any file the user can read when a
//! document links it (docs/documents.md). A build without the pure-Rust engine therefore
//! reports [`UiSendRejection::OfficeUnavailable`].

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use cc_office::{Engine, OfficeError, OfficeFormat, Options, WorkerSpec};

use crate::api::UiSendRejection;

static WORKER_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Set (or with `None` remove) an explicit conversion worker program. For tests and for hosts
/// whose layout differs from "next to the executable".
pub(crate) fn set_worker_override(path: Option<PathBuf>) {
    *lock(&WORKER_OVERRIDE) = path;
}

/// The worker that would be used now, if there is one.
fn spec() -> Option<WorkerSpec> {
    if let Some(p) = lock(&WORKER_OVERRIDE).clone() {
        return p.is_file().then(|| WorkerSpec::new(p));
    }
    WorkerSpec::discover()
}

/// The worker program that would be used now, for display and diagnostics.
pub(crate) fn worker_path() -> Option<String> {
    spec().map(|s| s.program().to_string_lossy().into_owned())
}

/// Options for one conversion. Without a worker the options name a program that does not
/// exist, so the conversion fails with `WorkerUnavailable` (never by searching elsewhere).
pub(crate) fn options() -> Options {
    Options {
        engine: Engine::Rust,
        worker: Some(spec().unwrap_or_else(|| WorkerSpec::new(NO_WORKER_PROGRAM))),
        ..Options::default()
    }
}

const NO_WORKER_PROGRAM: &str = "/nonexistent/cc-office-worker-not-found";

/// `true` for the file types the app converts: `.docx .pptx .xlsx` (the OpenDocument formats
/// need LibreOffice, which the app does not start).
pub(crate) fn is_office_name(name: &str) -> bool {
    matches!(
        OfficeFormat::from_name(name),
        Some(OfficeFormat::Docx | OfficeFormat::Pptx | OfficeFormat::Xlsx)
    )
}

/// The UI reason for a failed conversion. Text of the engine never reaches the UI.
pub(crate) fn map_error(e: &OfficeError) -> UiSendRejection {
    match e {
        OfficeError::UnsupportedFormat => UiSendRejection::UnsupportedType,
        OfficeError::LegacyOrEncrypted => UiSendRejection::OfficeEncrypted,
        OfficeError::Io(_) => UiSendRejection::Unreadable,
        OfficeError::InputTooLarge { .. } => UiSendRejection::TooLarge,
        OfficeError::NotRegularFile => UiSendRejection::NotARegularFile,
        OfficeError::NoEngine(_) | OfficeError::WorkerUnavailable(_) => {
            UiSendRejection::OfficeUnavailable
        }
        OfficeError::BadContainer(_)
        | OfficeError::TooMuchData(_)
        | OfficeError::ConversionFailed(_)
        | OfficeError::Timeout(_)
        | OfficeError::BadOutput(_) => UiSendRejection::OfficeFailed,
    }
}

/// The name that is sent: the file name with its extension replaced by `.pdf`.
pub(crate) fn pdf_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "document".to_owned());
    format!("{stem}.pdf")
}

/// Convert a file. Returns the name to send and the PDF. Blocking (starts a child process, up
/// to the deadline of 120 s): run it on a blocking thread.
pub(crate) fn convert(path: &Path, opts: &Options) -> Result<(String, Vec<u8>), UiSendRejection> {
    let converted = cc_office::convert_file(path, opts).map_err(|e| map_error(&e))?;
    if converted.pdf.len() as u64 > cc_files::MAX_DOCUMENT_BYTES {
        return Err(UiSendRejection::TooLarge);
    }
    Ok((pdf_name(path), converted.pdf))
}

// The ZIP writer of the cc-office tests (a path relative to `src/`).
#[cfg(test)]
#[path = "../../cc-office/tests/common/zipwriter.rs"]
mod zw;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use super::zw;

    #[test]
    fn only_the_three_ooxml_types_are_converted() {
        for n in ["a.docx", "A.PPTX", "b.c.xlsx"] {
            assert!(is_office_name(n), "{n}");
        }
        // OpenDocument needs LibreOffice, legacy formats are not accepted, a bare name is
        // no format.
        for n in [
            "a.odt", "a.odp", "a.ods", "a.doc", "a.xls", "a.ppt", "docx", "a.pdf", "",
        ] {
            assert!(!is_office_name(n), "{n}");
        }
    }

    #[test]
    fn every_office_error_has_a_reason_without_engine_text() {
        use UiSendRejection as R;
        let cases: Vec<(OfficeError, R)> = vec![
            (OfficeError::UnsupportedFormat, R::UnsupportedType),
            (OfficeError::LegacyOrEncrypted, R::OfficeEncrypted),
            (OfficeError::Io("x".into()), R::Unreadable),
            (OfficeError::InputTooLarge { limit: 1 }, R::TooLarge),
            (OfficeError::NotRegularFile, R::NotARegularFile),
            (OfficeError::BadContainer("x".into()), R::OfficeFailed),
            (OfficeError::TooMuchData("x".into()), R::OfficeFailed),
            (OfficeError::NoEngine("x".into()), R::OfficeUnavailable),
            (
                OfficeError::WorkerUnavailable("x".into()),
                R::OfficeUnavailable,
            ),
            (OfficeError::ConversionFailed("x".into()), R::OfficeFailed),
            (
                OfficeError::Timeout(Duration::from_secs(1)),
                R::OfficeFailed,
            ),
            (OfficeError::BadOutput("x".into()), R::OfficeFailed),
        ];
        for (e, want) in cases {
            assert_eq!(map_error(&e), want, "{e}");
        }
    }

    #[test]
    fn the_sent_name_is_the_stem_with_pdf() {
        assert_eq!(pdf_name(Path::new("/x/Report 1.docx")), "Report 1.pdf");
        assert_eq!(pdf_name(Path::new("a.b.pptx")), "a.b.pdf");
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    fn opts_with(worker: &Path) -> Options {
        Options {
            engine: Engine::Rust,
            worker: Some(WorkerSpec::new(worker)),
            ..Options::default()
        }
    }

    #[test]
    fn a_missing_worker_is_unavailable_not_a_broken_file() {
        let d = tempfile::tempdir().unwrap();
        let f = write(d.path(), "a.docx", &zw::docx("hello"));
        let r = convert(&f, &opts_with(&d.path().join("no-such-worker")));
        assert_eq!(r.err(), Some(UiSendRejection::OfficeUnavailable));
    }

    #[test]
    fn broken_and_hostile_files_are_refused_with_a_reason() {
        let d = tempfile::tempdir().unwrap();
        let o = opts_with(&d.path().join("never-started"));
        // Not a ZIP at all.
        let junk = write(d.path(), "a.docx", b"this is not a zip file at all, really");
        assert_eq!(
            convert(&junk, &o).err(),
            Some(UiSendRejection::OfficeFailed)
        );
        // A legacy / password-protected container (OLE signature).
        let mut ole = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        ole.resize(600, 0);
        let ole = write(d.path(), "b.docx", &ole);
        assert_eq!(
            convert(&ole, &o).err(),
            Some(UiSendRejection::OfficeEncrypted)
        );
        // A docx under another Office name: the content must match the extension.
        let wrong = write(d.path(), "c.xlsx", &zw::docx("x"));
        assert_eq!(
            convert(&wrong, &o).err(),
            Some(UiSendRejection::OfficeFailed)
        );
        // A missing file and a folder.
        assert_eq!(
            convert(&d.path().join("gone.docx"), &o).err(),
            Some(UiSendRejection::Unreadable)
        );
        std::fs::create_dir(d.path().join("dir.docx")).unwrap();
        assert_eq!(
            convert(&d.path().join("dir.docx"), &o).err(),
            Some(UiSendRejection::NotARegularFile)
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_worker_result_is_returned_as_a_pdf_with_the_new_name() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        // A fake worker: finds its work directory in `--work` and writes a tiny valid PDF.
        let script = d.path().join("fake-worker.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\nwhile [ $# -gt 0 ]; do [ \"$1\" = \"--work\" ] && W=\"$2\"; shift; done\n\
             printf '%%PDF-1.7\\n%%%%EOF\\n' > \"$W/out/input.pdf\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let f = write(d.path(), "Class notes.docx", &zw::docx("hello"));
        let (name, pdf) = convert(&f, &opts_with(&script)).expect("converted");
        assert_eq!(name, "Class notes.pdf");
        assert!(pdf.starts_with(b"%PDF-"));
        // A worker that fails is a failed conversion.
        let fail = d.path().join("fail.sh");
        std::fs::write(&fail, "#!/bin/sh\nexit 3\n").unwrap();
        std::fs::set_permissions(&fail, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            convert(&f, &opts_with(&fail)).err(),
            Some(UiSendRejection::OfficeFailed)
        );
    }

    #[test]
    fn the_override_decides_where_the_worker_is() {
        let d = tempfile::tempdir().unwrap();
        let w = write(d.path(), "w", b"#!/bin/sh\n");
        set_worker_override(Some(w.clone()));
        assert_eq!(worker_path().as_deref(), w.to_str());
        set_worker_override(Some(d.path().join("missing")));
        assert_eq!(worker_path(), None);
        set_worker_override(None);
    }
}
