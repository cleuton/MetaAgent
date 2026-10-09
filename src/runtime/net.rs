//! 0.1.3: everything about outbound network connections: certificates, proxies, and plain-language failures.
//!
//! One `ClientSet` serves agents, tools and the language model. Settings come only from metagente.toml and the
//! standard proxy environment variables, never from `.ag` code.

use super::config::{NetworkConfig, ProxyConfig};
use crate::diagnostics::Diagnostic;
use crate::tools::ToolError;
use base64::Engine as _;
use reqwest::Url;
use rustls::CertificateError;
use std::error::Error as StdError;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

type EnvFn = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

// ---------- secrets and addresses ----------

/// A user name or password. It cannot be printed by accident: there is no `Debug` or `Display`.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Secret {
        Secret(value.into())
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}

/// An address that is safe to show: anything before an `@` in the address part is replaced by `***`.
pub fn redact(address: &str) -> String {
    let Some((scheme, rest)) = address.split_once("://") else {
        return address.to_string();
    };
    let (authority, tail) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    match authority.rsplit_once('@') {
        Some((_, host)) => format!("{}://***@{}{}", scheme, host, tail),
        None => address.to_string(),
    }
}

/// A failure while setting up a connection, in plain words.
#[derive(Debug, Clone)]
pub struct NetError {
    pub message: String,
    pub fix: Option<String>,
}

impl NetError {
    fn new(message: impl Into<String>, fix: impl Into<String>) -> NetError {
        NetError {
            message: message.into(),
            fix: Some(fix.into()),
        }
    }
    /// One piece of text, for places that carry only a string (the language model).
    pub fn to_text(&self) -> String {
        match &self.fix {
            Some(f) => format!("{} ({})", self.message, f),
            None => self.message.clone(),
        }
    }
}

impl From<NetError> for ToolError {
    fn from(e: NetError) -> ToolError {
        let t = ToolError::new(e.message);
        match e.fix {
            Some(f) => t.fix(f),
            None => t,
        }
    }
}

impl From<Diagnostic> for NetError {
    fn from(d: Diagnostic) -> NetError {
        let mut message = d.message;
        if d.line > 0 {
            message = format!("line {} of {}: {}", d.line, d.file, message);
        }
        NetError {
            message,
            fix: d.suggestion,
        }
    }
}

// ---------- the proxy ----------

#[derive(Clone)]
pub struct ProxyTarget {
    /// The proxy address without any login.
    pub url: Url,
    pub login: Option<(Secret, Secret)>,
}

