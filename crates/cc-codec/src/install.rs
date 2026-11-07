// SPDX-License-Identifier: GPL-3.0-or-later
//! Download, decompress, verify and install the library.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use crate::store::{self, Format};
use crate::{
    Artifact, CodecError, CodecStatus, DownloadGuard, LIBRARY_ENV, MAX_COMPRESSED_BYTES,
    MAX_LIBRARY_BYTES, admin_dir, check_admin_dir, codec_status, current_artifact, env_path,
    find_library, set_progress, user_dir,
};

/// Where and how to download. The only values used outside tests are [`Endpoint::cisco`].
pub(crate) struct Endpoint {
    /// Everything before the file name, e.g. `https://ciscobinary.openh264.org`.
    base_url: String,
    /// Refuse plain `http://` (always `true` outside tests).
    https_only: bool,
    /// Ignore proxy settings of the environment (tests talk to localhost).
    no_proxy: bool,
    /// Extra attempts after the first one.
    retries: u32,
    /// Pause before the first retry, doubled for each further one.
    backoff: Duration,
    /// Limit for one attempt in total (connecting, headers and the whole body).
    total_timeout: Duration,
    /// An attempt fails when no data arrives for this long. Slow links can finish as long as
    /// they make progress.
    stall_timeout: Duration,
}

impl Endpoint {
    fn cisco() -> Self {
        Self {
            base_url: format!("https://{}", crate::CISCO_HOST),
            https_only: true,
            no_proxy: false,
            retries: 3,
            backoff: Duration::from_secs(2),
            total_timeout: Duration::from_secs(10 * 60),
            stall_timeout: Duration::from_secs(30),
        }
    }
}

/// Make the codec available: use what is there, else import the administrator's file (a
/// `.bz2` as Cisco serves it), else download it from Cisco. Blocking; call it from a worker
/// thread. Progress and the result are visible through [`codec_status`]. If another download
/// already runs (this process), it returns at once with [`CodecStatus::Downloading`].
///
/// A rejected file in the administrator folder (wrong hash, too large, not a library) does not
/// stop the download from Cisco; a wrong administrator folder setting (relative, unsafe) does.
///
/// Call it only after the user agreed, or when [`crate::auto_download_policy`] is true.
pub fn ensure_codec() -> CodecStatus {
    if find_library().is_some() {
        return codec_status();
    }
    if env_path(LIBRARY_ENV).is_some() {
        // The developer's explicit file is missing: downloading would not change that.
        return codec_status();
    }
    let Some(guard) = DownloadGuard::begin() else {
        return codec_status();
    };
    let result = run(&Endpoint::cisco());
    guard.finish(result.err().map(|e| e.to_string()));
    codec_status()
}

