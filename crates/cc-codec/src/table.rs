// SPDX-License-Identifier: GPL-3.0-or-later
//! The OpenH264 files this program accepts, per platform.
//!
//! The names and SHA-256 values are copied from `src/blobs/hashes.txt` of the `openh264-sys2`
//! crate 0.9.9 (the version in Cargo.lock), the list that the `openh264` crate checks before it
//! loads a library in `libloading` mode. That crate pins OpenH264 2.6.0 (`.8` build of the
//! Linux and Android files). The `compressed_bytes` values are the sizes of Cisco's `.bz2`
//! files measured on 2026-11-07 (informational: shown to the user, not a limit).
//!
//! Updating: when `openh264`/`openh264-sys2` is updated in Cargo.lock, copy the new
//! `hashes.txt` lines here. This stays a manual step, listed in docs/releasing.md.

/// Version of Cisco's OpenH264 that the table describes.
pub const OPENH264_VERSION: &str = "2.6.0";

/// Host Cisco serves the files from. Only HTTPS is used.
pub const CISCO_HOST: &str = "ciscobinary.openh264.org";

/// One library file, as Cisco publishes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Artifact {
    /// File name of the library, also the name Cisco publishes it under (plus `.bz2`).
    pub name: &'static str,
    /// SHA-256 of the decompressed library, lower-case hex.
    pub sha256: &'static str,
    /// Size of Cisco's `.bz2` file in bytes (for display only).
    pub compressed_bytes: u64,
}

impl Artifact {
    /// URL of Cisco's compressed file.
    pub fn url(&self) -> String {
        format!("https://{CISCO_HOST}/{}.bz2", self.name)
    }
}

const LINUX_X64: Artifact = Artifact {
    name: "libopenh264-2.6.0-linux64.8.so",
    sha256: "2f0cde7c6a6abcf5cae76942894ea42897fa677bce4ed6c91a24dd1b041d5f04",
    compressed_bytes: 634_264,
};
const LINUX_ARM64: Artifact = Artifact {
    name: "libopenh264-2.6.0-linux-arm64.8.so",
    sha256: "12e7b33623667cdab0e575170c147b1b36eadb77d0d2aa7ceb5afd3e58902140",
    compressed_bytes: 619_371,
};
const WINDOWS_X64: Artifact = Artifact {
    name: "openh264-2.6.0-win64.dll",
    sha256: "2076cb5675ec6c1a4c70e7a2a322552f547b6eeed649d6dfcd9e02a543b24691",
    compressed_bytes: 452_053,
};
const WINDOWS_ARM64: Artifact = Artifact {
    name: "openh264-2.6.0-win-arm64.dll",
    sha256: "fb75103938f4f47d119b983e06334df41a803bc72fb5c46e3623f6fea5782732",
    compressed_bytes: 378_234,
};
const MAC_ARM64: Artifact = Artifact {
    name: "libopenh264-2.6.0-mac-arm64.dylib",
    sha256: "052e98bfcf7a9167d22f3bbb3f5988ef79065591f36af8b52924b22b13624551",
    compressed_bytes: 482_124,
};
const MAC_X64: Artifact = Artifact {
    name: "libopenh264-2.6.0-mac-x64.dylib",
    sha256: "e3dc8bc01fe69363f61fd3c02fd27798537a585eadd38cd808f303d1ee505a19",
    compressed_bytes: 529_614,
};
const ANDROID_ARM: Artifact = Artifact {
    name: "libopenh264-2.6.0-android-arm.8.so",
    sha256: "45d8d77c119a488cad42549987a4a1ac50f634cfd6b08454c5565652e482022c",
    compressed_bytes: 540_728,
};
const ANDROID_ARM64: Artifact = Artifact {
    name: "libopenh264-2.6.0-android-arm64.8.so",
    sha256: "4d9bc54d2d38e53eb7bd551ec61acb8ad8320d8b957bda751cfdbdcbfabc3b07",
    compressed_bytes: 496_662,
};
const ANDROID_X64: Artifact = Artifact {
    name: "libopenh264-2.6.0-android-x64.8.so",
    sha256: "548fa57935ff0eaf0a1d44c7963c1681daa5bf4cef2b34029fcd20a1e71adf81",
    compressed_bytes: 562_893,
};

/// Every file in the table.
pub const ALL: [Artifact; 9] = [
    LINUX_X64,
    LINUX_ARM64,
    WINDOWS_X64,
    WINDOWS_ARM64,
    MAC_ARM64,
    MAC_X64,
    ANDROID_ARM,
    ANDROID_ARM64,
    ANDROID_X64,
];

