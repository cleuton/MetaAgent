// 0.1.3: new file. HTTPS for `metagente serve`: reading the certificate and key, refusing a bad pair in plain words,
// and the loop that serves the A2A and MCP routers over TLS with every connection handled at the same time.

use super::config::ServeTlsConfig;
use super::net::date_of;
use crate::diagnostics::Diagnostic;
use axum::Router;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::service::TowerToHyperService;
use rustls::ServerConfig;
use rustls::crypto::aws_lc_rs;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

/// How long before it expires a certificate starts to be mentioned when `serve` starts.
const WARN_DAYS: i64 = 30;

/// The certificate and key `serve` was asked to present, and where each setting came from.
#[derive(Debug, Clone)]
pub struct Pair {
    pub cert: PathBuf,
    pub key: PathBuf,
    /// The place in metagente.toml (file, line) when the value came from there.
    pub cert_at: Option<(String, usize)>,
    pub key_at: Option<(String, usize)>,
}

fn located(d: Diagnostic, at: &Option<(String, usize)>) -> Diagnostic {
    match at {
        Some((file, line)) if *line > 0 => d.at(file, *line, 1),
        _ => d,
    }
}

/// Decides the certificate and key from the flags and `[serve]`: a flag wins over metagente.toml (D4).
/// Returns the pair (if any) and the lines to print about where each setting came from.
pub fn resolve(
    flag_cert: Option<&Path>,
    flag_key: Option<&Path>,
    toml: &ServeTlsConfig,
) -> Result<(Option<Pair>, Vec<String>), Diagnostic> {
    let mut notes = Vec::new();
    let pick = |flag: Option<&Path>,
                from_toml: &Option<PathBuf>,
                name: &str,
                flag_name: &str,
                notes: &mut Vec<String>| {
        match (flag, from_toml) {
            (Some(f), Some(t)) if f != t.as_path() => {
                notes.push(format!(
                    "Using {} {} from the command line instead of {} in metagente.toml.",
                    flag_name,
                    f.display(),
                    name
                ));
                Some((f.to_path_buf(), false))
            }
            (Some(f), _) => Some((f.to_path_buf(), false)),
            (None, Some(t)) => Some((t.clone(), true)),
            (None, None) => None,
        }
    };
    let cert = pick(flag_cert, &toml.cert, "tls_cert", "--tls-cert", &mut notes);
    let key = pick(flag_key, &toml.key, "tls_key", "--tls-key", &mut notes);
    match (cert, key) {
        (None, None) => Ok((None, notes)),
        (Some((cert, from_toml)), Some((key, key_toml))) => Ok((
            Some(Pair {
                cert,
                key,
                cert_at: from_toml.then(|| (toml.file.clone(), toml.cert_line)),
                key_at: key_toml.then(|| (toml.file.clone(), toml.key_line)),
            }),
            notes,
        )),
        (Some((_, from_toml)), None) => Err(located(
            Diagnostic::new("I have a certificate but no key (tls_key).")
                .fix("add tls_key to [serve] in metagente.toml, or use --tls-key with the PEM file of the private key"),
            &from_toml.then(|| (toml.file.clone(), toml.cert_line)),
        )),
        (None, Some((_, from_toml))) => Err(located(
            Diagnostic::new("I have a key but no certificate (tls_cert).")
                .fix("add tls_cert to [serve] in metagente.toml, or use --tls-cert with the PEM file of the certificate"),
            &from_toml.then(|| (toml.file.clone(), toml.key_line)),
        )),
    }
}

/// A certificate and key that are ready to serve with, plus anything worth telling the person.
pub struct Loaded {
    pub config: Arc<ServerConfig>,
    pub warnings: Vec<String>,
}

