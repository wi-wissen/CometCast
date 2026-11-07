// SPDX-License-Identifier: GPL-3.0-or-later
//! The conversion engines and how LibreOffice is found.

use std::path::{Path, PathBuf};

use crate::format::OfficeFormat;

/// Environment variable that names the LibreOffice program (`soffice`). Must be an absolute
/// path to a regular file. Read by the **caller's** process only (the worker never reads it).
pub const SOFFICE_ENV: &str = "COMETCAST_SOFFICE";

/// Which engine converts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Engine {
    /// The pure-Rust engine for `.docx .pptx .xlsx` when it is compiled in; LibreOffice for
    /// OpenDocument files, and for OOXML only when the pure-Rust engine is not available.
    /// See [`Engine::order`] for the exact rule.
    #[default]
    Auto,
    /// Only the pure-Rust engine (`office2pdf`): `.docx .pptx .xlsx`, no installation.
    Rust,
    /// Only a locally installed LibreOffice: also `.odt .odp .ods`.
    Soffice,
}

/// A concrete engine for one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Concrete {
    /// `office2pdf`.
    Rust,
    /// LibreOffice.
    Soffice,
}

impl Concrete {
    /// Name used on the worker command line and in messages.
    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Soffice => "soffice",
        }
    }
}

impl Engine {
    /// Parse `auto`, `rust`, `soffice` (also `libreoffice`).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "rust" | "office2pdf" => Some(Self::Rust),
            "soffice" | "libreoffice" => Some(Self::Soffice),
            _ => None,
        }
    }

    /// The engines to try, in order.
    ///
    /// `Auto`: the pure-Rust engine for the OOXML formats when it is compiled in, because it
    /// runs inside the worker with no helper processes, and a much smaller feature set than
    /// LibreOffice, which can pull any file the user can read into the PDF through linked
    /// content (see docs/documents.md; UNCERTAIN whether `office2pdf` follows any links: not
    /// checked); LibreOffice for `.odt .odp .ods` and as the only choice when the
    /// pure-Rust engine is not compiled in. `--engine soffice` asks for LibreOffice
    /// explicitly. UNCERTAIN: this order is a security judgement; layout fidelity of the two
    /// engines on real classroom files has not been compared. A failure of the first engine is
    /// not retried with the second unless the first one could not be *started* (not installed,
    /// feature off): a document that one engine rejects as broken is not handed to another
    /// parser automatically.
    pub fn order(self, format: OfficeFormat, soffice: bool, rust: bool) -> Vec<Concrete> {
        let mut v = Vec::new();
        match self {
            Self::Rust => {
                if rust && format.rust_engine_supports() {
                    v.push(Concrete::Rust);
                }
            }
            Self::Soffice => {
                if soffice {
                    v.push(Concrete::Soffice);
                }
            }
            Self::Auto => {
                if rust && format.rust_engine_supports() {
                    v.push(Concrete::Rust);
                } else if soffice {
                    v.push(Concrete::Soffice);
                }
            }
        }
        v
    }
}

/// Whether the pure-Rust engine was compiled in.
pub const fn rust_engine_compiled() -> bool {
    cfg!(feature = "engine-rust")
}

fn is_executable_file(p: &Path) -> bool {
    let Ok(m) = std::fs::metadata(p) else {
        return false;
    };
    if !m.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        m.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Find LibreOffice's `soffice`: `explicit`, then [`SOFFICE_ENV`], then the usual install
/// places, then `PATH`. The result is an absolute path to a regular executable file.
///
/// UNCERTAIN: only the Linux locations were tried; the macOS and Windows paths are the
/// documented defaults and were not run. Flatpak and Snap installs are not looked for (their
/// sandboxes would need their own invocation).
pub fn find_soffice(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return (p.is_absolute() && is_executable_file(p)).then(|| p.to_path_buf());
    }
    if let Some(v) = std::env::var_os(SOFFICE_ENV).filter(|v| !v.is_empty()) {
        let p = PathBuf::from(v);
        return (p.is_absolute() && is_executable_file(&p)).then_some(p);
    }
    let fixed = [
        "/usr/bin/soffice",
        "/usr/local/bin/soffice",
        "/usr/lib/libreoffice/program/soffice",
        "/opt/libreoffice/program/soffice",
        "/Applications/LibreOffice.app/Contents/MacOS/soffice",
        "C:\\Program Files\\LibreOffice\\program\\soffice.exe",
        "C:\\Program Files (x86)\\LibreOffice\\program\\soffice.exe",
    ];
    for f in fixed {
        let p = PathBuf::from(f);
        if is_executable_file(&p) {
            return Some(p);
        }
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in ["soffice", "libreoffice"] {
            let p = dir.join(name);
            if p.is_absolute() && is_executable_file(&p) {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_prefers_rust_for_ooxml_and_soffice_for_odf() {
        use OfficeFormat::*;
        // OOXML: the pure-Rust engine, even when LibreOffice is installed.
        assert_eq!(Engine::Auto.order(Docx, true, true), vec![Concrete::Rust]);
        assert_eq!(Engine::Auto.order(Xlsx, false, true), vec![Concrete::Rust]);
        // No pure-Rust engine compiled in: LibreOffice if there, else nothing.
        assert_eq!(
            Engine::Auto.order(Pptx, true, false),
            vec![Concrete::Soffice]
        );
        assert!(Engine::Auto.order(Pptx, false, false).is_empty());
        // OpenDocument: LibreOffice only.
        assert!(Engine::Auto.order(Odt, false, true).is_empty());
        assert_eq!(Engine::Auto.order(Odt, true, true), vec![Concrete::Soffice]);
        assert_eq!(
            Engine::Auto.order(Ods, true, false),
            vec![Concrete::Soffice]
        );
        // Explicit choices.
        assert_eq!(
            Engine::Soffice.order(Docx, true, true),
            vec![Concrete::Soffice]
        );
        assert!(Engine::Rust.order(Odt, true, true).is_empty());
        assert!(Engine::Rust.order(Docx, true, false).is_empty());
        assert!(Engine::Soffice.order(Docx, false, true).is_empty());
    }

    #[test]
    fn engine_names_parse() {
        assert_eq!(Engine::parse("AUTO"), Some(Engine::Auto));
        assert_eq!(Engine::parse("libreoffice"), Some(Engine::Soffice));
        assert_eq!(Engine::parse("rust"), Some(Engine::Rust));
        assert_eq!(Engine::parse("word"), None);
    }

    #[test]
    fn explicit_relative_path_is_refused() {
        assert!(find_soffice(Some(Path::new("soffice"))).is_none());
    }
}