impl ProxyTarget {
    /// The address to show people.
    pub fn display(&self) -> String {
        redact(self.url.as_str().trim_end_matches('/'))
    }
    /// The address with the login inside, only ever handed to the HTTP library.
    fn with_login(&self) -> Url {
        let mut url = self.url.clone();
        if let Some((user, password)) = &self.login {
            let _ = url.set_username(user.expose());
            let _ = url.set_password(Some(password.expose()));
        }
        url
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxySource {
    None,
    Toml,
    Environment,
}

#[derive(Clone)]
pub struct EffectiveProxy {
    pub source: ProxySource,
    /// The environment variable the proxy came from, when it came from the environment.
    pub origin: String,
    pub https: Option<ProxyTarget>,
    pub http: Option<ProxyTarget>,
    pub no_proxy: Vec<String>,
    /// True when proxy variables exist in the environment but metagente.toml won.
    pub environment_ignored: bool,
}

const ENV_HTTPS: [&str; 2] = ["HTTPS_PROXY", "https_proxy"];
const ENV_HTTP: [&str; 2] = ["HTTP_PROXY", "http_proxy"];
const ENV_NO: [&str; 2] = ["NO_PROXY", "no_proxy"];

fn first_env(env: &dyn Fn(&str) -> Option<String>, names: &[&str]) -> Option<(String, String)> {
    names.iter().find_map(|n| {
        env(n)
            .filter(|v| !v.trim().is_empty())
            .map(|v| (n.to_string(), v))
    })
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn target_from(text: &str) -> Option<ProxyTarget> {
    let text = text.trim();
    let full = if text.contains("://") {
        text.to_string()
    } else {
        format!("http://{}", text)
    };
    let mut url = Url::parse(&full).ok()?;
    if !(url.scheme() == "http" || url.scheme() == "https") || url.host_str().is_none() {
        return None;
    }
    let login = if url.username().is_empty() {
        None
    } else {
        Some((
            Secret::new(decode(url.username())),
            Secret::new(decode(url.password().unwrap_or(""))),
        ))
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    Some(ProxyTarget { url, login })
}

fn read_var(
    env: &dyn Fn(&str) -> Option<String>,
    name: &str,
    key: &str,
    line: usize,
    file: &str,
) -> Result<Secret, Diagnostic> {
    match env(name).filter(|v| !v.is_empty()) {
        Some(v) => Ok(Secret::new(v)),
        None => {
            let mut d = Diagnostic::new(format!(
                "the variable `{}` named by {} is not set or is empty",
                name, key
            ))
            .fix(format!(
                "set {} in your environment before you start Metagente",
                name
            ));
            if line > 0 {
                d = d.at(file, line, 1);
            }
            Err(d)
        }
    }
}

impl EffectiveProxy {
    pub fn none() -> EffectiveProxy {
        EffectiveProxy {
            source: ProxySource::None,
            origin: String::new(),
            https: None,
            http: None,
            no_proxy: Vec::new(),
            environment_ignored: false,
        }
    }

    /// Decides which proxy is in use. metagente.toml wins entirely when it has a `url`.
    pub fn resolve(
        cfg: &ProxyConfig,
        file: &str,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Result<EffectiveProxy, Diagnostic> {
        let login = match (&cfg.username_env, &cfg.password_env) {
            (Some(u), Some(p)) => Some((
                read_var(env, u, "username_env", cfg.username_env_line, file)?,
                read_var(env, p, "password_env", cfg.password_env_line, file)?,
            )),
            _ => None,
        };
        let env_has_proxy =
            first_env(env, &ENV_HTTPS).is_some() || first_env(env, &ENV_HTTP).is_some();
        let mut result = EffectiveProxy::none();
        if let Some(url) = &cfg.url {
            let mut target = target_from(url).ok_or_else(|| {
                let mut d = Diagnostic::new(format!(
                    "`{}` is not a proxy address I can use",
                    redact(url)
                ))
                .fix("write it like url = \"http://proxy.company.com:3128\"");
                if cfg.url_line > 0 {
                    d = d.at(file, cfg.url_line, 1);
                }
                d
            })?;
            if login.is_some() {
                target.login = login.clone();
            }
            result.source = ProxySource::Toml;
            result.https = Some(target.clone());
            result.http = Some(target);
            result.no_proxy = cfg.no_proxy.clone();
            result.environment_ignored = env_has_proxy || first_env(env, &ENV_NO).is_some();
            return Ok(result);
        }
        let https = first_env(env, &ENV_HTTPS);
        let http = first_env(env, &ENV_HTTP);
        if https.is_some() || http.is_some() {
            result.source = ProxySource::Environment;
            result.origin = https
                .as_ref()
                .or(http.as_ref())
                .map(|(n, _)| n.clone())
                .unwrap_or_default();
            let bad = |name: &str, value: &str| {
                Diagnostic::new(format!(
                    "{} is `{}`, which is not a proxy address I can use",
                    name,
                    redact(value)
                ))
                .fix("write it like http://proxy.company.com:3128")
            };
            if let Some((name, value)) = &https {
                result.https = Some(target_from(value).ok_or_else(|| bad(name, value))?);
            }
            if let Some((name, value)) = &http {
                result.http = Some(target_from(value).ok_or_else(|| bad(name, value))?);
            }
            if login.is_some() {
                for t in [result.https.as_mut(), result.http.as_mut()]
                    .into_iter()
                    .flatten()
                {
                    t.login = login.clone();
                }
            }
        }
        result.no_proxy = if !cfg.no_proxy.is_empty() {
            cfg.no_proxy.clone()
        } else {
            first_env(env, &ENV_NO)
                .map(|(_, v)| {
                    v.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default()
        };
        Ok(result)
    }

    /// The proxy to use for this destination, or `None` to go direct.
    pub fn for_destination(&self, destination: &Url) -> Option<&ProxyTarget> {
        let host = destination.host_str()?;
        if no_proxy_matches(host, &self.no_proxy) {
            return None;
        }
        match destination.scheme() {
            "https" => self.https.as_ref(),
            "http" => self.http.as_ref(),
            _ => None,
        }
    }

    pub fn any(&self) -> bool {
        self.https.is_some() || self.http.is_some()
    }

    /// One plain sentence for `metagente check`.
    pub fn describe(&self) -> String {
        match self.source {
            ProxySource::None => "proxy: none (connections go direct)".to_string(),
            ProxySource::Toml => {
                let shown = self.https.as_ref().map(|t| t.display()).unwrap_or_default();
                let mut text = format!("proxy: from metagente.toml ({})", shown);
                if self.environment_ignored {
                    text.push_str(
                        "; the proxy variables in the environment are ignored because metagente.toml sets a proxy url",
                    );
                }
                text
            }
            ProxySource::Environment => {
                // the address never carries the login; say that one is used instead of showing it
                let shown = self
                    .https
                    .as_ref()
                    .or(self.http.as_ref())
                    .map(|t| {
                        if t.login.is_some() {
                            format!("{}, with a login", t.display())
                        } else {
                            t.display()
                        }
                    })
                    .unwrap_or_default();
                format!(
                    "proxy: from the environment variable {} ({})",
                    self.origin, shown
                )
            }
        }
    }

    /// The variables for a child process, when `pass_to_tools` is on. Includes the login when there is one.
    pub fn child_environment(&self) -> Vec<(String, String)> {
        if !self.any() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (names, target) in [(ENV_HTTPS, &self.https), (ENV_HTTP, &self.http)] {
            if let Some(t) = target {
                let value = t.with_login().to_string();
                let value = value.trim_end_matches('/').to_string();
                for n in names {
                    out.push((n.to_string(), value.clone()));
                }
            }
        }
        if !self.no_proxy.is_empty() {
            let list = self.no_proxy.join(",");
            for n in ENV_NO {
                out.push((n.to_string(), list.clone()));
            }
        }
        out
    }
}

/// Host names, domain suffixes (`.internal.company.com`), IP addresses and `localhost`.
pub fn no_proxy_matches(host: &str, patterns: &[String]) -> bool {
    let host = host
        .trim_matches(|c| c == '[' || c == ']')
        .to_ascii_lowercase();
    patterns.iter().any(|p| {
        let p = p.trim().to_ascii_lowercase();
        if p == "*" {
            return true;
        }
        let p = p
            .trim_start_matches("*.")
            .trim_start_matches('.')
            .to_string();
        // a pattern may carry a port, which does not matter here
        let p = match p.rsplit_once(':') {
            Some((h, port)) if port.chars().all(|c| c.is_ascii_digit()) && !h.contains(':') => {
                h.to_string()
            }
            _ => p,
        };
        if p.contains('/') {
            return cidr_contains(&p, &host);
        }
        !p.is_empty() && (host == p || host.ends_with(&format!(".{}", p)))
    })
}

/// `10.0.0.0/8` or `fd00::/8` against an IP address host. Anything else does not match.
fn cidr_contains(pattern: &str, host: &str) -> bool {
    use std::net::IpAddr;
    let Some((net, bits)) = pattern.split_once('/') else {
        return false;
    };
    let (Ok(net), Ok(bits), Ok(host)) = (
        net.parse::<IpAddr>(),
        bits.parse::<u32>(),
        host.parse::<IpAddr>(),
    ) else {
        return false;
    };
    match (net, host) {
        (IpAddr::V4(n), IpAddr::V4(h)) if bits <= 32 => {
            let mask = if bits == 0 {
                0
            } else {
                u32::MAX << (32 - bits)
            };
            u32::from(n) & mask == u32::from(h) & mask
        }
        (IpAddr::V6(n), IpAddr::V6(h)) if bits <= 128 => {
            let mask = if bits == 0 {
                0
            } else {
                u128::MAX << (128 - bits)
            };
            u128::from(n) & mask == u128::from(h) & mask
        }
        _ => false,
    }
}

// ---------- warnings ----------

/// The warnings that every `run`, `serve` and `check` prints. They cannot be switched off.
pub fn startup_warnings(cfg: &NetworkConfig) -> Vec<String> {
    let mut out = Vec::new();
    if cfg.allow_self_signed {
        let scope = if cfg.self_signed_hosts.is_empty() {
            String::new()
        } else {
            format!(" for {}", cfg.self_signed_hosts.join(", "))
        };
        out.push(format!(
            "certificate checks are OFF for agents and tools{} (allow_self_signed = true in metagente.toml). Use this for development only.",
            scope
        ));
    }
    if cfg.llm_allow_self_signed {
        out.push("certificate checks are OFF for the language model connection (llm_allow_self_signed = true in metagente.toml). Your model key will be sent to a server whose identity was not verified.".to_string());
    }
    out
}

/// Only `metagente check` prints this one.
pub fn pass_to_tools_warning(cfg: &NetworkConfig) -> Option<String> {
    cfg.proxy.pass_to_tools.then(|| {
        "the proxy login is passed to the tool programs your agents start (pass_to_tools = true). Declare only tools you trust.".to_string()
    })
}

// ---------- redirects ----------

/// A redirect that was refused, with the reason in plain words.
#[derive(Debug)]
pub struct RedirectRefused {
    pub from: String,
    pub to: String,
    pub reason: &'static str,
}

impl std::fmt::Display for RedirectRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "redirect refused: {}", self.reason)
    }
}
impl StdError for RedirectRefused {}

type HostRule = Arc<dyn Fn(&str) -> bool + Send + Sync>;

fn redirect_policy(lax_rule: Option<(HostRule, bool)>) -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= 10 {
            return attempt.error("too many redirects");
        }
        let next = attempt.url().clone();
        let previous = attempt.previous().last().cloned();
        if let Some(prev) = previous {
            if prev.scheme() == "https" && next.scheme() == "http" {
                return attempt.error(RedirectRefused {
                    from: redact(prev.as_str()),
                    to: redact(next.as_str()),
                    reason: "an https address sent me to an http address",
                });
            }
            if let Some((rule, in_lax_client)) = &lax_rule {
                let applies = rule(next.host_str().unwrap_or(""));
                if applies != *in_lax_client {
                    return attempt.error(RedirectRefused {
                        from: redact(prev.as_str()),
                        to: redact(next.as_str()),
                        reason: "the other host needs different certificate rules",
                    });
                }
            }
        }
        attempt.follow()
    })
}

// ---------- the clients ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Agents, MCP servers and the `http` tool.
    Agents,
    /// The language model provider.
    Model,
}

