//! The web client shared by the http tool, remote agents and language model providers.

/// Builds the client without ever failing or panicking.
///
/// It first trusts the certificates of the computer. On a bare machine that has none (a fresh
/// container, for example) it falls back to the certificates bundled in the binary, so that agents
/// still start and `https` still works with nothing installed.
pub fn build_client() -> reqwest::Client {
    let builder =
        || reqwest::Client::builder().user_agent(concat!("metagente/", env!("CARGO_PKG_VERSION")));
    if let Ok(client) = builder().build() {
        return client;
    }
    let bundled = webpki_root_certs::TLS_SERVER_ROOT_CERTS
        .iter()
        .filter_map(|der| reqwest::Certificate::from_der(der.as_ref()).ok());
    if let Ok(client) = builder().tls_certs_only(bundled).build() {
        return client;
    }
    // Without any usable certificates only plain http can work; https calls will say why they fail.
    reqwest::Client::builder()
        .user_agent(concat!("metagente/", env!("CARGO_PKG_VERSION")))
        .tls_certs_only(std::iter::empty())
        .build()
        .unwrap_or_else(|_| reqwest::Client::builder().build().unwrap_or_default())
}