fn read(path: &Path, what: &str, at: &Option<(String, usize)>) -> Result<Vec<u8>, Diagnostic> {
    std::fs::read(path).map_err(|e| {
        let why = if e.kind() == std::io::ErrorKind::NotFound {
            "the file does not exist".to_string()
        } else {
            "I am not allowed to read it".to_string()
        };
        located(
            Diagnostic::new(format!(
                "I could not read the {} file {}: {}",
                what,
                path.display(),
                why
            ))
            .fix(format!(
                "check the path of the {} and that the file can be read",
                what
            )),
            at,
        )
    })
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Loads the pair: the files must be PEM, the key must match the certificate, and the certificate must be
/// in date. A certificate that expires within 30 days only produces a warning.
pub fn load(pair: &Pair) -> Result<Loaded, Diagnostic> {
    let cert_bytes = read(&pair.cert, "certificate", &pair.cert_at)?;
    let key_bytes = read(&pair.key, "key", &pair.key_at)?;
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(&cert_bytes)
        .filter_map(|c| c.ok())
        .collect();
    if certs.is_empty() {
        return Err(located(
            Diagnostic::new(format!(
                "{} is not a PEM certificate file (I found no certificate in it)",
                pair.cert.display()
            ))
            .fix("give the PEM file that starts with -----BEGIN CERTIFICATE-----"),
            &pair.cert_at,
        ));
    }
    let key = PrivateKeyDer::from_pem_slice(&key_bytes).map_err(|_| {
        located(
            Diagnostic::new(format!(
                "{} is not a PEM private key file (I found no key in it)",
                pair.key.display()
            ))
            .fix("give the PEM file that starts with -----BEGIN PRIVATE KEY-----"),
            &pair.key_at,
        )
    })?;
    let mut warnings = Vec::new();
    if let Ok((_, parsed)) = x509_parser::parse_x509_certificate(certs[0].as_ref()) {
        let now = now_seconds();
        let not_after = parsed.validity().not_after.timestamp();
        let not_before = parsed.validity().not_before.timestamp();
        if not_after < now {
            return Err(located(
                Diagnostic::new(format!(
                    "the certificate in {} expired on {}",
                    pair.cert.display(),
                    date_of(not_after.max(0) as u64)
                ))
                .fix("make a new certificate and key, and point tls_cert and tls_key at them"),
                &pair.cert_at,
            ));
        }
        if not_before > now {
            return Err(located(
                Diagnostic::new(format!(
                    "the certificate in {} is not valid before {}",
                    pair.cert.display(),
                    date_of(not_before.max(0) as u64)
                ))
                .fix("check the clock of this computer, or make a new certificate"),
                &pair.cert_at,
            ));
        }
        if not_after - now < WARN_DAYS * 24 * 3600 {
            warnings.push(format!(
                "The certificate in {} expires on {}.",
                pair.cert.display(),
                date_of(not_after.max(0) as u64)
            ));
        }
    }
    let provider = Arc::new(aws_lc_rs::default_provider());
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| {
            Diagnostic::new("I could not set up TLS on this computer")
                .fix("check the installation of Metagente")
        })?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|_| {
            located(
                Diagnostic::new(format!(
                    "the key in {} does not match the certificate in {}",
                    pair.key.display(),
                    pair.cert.display()
                ))
                .fix("use the key that was made together with this certificate"),
                &pair.key_at,
            )
        })?;
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(Loaded {
        config: Arc::new(config),
        warnings,
    })
}

/// Serves `app` over TLS on `listener` until the task is stopped. Every connection is its own task, so requests are
/// handled at the same time, and a client that never finishes the handshake is dropped after `handshake`.
pub async fn serve(
    listener: TcpListener,
    app: Router,
    config: Arc<ServerConfig>,
    handshake: Duration,
) {
    let acceptor = TlsAcceptor::from(config);
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(pair) => pair,
            Err(_) => {
                // too many open files and the like: wait a little and keep serving
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        };
        let acceptor = acceptor.clone();
        let app = app.clone();
        tokio::spawn(async move {
            let Ok(Ok(tls)) = tokio::time::timeout(handshake, acceptor.accept(stream)).await else {
                return;
            };
            let _ = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(TokioIo::new(tls), TowerToHyperService::new(app))
                .await;
        });
    }
}
