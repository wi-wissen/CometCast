// SPDX-License-Identifier: GPL-3.0-or-later
//! Download, decompress, verify and install the library.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::store::{self, Format};
use crate::{
    Artifact, CodecError, CodecStatus, MAX_COMPRESSED_BYTES, MAX_LIBRARY_BYTES, admin_dir,
    begin_download, codec_status, current_artifact, end_download, find_library, set_progress,
    user_dir,
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
    /// Limit for one attempt.
    timeout: Duration,
}

impl Endpoint {
    fn cisco() -> Self {
        Self {
            base_url: format!("https://{}", crate::CISCO_HOST),
            https_only: true,
            no_proxy: false,
            retries: 3,
            backoff: Duration::from_secs(2),
            timeout: Duration::from_secs(120),
        }
    }
}

/// Make the codec available: use what is there, else import the administrator's file (a
/// `.bz2` as Cisco serves it), else download it from Cisco. Blocking; call it from a worker
/// thread. Progress and the result are visible through [`codec_status`].
///
/// Call it only after the user agreed, or when [`crate::auto_download_policy`] is true.
pub fn ensure_codec() -> CodecStatus {
    if find_library().is_some() {
        return codec_status();
    }
    if !begin_download() {
        return codec_status();
    }
    let result = run(&Endpoint::cisco());
    end_download(result.err().map(|e| e.to_string()));
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
    if let Some(admin) = admin_dir()
        && let Some(p) = import_from_dir(&art, &admin, &dir)?
    {
        return Ok(p);
    }
    install_from_network(ep, &art, &dir, &mut set_progress)
}

/// Import Cisco's `.bz2` file (or a raw file) that an administrator copied into `from`.
/// `Ok(None)` if there is no such file.
pub(crate) fn import_from_dir(
    art: &Artifact,
    from: &Path,
    to: &Path,
) -> Result<Option<PathBuf>, CodecError> {
    for name in [format!("{}.bz2", art.name), art.name.to_string()] {
        let path = from.join(&name);
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        if meta.len() > MAX_COMPRESSED_BYTES {
            return Err(CodecError::TooLarge);
        }
        let bytes = fs::read(&path)?;
        let lib = decode(&bytes)?;
        store::check_hash(&lib, art.sha256)?;
        return store::install_atomic(to, art.name, &lib).map(Some);
    }
    Ok(None)
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
    store::check_hash(&lib, art.sha256)?;
    store::install_atomic(dir, art.name, &lib)
}

/// One download attempt: the compressed bytes, at most [`MAX_COMPRESSED_BYTES`].
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
        .timeout_global(Some(ep.timeout))
        .timeout_connect(Some(Duration::from_secs(15)))
        .user_agent(format!("CometCast-codec/{}", env!("CARGO_PKG_VERSION")));
    if ep.no_proxy {
        cfg = cfg.proxy(None);
    }
    let agent = ureq::Agent::new_with_config(cfg.build());
    let mut resp = agent
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
    let mut reader = resp.body_mut().as_reader();
    let mut out = Vec::with_capacity(total.min(MAX_COMPRESSED_BYTES) as usize);
    let mut buf = [0u8; 16 * 1024];
    progress(0, total);
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| CodecError::Network(e.to_string()))?;
        if n == 0 {
            break;
        }
        if out.len() as u64 + n as u64 > MAX_COMPRESSED_BYTES {
            return Err(CodecError::TooLarge);
        }
        out.extend_from_slice(&buf[..n]);
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
