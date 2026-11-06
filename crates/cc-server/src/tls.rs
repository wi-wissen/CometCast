// SPDX-License-Identifier: GPL-3.0-or-later
//! HTTPS / WSS with rustls: certificate and key from files, or (development only)
//! a self-signed certificate made at start.
//!
//! UNCERTAIN: certificates are read once at start. A renewed certificate (for example
//! after an ACME renewal, which is not built yet) needs a restart of the server.

use std::io;
use std::sync::Arc;

use axum_server::tls_rustls::RustlsConfig;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

use crate::config::TlsConfig;

fn other<E: std::fmt::Display>(what: &str, e: E) -> io::Error {
    io::Error::other(format!("{what}: {e}"))
}

/// Open a certificate or key file now, so a problem is reported clearly at start
/// (not as a vague TLS error). Explains the usual permission traps.
fn check_readable(path: &std::path::Path, setting: &str) -> io::Result<()> {
    match std::fs::File::open(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => Err(io::Error::other(format!(
            "{setting}: cannot read {}: permission denied. The server may run as another user \
             (in the Docker image: UID 65532), and tools such as certbot create private keys \
             readable by root only (mode 0600). Give the server user read access to the key \
             and to the folders above it, for example with a group or a copy owned by that user.",
            path.display()
        ))),
        Err(e) => Err(io::Error::other(format!(
            "{setting}: cannot read {}: {e}",
            path.display()
        ))),
    }
}

/// Build the TLS settings for the listener. Call only when `cfg.enabled()`.
pub(crate) fn build(cfg: &TlsConfig) -> io::Result<RustlsConfig> {
    let (certs, key) = if cfg.self_signed {
        self_signed(cfg)?
    } else {
        let (Some(cert_file), Some(key_file)) = (&cfg.cert_file, &cfg.key_file) else {
            return Err(io::Error::other(
                "tls.cert_file and tls.key_file must be set together",
            ));
        };
        check_readable(cert_file, "tls.cert_file")?;
        check_readable(key_file, "tls.key_file")?;
        let certs = CertificateDer::pem_file_iter(cert_file)
            .map_err(|e| other(&format!("cannot read {}", cert_file.display()), e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| other(&format!("bad certificate in {}", cert_file.display()), e))?;
        if certs.is_empty() {
            return Err(io::Error::other(format!(
                "no certificate found in {}",
                cert_file.display()
            )));
        }
        let key = PrivateKeyDer::from_pem_file(key_file)
            .map_err(|e| other(&format!("cannot read key {}", key_file.display()), e))?;
        (certs, key)
    };

    // The `ring` crypto backend is named explicitly, so no process-wide default is needed.
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| other("TLS versions", e))?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| other("certificate and key do not fit together", e))?;
    // WebSocket upgrades are an HTTP/1.1 feature, so only HTTP/1.1 is offered.
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(RustlsConfig::from_config(Arc::new(config)))
}

/// Throw-away certificate for development. Loud warning on purpose.
fn self_signed(
    cfg: &TlsConfig,
) -> io::Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    eprintln!("WARNING: using a SELF-SIGNED certificate (tls.self_signed = true).");
    eprintln!("WARNING: this is for development only. Browsers will not trust it, and it");
    eprintln!("WARNING: changes at every start. Use a real certificate in production.");
    let ck = rcgen::generate_simple_self_signed(cfg.self_signed_names.clone())
        .map_err(|e| other("cannot generate the self-signed certificate", e))?;
    if let Some(path) = &cfg.self_signed_cert_out {
        // Only the public certificate is written, never the key.
        std::fs::write(path, ck.cert.pem())
            .map_err(|e| other(&format!("cannot write {}", path.display()), e))?;
        eprintln!(
            "WARNING: wrote the generated certificate to {}",
            path.display()
        );
    }
    let key = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(ck.signing_key.serialize_der()));
    Ok((vec![ck.cert.der().clone()], key))
}