struct Inner {
    cfg: NetworkConfig,
    env: EnvFn,
    /// How long opening a connection (including the proxy tunnel and the TLS handshake) may take.
    connect_timeout: Duration,
    proxy: OnceLock<Result<Arc<EffectiveProxy>, NetError>>,
    /// Agents clients by (lax, for MCP): index = lax + 2 * mcp.
    agents: [OnceLock<Result<reqwest::Client, NetError>>; 4],
    model: OnceLock<Result<reqwest::Client, NetError>>,
}

/// The HTTP clients of one runtime. Built only when first needed, so an agent that never uses the network pays nothing.
pub struct ClientSet {
    inner: RwLock<Arc<Inner>>,
}

fn host_rule(cfg: &NetworkConfig) -> HostRule {
    let on = cfg.allow_self_signed;
    let hosts: Vec<String> = cfg
        .self_signed_hosts
        .iter()
        .map(|h| {
            let h = h.trim().to_ascii_lowercase();
            // a port makes no difference to which host this is: "localhost:8443" means "localhost"
            let h = match h.rsplit_once(':') {
                Some((name, port))
                    if !name.contains(':')
                        && !port.is_empty()
                        && port.chars().all(|c| c.is_ascii_digit()) =>
                {
                    name.to_string()
                }
                _ => h,
            };
            h.trim_matches(|c| c == '[' || c == ']').to_string()
        })
        .collect();
    Arc::new(move |host: &str| {
        let host = host
            .trim_matches(|c| c == '[' || c == ']')
            .to_ascii_lowercase();
        on && (hosts.is_empty() || hosts.contains(&host))
    })
}

