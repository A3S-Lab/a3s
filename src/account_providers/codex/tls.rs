//! Shared trust-store policy for Codex WebSocket, HTTPS, and OAuth traffic.

use anyhow::{anyhow, Context, Result};
use rustls::pki_types::CertificateDer;
use rustls::{ClientConfig, RootCertStore};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct TlsRoots {
    platform: Arc<Vec<CertificateDer<'static>>>,
    custom: Arc<Vec<CertificateDer<'static>>>,
}

impl TlsRoots {
    pub(super) fn load() -> Result<Self> {
        let rustls_native_certs::CertificateResult {
            certs: platform,
            errors,
            ..
        } = rustls_native_certs::load_native_certs();
        if !errors.is_empty() {
            tracing::warn!(
                error_count = errors.len(),
                "Some platform root certificates could not be loaded for Codex"
            );
        }

        let custom = configured_ca_path()
            .map(|path| {
                let pem = std::fs::read(&path)
                    .with_context(|| format!("read Codex CA bundle {}", path.display()))?;
                parse_pem_certificates(&pem)
                    .with_context(|| format!("parse Codex CA bundle {}", path.display()))
            })
            .transpose()?
            .unwrap_or_default();

        Ok(Self {
            platform: Arc::new(platform),
            custom: Arc::new(custom),
        })
    }

    pub(super) fn rustls_client_config(&self) -> Result<ClientConfig> {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let (accepted, ignored) = roots.add_parsable_certificates(self.platform.iter().cloned());
        if ignored > 0 {
            tracing::warn!(
                accepted,
                ignored,
                "Some platform root certificates were rejected for Codex WebSocket TLS"
            );
        }
        for (index, certificate) in self.custom.iter().cloned().enumerate() {
            roots.add(certificate).with_context(|| {
                format!(
                    "register Codex custom CA certificate #{} for WebSocket TLS",
                    index + 1
                )
            })?;
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        Ok(ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .context("configure Codex WebSocket TLS protocol versions")?
            .with_root_certificates(roots)
            .with_no_client_auth())
    }

    pub(super) fn add_to_reqwest(
        &self,
        mut builder: reqwest::ClientBuilder,
    ) -> Result<reqwest::ClientBuilder> {
        for certificate in self.platform.iter() {
            match reqwest::Certificate::from_der(certificate.as_ref()) {
                Ok(certificate) => builder = builder.add_root_certificate(certificate),
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        "A platform root certificate was rejected for Codex HTTPS"
                    );
                }
            }
        }
        for (index, certificate) in self.custom.iter().enumerate() {
            let certificate =
                reqwest::Certificate::from_der(certificate.as_ref()).with_context(|| {
                    format!(
                        "register Codex custom CA certificate #{} for HTTPS",
                        index + 1
                    )
                })?;
            builder = builder.add_root_certificate(certificate);
        }
        Ok(builder)
    }
}

fn configured_ca_path() -> Option<PathBuf> {
    ["CODEX_CA_CERTIFICATE", "SSL_CERT_FILE"]
        .into_iter()
        .find_map(|key| {
            std::env::var_os(key)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
}

fn parse_pem_certificates(pem: &[u8]) -> Result<Vec<CertificateDer<'static>>> {
    let normalized = String::from_utf8_lossy(pem)
        .replace("BEGIN TRUSTED CERTIFICATE", "BEGIN CERTIFICATE")
        .replace("END TRUSTED CERTIFICATE", "END CERTIFICATE");
    let certificates = rustls_pemfile::certs(&mut Cursor::new(normalized.as_bytes()))
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("decode PEM certificate blocks")?;
    if certificates.is_empty() {
        return Err(anyhow!(
            "the configured file contains no CERTIFICATE blocks"
        ));
    }
    Ok(certificates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};

    #[test]
    fn parses_certificate_bundles_and_trusted_certificate_labels() {
        let der = b"test certificate bytes";
        let encoded = STANDARD.encode(der);
        let pem = format!(
            "-----BEGIN TRUSTED CERTIFICATE-----\n{encoded}\n-----END TRUSTED CERTIFICATE-----\n"
        );

        let certificates = parse_pem_certificates(pem.as_bytes()).unwrap();

        assert_eq!(certificates.len(), 1);
        assert_eq!(certificates[0].as_ref(), der);
    }

    #[test]
    fn rejects_a_configured_ca_file_without_certificates() {
        let error = parse_pem_certificates(b"not a certificate").unwrap_err();
        assert!(error.to_string().contains("no CERTIFICATE blocks"));
    }

    #[test]
    fn builds_websocket_config_with_an_explicit_crypto_provider() {
        let roots = TlsRoots {
            platform: Arc::new(Vec::new()),
            custom: Arc::new(Vec::new()),
        };

        roots
            .rustls_client_config()
            .expect("Codex WebSocket TLS config");
    }
}