/// The file for an operating system and CPU, named as Rust's `target_os` and `target_arch`
/// (`"linux"`, `"windows"`, `"macos"`, `"android"`; `"x86_64"`, `"aarch64"`, `"arm"`).
/// `None`: Cisco has no file for it (or this program does not offer it), for example
/// 32-bit x86 Linux, iOS (there the library cannot be loaded at run time at all), FreeBSD.
pub fn artifact_for(os: &str, arch: &str) -> Option<Artifact> {
    Some(match (os, arch) {
        ("linux", "x86_64") => LINUX_X64,
        ("linux", "aarch64") => LINUX_ARM64,
        ("windows", "x86_64") => WINDOWS_X64,
        ("windows", "aarch64") => WINDOWS_ARM64,
        ("macos", "aarch64") => MAC_ARM64,
        ("macos", "x86_64") => MAC_X64,
        ("android", "arm") => ANDROID_ARM,
        ("android", "aarch64") => ANDROID_ARM64,
        ("android", "x86_64") => ANDROID_X64,
        _ => return None,
    })
}

/// The file for the platform this program runs on (chosen at compile time by `cfg`: a macOS
/// program built for arm64 gets the arm64 library, one built for x86_64 the x64 one).
pub fn current_artifact() -> Option<Artifact> {
    artifact_for(std::env::consts::OS, std::env::consts::ARCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_table() {
        assert_eq!(
            artifact_for("linux", "x86_64").unwrap().name,
            "libopenh264-2.6.0-linux64.8.so"
        );
        assert_eq!(
            artifact_for("windows", "aarch64").unwrap().name,
            "openh264-2.6.0-win-arm64.dll"
        );
        assert_eq!(
            artifact_for("macos", "aarch64").unwrap().name,
            "libopenh264-2.6.0-mac-arm64.dylib"
        );
        assert_eq!(
            artifact_for("macos", "x86_64").unwrap().name,
            "libopenh264-2.6.0-mac-x64.dylib"
        );
        assert_eq!(
            artifact_for("android", "arm").unwrap().name,
            "libopenh264-2.6.0-android-arm.8.so"
        );
        assert!(artifact_for("linux", "x86").is_none());
        assert!(artifact_for("ios", "aarch64").is_none());
        assert!(artifact_for("freebsd", "x86_64").is_none());
    }

    #[test]
    fn entries_are_well_formed_and_unique() {
        for a in ALL {
            assert_eq!(a.sha256.len(), 64, "{}", a.name);
            assert!(
                a.sha256
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            );
            assert!(a.name.contains(OPENH264_VERSION));
            assert!(!a.name.contains(['/', '\\']));
            assert!(a.compressed_bytes < 4 * 1024 * 1024);
            assert_eq!(a.url(), format!("https://{CISCO_HOST}/{}.bz2", a.name));
        }
        for (i, a) in ALL.iter().enumerate() {
            for b in &ALL[i + 1..] {
                assert_ne!(a.name, b.name);
                assert_ne!(a.sha256, b.sha256);
            }
        }
    }

    /// The directory of the `openh264-sys2` crate in this build's dependency graph, found with
    /// `cargo metadata`. `None` if it cannot be found (no cargo, no registry sources).
    fn sys2_dir() -> Option<std::path::PathBuf> {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let out = std::process::Command::new(cargo)
            .args(["metadata", "--format-version", "1", "--locked", "--offline"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let meta: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        let pkg = meta["packages"]
            .as_array()?
            .iter()
            .find(|p| p["name"] == "openh264-sys2")?;
        let manifest = std::path::Path::new(pkg["manifest_path"].as_str()?);
        Some(manifest.parent()?.to_path_buf())
    }

    /// The table must agree with the list that the `openh264` crate checks when it loads a
    /// library: otherwise a file that passes our check would be refused at load time (or the
    /// other way round). Skips, loudly, if the crate's sources are not on this machine.
    #[test]
    fn table_matches_openh264_sys2_hashes_txt() {
        let Some(dir) = sys2_dir() else {
            eprintln!("SKIPPED: cannot find openh264-sys2 with `cargo metadata`");
            return;
        };
        let file = dir.join("src").join("blobs").join("hashes.txt");
        let Ok(text) = std::fs::read_to_string(&file) else {
            eprintln!("SKIPPED: {} cannot be read", file.display());
            return;
        };
        // Lines are `<sha256> <*| >name`; `*` marks binary mode in sha256sum files.
        let theirs: std::collections::HashMap<&str, &str> = text
            .lines()
            .filter_map(|l| {
                let (hash, name) = l.split_once(' ')?;
                Some((name.trim_start_matches([' ', '*']), hash))
            })
            .collect();
        assert!(theirs.len() > 9, "hashes.txt looks empty: {}", theirs.len());
        for a in ALL {
            assert_eq!(
                theirs.get(a.name).copied(),
                Some(a.sha256),
                "{} differs from (or is missing in) hashes.txt of openh264-sys2",
                a.name
            );
        }
    }
}