impl Inner {
    fn new(cfg: NetworkConfig, env: EnvFn, connect_timeout: Duration) -> Arc<Inner> {
        Arc::new(Inner {
            cfg,
            env,
            connect_timeout,
            proxy: OnceLock::new(),
            agents: [
                OnceLock::new(),
                OnceLock::new(),
                OnceLock::new(),
                OnceLock::new(),
            ],
            model: OnceLock::new(),
        })
    }

    fn proxy(&self) -> Result<Arc<EffectiveProxy>, NetError> {
        self.proxy
            .get_or_init(|| {
                let env = self.env.clone();
                EffectiveProxy::resolve(&self.cfg.proxy, &self.cfg.file, &*env)
                    .map(Arc::new)
                    .map_err(NetError::from)
            })
            .clone()
    }

    fn extra_roots(&self) -> Result<Vec<reqwest::Certificate>, NetError> {
        let Some(path) = &self.cfg.ca_file else {
            return Ok(Vec::new());
        };
        let bad = |why: String| {
            NetError::new(
                format!(
                    "I could not use the certificate file {}: {}",
                    path.display(),
                    why
                ),
                "ca_file must be a PEM file with one or more certificates",
            )
        };
        let bytes = std::fs::read(path).map_err(|e| bad(e.to_string()))?;
        let certs = reqwest::Certificate::from_pem_bundle(&bytes)
            .map_err(|_| bad("it is not a PEM certificate file".to_string()))?;
        if certs.is_empty() {
            return Err(bad("it has no certificates in it".to_string()));
        }
        Ok(certs)
    }