fn run(ep: &Endpoint) -> Result<PathBuf, CodecError> {
    let art = current_artifact().ok_or_else(|| {
        CodecError::Unsupported(format!(
            "{} / {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ))
    })?;
    let dir = user_dir().ok_or(CodecError::NoDataDir)?;
    run_in(ep, &art, admin_dir(), &dir)
}

/// [`run`] with the inputs given.
pub(crate) fn run_in(
    ep: &Endpoint,
    art: &Artifact,
    admin: Option<PathBuf>,
    dir: &Path,
) -> Result<PathBuf, CodecError> {
    let mut admin_error = None;
    if let Some(admin) = admin {
        match import_from_dir(art, &admin, dir)? {
            Imported::Done(p) => return Ok(p),
            Imported::Rejected(e) => admin_error = Some(e),
            Imported::Nothing => {}
        }
    }
    install_from_network(ep, art, dir, &mut set_progress).map_err(|e| match admin_error {
        Some(a) => CodecError::AfterAdminError {
            admin: a.to_string(),
            source: Box::new(e),
        },
        None => e,
    })
}

/// Result of [`import_from_dir`] when the folder itself is fine.
#[derive(Debug)]
pub(crate) enum Imported {
    /// The verified copy in the per-user directory.
    Done(PathBuf),
    /// No file of the right name.
    Nothing,
    /// There was a file but it is not usable; the download may still work.
    Rejected(CodecError),
}

/// Import Cisco's `.bz2` file (or a raw file) that an administrator copied into `from`: the
/// bytes are checked and copied into `to`, which is where the library is loaded from (never
/// from `from`). `Err` only for a wrong folder (relative path, owner, mode) or for failing
/// to write `to`; a bad file is [`Imported::Rejected`].
pub(crate) fn import_from_dir(
    art: &Artifact,
    from: &Path,
    to: &Path,
) -> Result<Imported, CodecError> {
    check_admin_dir(from)?;
    let mut rejected = None;
    for (name, cap) in [
        (format!("{}.bz2", art.name), MAX_COMPRESSED_BYTES),
        (art.name.to_string(), MAX_LIBRARY_BYTES),
    ] {
        let path = from.join(&name);
        if !fs::metadata(&path).is_ok_and(|m| m.is_file()) {
            continue;
        }
        let read = store::check_trusted(&path)
            .and_then(|()| store::read_bounded(&path, false, cap))
            .and_then(|bytes| decode(&bytes))
            .and_then(|lib| store::check_hash(&lib, art.sha256).map(|()| lib));
        match read {
            Ok(lib) => {
                let p = store::install_verified(to, art.name, art.sha256, &lib)?;
                return Ok(Imported::Done(p));
            }
            Err(e) => {
                rejected.get_or_insert(e);
            }
        }
    }
    Ok(rejected.map_or(Imported::Nothing, Imported::Rejected))
}

/// Download with retries, decode, verify, install.
pub(crate) fn install_from_network(
    ep: &Endpoint,
    art: &Artifact,
    dir: &Path,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<PathBuf, CodecError> {
    let mut attempt = 0;
    let bytes = loop {
        match fetch(ep, art, progress) {
            Ok(b) => break b,
            Err(e) if e.is_retryable() && attempt < ep.retries => {
                std::thread::sleep(ep.backoff * 2u32.pow(attempt));
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    };
    let lib = decode(&bytes)?;
    store::install_verified(dir, art.name, art.sha256, &lib)
}

/// One download attempt: the compressed bytes, at most [`MAX_COMPRESSED_BYTES`].
///
/// The body is read by a helper thread and handed over in pieces; the caller waits at most
/// `stall_timeout` for the next piece (no progress for that long = failure), while the
/// agent's own limit `total_timeout` ends the helper (and the attempt) in any case.
fn fetch(
    ep: &Endpoint,
    art: &Artifact,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Vec<u8>, CodecError> {
    let url = format!("{}/{}.bz2", ep.base_url.trim_end_matches('/'), art.name);
    let mut cfg = ureq::Agent::config_builder()
        .https_only(ep.https_only)
        // A redirect could lead to another host. Cisco answers directly; anything else fails.
        .max_redirects(0)
        .max_redirects_will_error(false)
        .http_status_as_error(false)
        .timeout_global(Some(ep.total_timeout))
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(ep.stall_timeout))
        .user_agent(format!("CometCast-codec/{}", env!("CARGO_PKG_VERSION")));
    if ep.no_proxy {
        cfg = cfg.proxy(None);
    }
    let agent = ureq::Agent::new_with_config(cfg.build());
    let resp = agent
        .get(&url)
        .call()
        .map_err(|e| CodecError::Network(e.to_string()))?;
    let status = resp.status().as_u16();
    if status != 200 {
        return Err(CodecError::Http(status));
    }
    let announced = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if announced.is_some_and(|n| n > MAX_COMPRESSED_BYTES) {
        return Err(CodecError::TooLarge);
    }
    let total = announced.unwrap_or(art.compressed_bytes);
    let (tx, rx) = mpsc::sync_channel::<std::io::Result<Vec<u8>>>(4);
    let mut reader = resp.into_body().into_reader();
    std::thread::spawn(move || {
        let mut buf = [0u8; 16 * 1024];
        loop {
            let msg = match reader.read(&mut buf) {
                Ok(0) => return,
                Ok(n) => Ok(buf[..n].to_vec()),
                Err(e) => Err(e),
            };
            let failed = msg.is_err();
            // The receiver is gone after a stall or an error: stop.
            if tx.send(msg).is_err() || failed {
                return;
            }
        }
    });
    let mut out = Vec::with_capacity(total.min(MAX_COMPRESSED_BYTES) as usize);
    progress(0, total);
    loop {
        let chunk = match rx.recv_timeout(ep.stall_timeout) {
            Ok(Ok(chunk)) => chunk,
            Ok(Err(e)) => return Err(CodecError::Network(e.to_string())),
            // The helper ended: the body is complete.
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(CodecError::Network(format!(
                    "the download stalled (no data for {} s)",
                    ep.stall_timeout.as_secs()
                )));
            }
        };
        if out.len() as u64 + chunk.len() as u64 > MAX_COMPRESSED_BYTES {
            return Err(CodecError::TooLarge);
        }
        out.extend_from_slice(&chunk);
        progress(out.len() as u64, total.max(out.len() as u64));
    }
    if announced.is_some_and(|n| n != out.len() as u64) {
        return Err(CodecError::Network("download ended early".into()));
    }
    Ok(out)
}

/// Turn what was downloaded or copied into the library bytes: decompress bzip2, pass a native
/// library through, reject anything else. The output is at most [`MAX_LIBRARY_BYTES`].
pub(crate) fn decode(bytes: &[u8]) -> Result<Vec<u8>, CodecError> {
    match store::sniff(bytes) {
        Format::Native => {
            if bytes.len() as u64 > MAX_LIBRARY_BYTES {
                return Err(CodecError::TooLarge);
            }
            Ok(bytes.to_vec())
        }
        Format::Bzip2 => {
            let reader = bzip2_rs::DecoderReader::new(bytes);
            let mut out = Vec::new();
            reader
                .take(MAX_LIBRARY_BYTES + 1)
                .read_to_end(&mut out)
                .map_err(|e| CodecError::BadFormat(format!("bzip2: {e}")))?;
            if out.len() as u64 > MAX_LIBRARY_BYTES {
                return Err(CodecError::TooLarge);
            }
            // The decompressed data must be a library too (the hash decides in the end).
            if store::sniff(&out) != Format::Native {
                return Err(CodecError::BadFormat("not a native library".into()));
            }
            Ok(out)
        }
        Format::Unknown => Err(CodecError::BadFormat(
            "neither bzip2 nor a native library".into(),
        )),
    }
}

#[cfg(test)]
mod tests;
