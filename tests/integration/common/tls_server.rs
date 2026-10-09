// 0.1.3: new file. A test-only HTTPS server with a certificate made on the spot. Not a product feature.
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio_rustls::rustls::{ServerConfig, crypto::aws_lc_rs};

/// A certificate and its private key, as PEM text.
#[derive(Clone)]
pub struct TestCert {
    pub cert_pem: String,
    pub key_pem: String,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
}

/// A self-signed certificate valid for the given names (host names or IP addresses).
pub fn self_signed(names: &[&str]) -> TestCert {
    let names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
    let made = rcgen::generate_simple_self_signed(names).expect("make certificate");
    TestCert {
        cert_pem: made.cert.pem(),
        key_pem: made.signing_key.serialize_pem(),
        cert_der: made.cert.der().to_vec(),
        key_der: made.signing_key.serialize_der(),
    }
}

/// A certificate for `names` signed by a small throwaway authority. Returns (authority PEM, certificate).
pub fn signed_by_new_authority(names: &[&str]) -> (String, TestCert) {
    signed_by_new_authority_with(names, false)
}

/// Like `signed_by_new_authority`; with `expired` the certificate ran out in the year 2000.
pub fn signed_by_new_authority_with(names: &[&str], expired: bool) -> (String, TestCert) {
    use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
    let ca_key = KeyPair::generate().expect("ca key");
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).expect("ca params");
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca = ca_params.self_signed(&ca_key).expect("ca cert");
    let ca_pem = ca.pem();
    let issuer = rcgen::Issuer::new(ca_params, ca_key);
    let key = KeyPair::generate().expect("key");
    let params = CertificateParams::new(names.iter().map(|n| n.to_string()).collect::<Vec<_>>())
        .expect("params");
    let mut params = params;
    if expired {
        params.not_before = rcgen::date_time_ymd(1999, 1, 1);
        params.not_after = rcgen::date_time_ymd(2000, 1, 1);
    }
    let cert = params.signed_by(&key, &issuer).expect("signed");
    (
        ca_pem,
        TestCert {
            cert_pem: cert.pem(),
            key_pem: key.serialize_pem(),
            cert_der: cert.der().to_vec(),
            key_der: key.serialize_der(),
        },
    )
}

struct TlsListener {
    tcp: TcpListener,
    acceptor: TlsAcceptor,
}

impl axum::serve::Listener for TlsListener {
    type Io = tokio_rustls::server::TlsStream<tokio::net::TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let Ok((stream, addr)) = self.tcp.accept().await else {
                continue;
            };
            // A client that does not trust us hangs up during the handshake; wait for the next one.
            if let Ok(tls) = self.acceptor.accept(stream).await {
                return (tls, addr);
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.tcp.local_addr()
    }
}

/// A TLS acceptor for this certificate (for fixtures that are not axum apps).
pub fn acceptor(cert: &TestCert) -> TlsAcceptor {
    let provider = Arc::new(aws_lc_rs::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("protocol versions")
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(cert.cert_der.clone())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.key_der.clone())),
        )
        .expect("server config");
    TlsAcceptor::from(Arc::new(config))
}

/// Serves `app` over TLS on a free local port. Returns the port.
pub async fn serve_tls(app: Router, cert: &TestCert) -> u16 {
    serve_tls_with(move |_| app, cert).await
}

/// Like `serve_tls`, for an app that must know its own port before it is built.
pub async fn serve_tls_with(make_app: impl FnOnce(u16) -> Router, cert: &TestCert) -> u16 {
    let provider = Arc::new(aws_lc_rs::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("protocol versions")
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(cert.cert_der.clone())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.key_der.clone())),
        )
        .expect("server config");
    let tcp = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = tcp.local_addr().expect("addr").port();
    let app = make_app(port);
    let listener = TlsListener {
        tcp,
        acceptor: TlsAcceptor::from(Arc::new(config)),
    };
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    port
}