    fn build(&self, purpose: Purpose, lax: bool, mcp: bool) -> Result<reqwest::Client, NetError> {
        let proxy = self.proxy()?;
        let roots = self.extra_roots()?;
        let rule = host_rule(&self.cfg);
        let configure = |b: reqwest::ClientBuilder| -> reqwest::ClientBuilder {
            let mut b = b
                .user_agent(concat!("metagente/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(self.connect_timeout)
                .redirect(match purpose {
                    Purpose::Agents => redirect_policy(Some((rule.clone(), lax))),
                    Purpose::Model => redirect_policy(None),
                });
            if mcp {
                // as the MCP library does for itself: no idle connections kept (avoids stalls), no redirects
                b = b
                    .pool_max_idle_per_host(0)
                    .redirect(reqwest::redirect::Policy::none());
            }
            for c in &roots {
                b = b.add_root_certificate(c.clone());
            }
            b = if proxy.any() {
                let chosen = proxy.clone();
                b.proxy(reqwest::Proxy::custom(move |url| {
                    chosen.for_destination(url).map(|t| t.with_login())
                }))
            } else {
                b.no_proxy()
            };
            if lax {
                b = b.danger_accept_invalid_certs(true);
            }
            b
        };
        // The certificates of the computer first. On a bare machine that has none (a fresh container), the
        // certificates bundled in the binary are used, so agents still start and `https` still works.
        if let Ok(client) = configure(reqwest::Client::builder()).build() {
            return Ok(client);
        }
        let bundled = webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .filter_map(|der| reqwest::Certificate::from_der(der.as_ref()).ok());
        if let Ok(client) = configure(reqwest::Client::builder())
            .tls_certs_only(bundled)
            .build()
        {
            return Ok(client);
        }
        // Without any usable certificates only plain http can work; https calls will say why they fail.
        configure(reqwest::Client::builder())
            .tls_certs_only(std::iter::empty())
            .build()
            .map_err(|_| {
                NetError::new(
                    "I could not set up the network client",
                    "check the [network] section of metagente.toml",
                )
            })
    }
}

fn real_env() -> EnvFn {
    Arc::new(|name: &str| std::env::var(name).ok())
}

impl ClientSet {
    pub fn new(cfg: NetworkConfig) -> ClientSet {
        Self::with_connect_timeout(cfg, Duration::from_secs(20))
    }

    /// The clients for a runtime: opening a connection may take two thirds of the tool timeout, so a proxy that
    /// never answers is reported as such before the whole call times out.
    pub fn for_config(config: &super::config::Config) -> ClientSet {
        let seconds = ((config.timeout_seconds * 2) / 3).max(1);
        Self::with_connect_timeout(config.network.clone(), Duration::from_secs(seconds))
    }

    pub fn with_connect_timeout(cfg: NetworkConfig, connect_timeout: Duration) -> ClientSet {
        ClientSet {
            inner: RwLock::new(Inner::new(cfg, real_env(), connect_timeout)),
        }
    }

    /// For tests that must not touch the real environment.
    pub fn with_env(
        cfg: NetworkConfig,
        connect_timeout: Duration,
        env: impl Fn(&str) -> Option<String> + Send + Sync + 'static,
    ) -> ClientSet {
        ClientSet {
            inner: RwLock::new(Inner::new(cfg, Arc::new(env), connect_timeout)),
        }
    }

    fn snapshot(&self) -> Arc<Inner> {
        match self.inner.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// Applies new settings to the next request. Requests already running keep the old ones.
    /// Returns true when something changed.
    pub fn reload(&self, cfg: NetworkConfig) -> bool {
        let old = self.snapshot();
        if old.cfg == cfg {
            return false;
        }
        let fresh = Inner::new(cfg, old.env.clone(), old.connect_timeout);
        match self.inner.write() {
            Ok(mut guard) => *guard = fresh,
            Err(poisoned) => *poisoned.into_inner() = fresh,
        }
        true
    }

    pub fn config(&self) -> NetworkConfig {
        self.snapshot().cfg.clone()
    }

    /// The client for an agent, MCP server or web address. Certificate checks are skipped only where
    /// `allow_self_signed` says so.
    pub fn agents_for(&self, url: &str) -> Result<reqwest::Client, NetError> {
        self.agents_client(url, false)
    }

    /// 0.1.3: the same rules, for an MCP server over HTTP.
    pub fn mcp_for(&self, url: &str) -> Result<reqwest::Client, NetError> {
        self.agents_client(url, true)
    }

    fn agents_client(&self, url: &str, mcp: bool) -> Result<reqwest::Client, NetError> {
        let inner = self.snapshot();
        let host = Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(String::from))
            .unwrap_or_default();
        let lax = host_rule(&inner.cfg)(&host);
        let slot = &inner.agents[lax as usize + 2 * mcp as usize];
        slot.get_or_init(|| inner.build(Purpose::Agents, lax, mcp))
            .clone()
    }

    /// The client for the language model provider.
    pub fn model(&self) -> Result<reqwest::Client, NetError> {
        let inner = self.snapshot();
        let lax = inner.cfg.llm_allow_self_signed;
        inner
            .model
            .get_or_init(|| inner.build(Purpose::Model, lax, false))
            .clone()
    }

    /// How many clients have been built so far. An agent that never uses the network keeps this at 0.
    pub fn clients_built(&self) -> usize {
        let inner = self.snapshot();
        inner.agents.iter().filter(|c| c.get().is_some()).count()
            + usize::from(inner.model.get().is_some())
    }

    /// The proxy in use (from metagente.toml or the environment).
    pub fn proxy(&self) -> Result<Arc<EffectiveProxy>, NetError> {
        self.snapshot().proxy()
    }

    /// Variables to add to a tool program started over stdio. Empty unless `pass_to_tools` is on.
    pub fn child_environment(&self) -> Vec<(String, String)> {
        let inner = self.snapshot();
        if !inner.cfg.proxy.pass_to_tools {
            return Vec::new();
        }
        inner
            .proxy()
            .map(|p| p.child_environment())
            .unwrap_or_default()
    }

    /// For errors from libraries that wrap `reqwest` (the MCP library): finds the request error inside and
    /// classifies it. The answer is plain data, so the caller can drop the library error before waiting.
    pub fn classify_wrapped(
        &self,
        error: &(dyn StdError + 'static),
        url: &str,
        who: &str,
        fix: &str,
    ) -> Option<Outcome> {
        let mut next: Option<&(dyn StdError + 'static)> = Some(error);
        while let Some(e) = next {
            if let Some(r) = e.downcast_ref::<reqwest::Error>() {
                return Some(classify(
                    &self.snapshot(),
                    r,
                    url,
                    who,
                    fix,
                    Purpose::Agents,
                ));
            }
            next = e.source();
        }
        None
    }

    /// When a library hides why a connection failed, ask the address once more ourselves and explain that answer.
    /// Returns `None` when the address answers (so the problem is not the connection).
    pub async fn diagnose(&self, url: &str, who: &str, fix: &str) -> Option<ToolError> {
        let client = self.mcp_for(url).ok()?;
        match tokio::time::timeout(Duration::from_secs(5), client.get(url).send()).await {
            Ok(Err(e)) => Some(self.explain(&e, url, who, fix, Purpose::Agents).await),
            _ => None,
        }
    }

    /// Turns a failed request into a plain message that says what went wrong and how to fix it.
    pub async fn explain(
        &self,
        error: &reqwest::Error,
        url: &str,
        who: &str,
        fix: &str,
        purpose: Purpose,
    ) -> ToolError {
        let outcome = classify(&self.snapshot(), error, url, who, fix, purpose);
        self.finish(outcome).await
    }

    /// Completes a classification; a refused proxy is asked what it wanted, so the message can say.
    pub async fn finish(&self, outcome: Outcome) -> ToolError {
        match outcome {
            Outcome::Done(e) => e,
            Outcome::ProxyLogin { target, wanted } => {
                let name = target.display();
                if let Some(schemes) = offered_schemes(&target, &wanted).await {
                    if !schemes.is_empty()
                        && !schemes.iter().any(|s| s.eq_ignore_ascii_case("basic"))
                    {
                        return ToolError::new(format!(
                            "the proxy {} asks for {} login, and this version supports only Basic",
                            name,
                            schemes.join(" or ")
                        ))
                        .fix("use a proxy that accepts Basic login, or ask for it to be allowed");
                    }
                }
                ToolError::new(format!("the proxy {} refused the login", name)).fix(
                    "check username_env and password_env in [network.proxy], and that the variables they name hold the right values",
                )
            }
            Outcome::ProxyRefused { target, wanted } => {
                let status = probe_status(&target, &wanted).await;
                let said = status
                    .map(|s| format!(" (it answered {})", s))
                    .unwrap_or_default();
                ToolError::new(format!(
                    "the proxy {} refused to open a connection to {}{}",
                    target.display(),
                    wanted,
                    said
                ))
                .fix("ask the proxy administrator to allow this address, or add the host to no_proxy")
            }
        }
    }
}

/// What a failed request turned out to be; some outcomes still need to ask the proxy a question.
pub enum Outcome {
    Done(ToolError),
    ProxyLogin { target: ProxyTarget, wanted: String },
    ProxyRefused { target: ProxyTarget, wanted: String },
}

fn classify(
    inner: &Inner,
    error: &reqwest::Error,
    url: &str,
    who: &str,
    fix: &str,
    purpose: Purpose,
) -> Outcome {
    let shown = redact(url);
    let chain = error_chain(error);
    // A redirect that was refused.
    for e in &chain {
        if let Some(r) = e.downcast_ref::<RedirectRefused>() {
            return Outcome::Done(
                ToolError::new(format!(
                    "{} sent me to {}, and I will not follow it: {}",
                    r.from, r.to, r.reason
                ))
                .fix("serve the agent over https, or use the final address directly"),
            );
        }
    }
    // A certificate that cannot be trusted.
    for e in &chain {
        if let Some(rustls::Error::InvalidCertificate(problem)) = e.downcast_ref::<rustls::Error>()
        {
            // Through a proxy that is itself https, the certificate may be the proxy's: say so instead of guessing
            if let Some(t) = inner.proxy().ok().and_then(|p| {
                Url::parse(url)
                    .ok()
                    .and_then(|u| p.for_destination(&u).cloned())
            }) {
                if t.url.scheme() == "https" {
                    return Outcome::Done(
                        ToolError::new(format!(
                            "I could not trust a certificate while connecting to {} through the proxy {}",
                            shown,
                            t.display()
                        ))
                        .fix("add the authority of the proxy or of the server with ca_file in [network]; for development only you can set allow_self_signed = true"),
                    );
                }
            }
            return Outcome::Done(certificate_message(
                problem, &shown, who, purpose, &inner.cfg,
            ));
        }
    }
    // The proxy.
    let text = chain
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join(" | ");
    let target = inner.proxy().ok().and_then(|p| {
        Url::parse(url)
            .ok()
            .and_then(|u| p.for_destination(&u).cloned())
    });
    if let Some(target) = target {
        let wanted = Url::parse(url)
            .ok()
            .map(|u| {
                format!(
                    "{}:{}",
                    u.host_str().unwrap_or(""),
                    u.port_or_known_default().unwrap_or(443)
                )
            })
            .unwrap_or_default();
        if text.contains("proxy authorization required") {
            return Outcome::ProxyLogin { target, wanted };
        }
        if text.contains("tunnel error: unsuccessful") {
            return Outcome::ProxyRefused { target, wanted };
        }
        let name = target.display();
        if text.contains("failed to create underlying connection") || text.contains("proxy connect")
        {
            return Outcome::Done(
                ToolError::new(format!(
                    "I could not reach the proxy {}; {} was never contacted",
                    name, shown
                ))
                .fix("check url in [network.proxy], or the HTTPS_PROXY variable"),
            );
        }
        if error.is_timeout() && error.is_connect() {
            return Outcome::Done(
                ToolError::new(format!("the proxy {} did not answer in time", name))
                    .fix("check the proxy, or raise timeout_seconds in [runtime]"),
            );
        }
    }
    let why = if error.is_timeout() {
        "it took too long"
    } else if error.is_connect() {
        "the connection failed"
    } else {
        "the request did not complete"
    };
    Outcome::Done(
        ToolError::new(if who.is_empty() {
            format!("I could not reach {}: {}", shown, why)
        } else {
            format!("I could not reach {} ({}): {}", shown, who, why)
        })
        .fix(fix),
    )
}

fn error_chain(error: &reqwest::Error) -> Vec<&(dyn StdError + 'static)> {
    let mut out: Vec<&(dyn StdError + 'static)> = Vec::new();
    let mut next: Option<&(dyn StdError + 'static)> = Some(error);
    while let Some(e) = next {
        out.push(e);
        // io::Error hides its inner error from `source()`, so look inside it on purpose
        if let Some(io) = e.downcast_ref::<std::io::Error>() {
            if let Some(inner) = io.get_ref() {
                let inner: &(dyn StdError + 'static) = inner;
                next = Some(inner);
                continue;
            }
        }
        next = e.source();
    }
    out
}

fn date_of(seconds: u64) -> String {
    chrono::DateTime::from_timestamp(seconds as i64, 0)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "an earlier date".to_string())
}

/// A name from a certificate as people write it: `other.example` or `10.0.0.1`, without the library's wrapping.
fn plain_name(shown: &str) -> String {
    if let (Some(a), Some(b)) = (shown.find('"'), shown.rfind('"')) {
        if b > a {
            return shown[a + 1..b].to_string();
        }
    }
    shown
        .split(|c: char| !(c.is_ascii_hexdigit() || c == '.' || c == ':'))
        .find(|run| run.contains('.') || run.contains(':'))
        .unwrap_or(shown)
        .to_string()
}

fn certificate_message(
    problem: &CertificateError,
    shown: &str,
    who: &str,
    purpose: Purpose,
    cfg: &NetworkConfig,
) -> ToolError {
    match problem {
        CertificateError::Expired => ToolError::new(format!("the certificate of {} has expired", shown))
            .fix("renew the certificate on that server"),
        CertificateError::ExpiredContext { not_after, .. } => ToolError::new(format!(
            "the certificate of {} expired on {}",
            shown,
            date_of(not_after.as_secs())
        ))
        .fix("renew the certificate on that server"),
        CertificateError::NotValidYet | CertificateError::NotValidYetContext { .. } => {
            let when = match problem {
                CertificateError::NotValidYetContext { not_before, .. } => {
                    format!("until {}", date_of(not_before.as_secs()))
                }
                _ => "yet".to_string(),
            };
            ToolError::new(format!("the certificate of {} is not valid {}", shown, when))
                .fix("check the clock of this computer and of the server")
        }
        CertificateError::NotValidForName => ToolError::new(format!(
            "the certificate of {} is for a different name than the one in the address",
            shown
        ))
        .fix("use the name that is on the certificate, or for development only set allow_self_signed = true in [network]"),
        CertificateError::NotValidForNameContext { expected, presented } => {
            let wanted = expected.to_str().to_string();
            let names: Vec<String> = presented.iter().map(|n| plain_name(n)).collect();
            let have = if names.is_empty() {
                "none".to_string()
            } else {
                names.join(", ")
            };
            ToolError::new(format!(
                "the certificate of {} is for {}, but the address says {}",
                shown, have, wanted
            ))
            .fix("use the name that is on the certificate, or for development only set allow_self_signed = true in [network]")
        }
        _ => {
            if purpose == Purpose::Model {
                ToolError::new(format!(
                    "I could not trust the certificate of the language model server {}",
                    shown
                ))
                .fix(if cfg.allow_self_signed {
                    "allow_self_signed does not apply to the language model; set llm_allow_self_signed = true in [network] only if you trust this gateway, or add its authority with ca_file"
                } else {
                    "add the authority of that server with ca_file in [network], or for a private gateway you trust set llm_allow_self_signed = true"
                })
            } else {
                ToolError::new(format!(
                    "I could not trust the certificate of {} ({}): it was signed by an authority I do not know, for example a self-signed certificate",
                    shown, who
                ))
                .fix("add that authority with ca_file in [network]; for development only you can set allow_self_signed = true")
            }
        }
    }
}

// ---------- diagnosing a proxy that refused ----------

async fn connect_probe(target: &ProxyTarget, wanted: &str) -> Option<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    if target.url.scheme() != "http" {
        return None;
    }
    let host = target.url.host_str()?;
    let port = target.url.port_or_known_default()?;
    let work = async {
        let mut stream = tokio::net::TcpStream::connect((host, port)).await.ok()?;
        let mut request = format!("CONNECT {} HTTP/1.1\r\nHost: {}\r\n", wanted, wanted);
        if let Some((user, password)) = &target.login {
            use std::fmt::Write as _;
            let token = base64::engine::general_purpose::STANDARD
                .encode(format!("{}:{}", user.expose(), password.expose()).as_bytes());
            let _ = write!(request, "Proxy-Authorization: Basic {}\r\n", token);
        }
        request.push_str("\r\n");
        stream.write_all(request.as_bytes()).await.ok()?;
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") && head.len() < 8192 {
            if stream.read(&mut byte).await.ok()? == 0 {
                break;
            }
            head.push(byte[0]);
        }
        Some(String::from_utf8_lossy(&head).to_string())
    };
    tokio::time::timeout(Duration::from_secs(5), work)
        .await
        .ok()
        .flatten()
}

async fn probe_status(target: &ProxyTarget, wanted: &str) -> Option<u16> {
    let head = connect_probe(target, wanted).await?;
    head.lines().next()?.split_whitespace().nth(1)?.parse().ok()
}

async fn offered_schemes(target: &ProxyTarget, wanted: &str) -> Option<Vec<String>> {
    let head = connect_probe(target, wanted).await?;
    let mut out = Vec::new();
    for line in head.lines() {
        if let Some(rest) = line
            .to_ascii_lowercase()
            .strip_prefix("proxy-authenticate:")
        {
            let original = &line[line.len() - rest.len()..];
            if let Some(scheme) = original.split_whitespace().next() {
                out.push(scheme.to_string());
            }
        }
    }
    Some(out)
}

/// Resolves everything `metagente check` should report about the network, as plain lines.
pub fn check_lines(
    cfg: &NetworkConfig,
    env: &dyn Fn(&str) -> Option<String>,
) -> (Vec<String>, Vec<Diagnostic>) {
    let mut notes = Vec::new();
    let mut problems = Vec::new();
    match EffectiveProxy::resolve(&cfg.proxy, &cfg.file, env) {
        Ok(p) if p.source != ProxySource::None => notes.push(p.describe()),
        Ok(_) => {}
        Err(d) => problems.push(d),
    }
    (notes, problems)
}
