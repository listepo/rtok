//! Preconfigured TLS roots (T53.3).
//!
//! reqwest 0.13's `rustls` feature hard-depends on `rustls-platform-verifier`,
//! which links Security.framework and CoreFoundation into the one binary —
//! about 1.3–1.5 ms of dyld time on every hook spawn, while nothing on the
//! hook path ever verifies a certificate. Both HTTPS clients (proxy, otel)
//! therefore hand [`preconfigured`] — Mozilla roots via `webpki-roots` — to
//! `ClientBuilder::use_preconfigured_tls` instead of reqwest's default
//! verifier. Corporate CAs: `SSL_CERT_FILE` (curl's convention) points at a
//! PEM bundle whose certificates *extend* the Mozilla set; a set-but-broken
//! variable fails closed with the path in the error, like curl.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};

/// Extra PEM bundle honored like curl's `SSL_CERT_FILE`.
const SSL_CERT_FILE: &str = "SSL_CERT_FILE";

/// The `rustls::ClientConfig` both HTTPS clients hand reqwest: Mozilla roots,
/// plus the `SSL_CERT_FILE` bundle on top when the variable is set.
pub fn preconfigured() -> Result<rustls::ClientConfig> {
    let extra = std::env::var_os(SSL_CERT_FILE);
    let roots = store_with(extra.as_deref().map(Path::new))?;
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    Ok(rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(rustls::ALL_VERSIONS)
        .context("TLS versions")?
        .with_root_certificates(roots)
        .with_no_client_auth())
}

/// Mozilla roots, plus `extra` when given. Fails closed: a missing,
/// unparsable or certificate-free bundle is an error naming the path.
fn store_with(extra: Option<&Path>) -> Result<rustls::RootCertStore> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    if let Some(path) = extra {
        let file = std::fs::File::open(path)
            .with_context(|| format!("{SSL_CERT_FILE}: cannot open {}", path.display()))?;
        let certs: Vec<_> = rustls_pemfile::certs(&mut std::io::BufReader::new(file))
            .collect::<std::result::Result<_, _>>()
            .with_context(|| format!("{SSL_CERT_FILE}: cannot parse {}", path.display()))?;
        if certs.is_empty() {
            anyhow::bail!("{SSL_CERT_FILE}: no certificates in {}", path.display());
        }
        for cert in certs {
            roots.add(cert).with_context(|| {
                format!("{SSL_CERT_FILE}: invalid certificate in {}", path.display())
            })?;
        }
    }
    Ok(roots)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mozilla ships well over a hundred roots; the floor only guards against
    /// resolving to an empty store, not the exact upstream count.
    const MOZILLA_FLOOR: usize = 100;

    #[test]
    fn mozilla_roots_load_without_extras() {
        let roots = store_with(None).expect("Mozilla roots");
        assert!(roots.len() > MOZILLA_FLOOR, "only {} roots", roots.len());
    }

    #[test]
    fn bundle_extends_the_mozilla_set() {
        let base = store_with(None).expect("Mozilla roots").len();
        let bundled = store_with(Some(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/tls/test-ca.pem"
        ))))
        .expect("test CA bundle");
        assert_eq!(bundled.len(), base + 1);
    }

    #[test]
    fn broken_bundles_fail_closed_naming_the_path() {
        for name in ["missing.pem", "empty.pem", "garbage.pem"] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/tls")
                .join(name);
            let err = format!("{:?}", store_with(Some(&path)).unwrap_err());
            assert!(
                err.contains(name),
                "{name} error names the path, got: {err}"
            );
        }
    }
}
