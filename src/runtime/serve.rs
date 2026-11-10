//! The agents being served: reloaded from disk when the file changes.

use super::Runtime;
use crate::lang::AgentDef;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct Served {
    pub rt: Arc<Runtime>,
    pub file: PathBuf,
    /// The agent shown at the top level (Agent Card, MCP). The first one by default.
    pub default_agent: Option<String>,
    last_good: Mutex<Vec<Arc<AgentDef>>>,
    // 0.1.3: how the listeners were started, so a later change that needs a restart can be mentioned once
    listener: Mutex<Option<ListenerWatch>>,
    // 0.1.4: [version] and [auth] as they were at start; a later edit is not applied and is mentioned once (D16)
    auth_watch: Mutex<AuthWatch>,
    // 0.1.5: the named models, [llm.NAME], as they were at start; a later edit is not applied and is mentioned once (D8)
    models_watch: Mutex<AuthWatch>,
}

struct AuthWatch {
    start: String,
    told: Option<String>,
}

/// 0.1.4: the raw text of the `[version]` and `[auth]` sections of the project's metagente.toml.
fn auth_signature(root: &std::path::Path) -> String {
    let mut dir = Some(root);
    while let Some(d) = dir {
        let candidate = d.join("metagente.toml");
        if candidate.is_file() {
            return std::fs::read_to_string(&candidate)
                .ok()
                .and_then(|t| t.parse::<toml::Table>().ok())
                .map(|t| format!("{:?}|{:?}", t.get("version"), t.get("auth")))
                .unwrap_or_default();
        }
        dir = d.parent();
    }
    String::new()
}

/// 0.1.5: the raw text of the sub-tables of `[llm]` (`[llm.NAME]`) in the project's metagente.toml. The scalars of `[llm]`
/// are not part of it: `[llm]` has no restart line, and does not get one in this release.
fn models_signature(root: &std::path::Path) -> String {
    let mut dir = Some(root);
    while let Some(d) = dir {
        let candidate = d.join("metagente.toml");
        if candidate.is_file() {
            return std::fs::read_to_string(&candidate)
                .ok()
                .and_then(|t| t.parse::<toml::Table>().ok())
                .and_then(|t| t.get("llm").and_then(|v| v.as_table()).cloned())
                .map(|llm| {
                    let tables: std::collections::BTreeMap<_, _> =
                        llm.iter().filter(|(_, v)| v.is_table()).collect();
                    format!("{:?}", tables)
                })
                .unwrap_or_default();
        }
        dir = d.parent();
    }
    String::new()
}

/// 0.1.3: what decides how a process answers: https or plain, and with which certificate and key.
#[derive(Debug, Clone, PartialEq)]
pub struct ListenerState {
    pub https: bool,
    pub cert: Option<PathBuf>,
    pub key: Option<PathBuf>,
}

struct ListenerWatch {
    start: ListenerState,
    flag_cert: Option<PathBuf>,
    flag_key: Option<PathBuf>,
    told: Option<ListenerState>,
}

impl Served {
    pub fn new(
        rt: Arc<Runtime>,
        file: PathBuf,
        default_agent: Option<String>,
        agents: Vec<Arc<AgentDef>>,
    ) -> Served {
        rt.linker.register(&agents);
        let auth_start = auth_signature(&rt.config.root); // 0.1.4
        let models_start = models_signature(&rt.config.root); // 0.1.5
        Served {
            rt,
            file,
            default_agent,
            last_good: Mutex::new(agents),
            listener: Mutex::new(None),
            auth_watch: Mutex::new(AuthWatch {
                start: auth_start,
                told: None,
            }),
            models_watch: Mutex::new(AuthWatch {
                start: models_start,
                told: None,
            }),
        }
    }

    /// 0.1.3: remembers how the listeners were started (the flags win over metagente.toml, D4).
    pub fn watch_listener(
        &self,
        start: ListenerState,
        flag_cert: Option<PathBuf>,
        flag_key: Option<PathBuf>,
    ) {
        if let Ok(mut w) = self.listener.lock() {
            *w = Some(ListenerWatch {
                start,
                flag_cert,
                flag_key,
                told: None,
            });
        }
    }

    /// 0.1.3: the listener mode and the certificate are read at start (D7). A change in metagente.toml that would
    /// alter them is not applied; one plain line says a restart is needed, once per new state.
    fn mention_listener_change(&self, fresh: &super::config::Config, agents: &[Arc<AgentDef>]) {
        let Ok(mut guard) = self.listener.lock() else {
            return;
        };
        let Some(watch) = guard.as_mut() else {
            return;
        };
        let now = listener_state(
            fresh,
            watch.flag_cert.as_deref(),
            watch.flag_key.as_deref(),
            agents,
        );
        if now == watch.start {
            watch.told = None;
        } else if watch.told.as_ref() != Some(&now) {
            eprintln!(
                "Note: metagente.toml changed the https settings (a certificate, a key or a secure agent). I keep serving as I started; restart `metagente serve` for the change to apply."
            );
            watch.told = Some(now);
        }
    }

    /// 0.1.4: `[version]` and `[auth]` are read at start only (D16). A later edit, even deleting the section, changes
    /// nothing; one plain line says a restart is needed, once per new state.
    fn mention_auth_change(&self) {
        let now = auth_signature(&self.rt.config.root);
        let Ok(mut watch) = self.auth_watch.lock() else {
            return;
        };
        if now == watch.start {
            watch.told = None;
        } else if watch.told.as_ref() != Some(&now) {
            eprintln!(
                "Note: metagente.toml changed [auth] (or [version]). I keep serving as I started; restart `metagente serve` for the change to apply."
            );
            watch.told = Some(now);
        }
    }

    /// 0.1.5: `[llm.NAME]` is read at start only (D8). A later edit changes nothing; one plain line says a restart is
    /// needed, once per new state.
    fn mention_models_change(&self) {
        let now = models_signature(&self.rt.config.root);
        let Ok(mut watch) = self.models_watch.lock() else {
            return;
        };
        if now == watch.start {
            watch.told = None;
        } else if watch.told.as_ref() != Some(&now) {
            eprintln!(
                "Note: metagente.toml changed the models under [llm]. I keep serving as I started; restart `metagente serve` for the change to apply."
            );
            watch.told = Some(now);
        }
    }

    /// The current agents. If the file was edited into something broken, the last good version stays.
    pub fn agents(&self) -> Vec<Arc<AgentDef>> {
        self.mention_auth_change(); // 0.1.4
        self.mention_models_change(); // 0.1.5
        // 0.1.2: a parameter edited in metagente.toml applies to the next request
        // 0.1.3: and so does a changed [network] or [secure] section; requests already running keep the settings they started with
        if let Ok(fresh) = super::config::Config::load_with_env(&self.rt.config.root, &|_| None) {
            self.rt.linker.set_parameters(fresh.parameters.clone());
            let current = self.last_good.lock().map(|l| l.clone()).unwrap_or_default();
            self.mention_listener_change(&fresh, &current); // 0.1.3
            if self.rt.net.reload_settings(fresh.network, fresh.secure) {
                self.rt.mcp.forget_all();
            }
        }
        match self.rt.linker.load(&self.file) {
            // 0.1.5: an edit that names a model not read at start is a broken edit: the last good version stays (D8)
            Ok(agents)
                if crate::lang::check::check_all(&agents).is_empty()
                    && crate::runtime::models::check_uses(&agents, &self.rt.config.models)
                        .is_empty() =>
            {
                if let Ok(mut last) = self.last_good.lock() {
                    *last = agents.clone();
                }
                agents
            }
            Ok(_) | Err(_) => self.last_good.lock().map(|l| l.clone()).unwrap_or_default(),
        }
    }

    pub fn find(&self, name: &str) -> Option<Arc<AgentDef>> {
        self.agents().into_iter().find(|a| a.name == name)
    }

    pub fn default(&self) -> Option<Arc<AgentDef>> {
        let agents = self.agents();
        match &self.default_agent {
            Some(name) => agents.iter().find(|a| &a.name == name).cloned(),
            None => agents.first().cloned(),
        }
    }
}

// ---------- `metagente serve` ----------

use crate::diagnostics::{Diagnostic, MgResult};

pub enum McpMode {
    Stdio,
    Port(u16),
}

pub struct ServeOptions {
    pub file: PathBuf,
    pub a2a: Option<u16>,
    pub mcp: Option<McpMode>,
    pub public: bool,
    pub agent: Option<String>,
    /// 0.1.3: `--tls-cert` and `--tls-key`; they win over `[serve]` in metagente.toml (D4)
    pub tls_cert: Option<PathBuf>,
    pub tls_key: Option<PathBuf>,
}

/// 0.1.3: how a process answers, from the settings and the agents it holds (D5, D6).
pub fn listener_state(
    config: &super::config::Config,
    flag_cert: Option<&std::path::Path>,
    flag_key: Option<&std::path::Path>,
    agents: &[Arc<AgentDef>],
) -> ListenerState {
    let secure_any = agents.iter().any(|a| config.secure.lists_agent(&a.name));
    let pair = super::tls::resolve(flag_cert, flag_key, &config.serve_tls)
        .ok()
        .and_then(|(p, _)| p);
    ListenerState {
        https: secure_any || pair.is_some(),
        cert: pair.as_ref().map(|p| p.cert.clone()),
        key: pair.as_ref().map(|p| p.key.clone()),
    }
}

fn is_local(host: &str) -> bool {
    host == "127.0.0.1" || host == "localhost" || host == "::1"
}

/// Serves the agents until the user presses Ctrl-C (or, for MCP over stdio, until the client leaves).
pub async fn serve(opts: ServeOptions, rt: Arc<Runtime>) -> MgResult<()> {
    let agents = super::run::load_agents_with(&opts.file, &rt.config.parameters)?; // 0.1.2: the project's parameters
    rt.linker.register(&agents);
    let mut problems = crate::lang::check::check_all(&agents);
    problems.extend(crate::link::check::check_links(
        &rt.linker,
        &rt.config.root,
        &agents,
    ));
    // 0.1.5: every `using NAME` must name a model of [llm.NAME]
    problems.extend(crate::runtime::models::check_uses(
        &agents,
        &rt.config.models,
    ));
    if !problems.is_empty() {
        let extra = problems.len() - 1;
        let mut first = problems.remove(0);
        if extra > 0 {
            first = first.related(format!(
                "({} more problems; run `metagente check {}` to see them all)",
                extra,
                opts.file.display()
            ));
        }
        return Err(first);
    }
    if let Some(name) = &opts.agent {
        if !agents.iter().any(|a| &a.name == name) {
            let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
            return Err(Diagnostic::new(format!(
                "{} has no agent called `{}`",
                opts.file.display(),
                name
            ))
            .fix(format!("it has: {}", names.join(", "))));
        }
    }
    // 0.1.3: https or plain is decided once, here (D5, D6, D7). A secure agent is never served over plain http.
    let list = rt.net.secure();
    let secure_names: Vec<&str> = agents
        .iter()
        .filter(|a| list.lists_agent(&a.name))
        .map(|a| a.name.as_str())
        .collect();
    if let (Some(name), Some(McpMode::Stdio)) = (secure_names.first(), &opts.mcp) {
        return Err(Diagnostic::new(format!(
            "{} is declared secure and stdio is not https.",
            name
        ))
        .fix("serve it with --mcp PORT and a certificate (tls_cert and tls_key in [serve], or --tls-cert and --tls-key)"));
    }
    // 0.1.4: a protected message is never served where a token has no place (D11): a pipe has no header
    if let (Some(policy), Some(McpMode::Stdio)) = (&rt.config.auth, &opts.mcp) {
        if let Some(what) = policy.first_protected(&agents) {
            return Err(Diagnostic::new(format!(
                "{} is protected, and stdio has no place for a token.",
                what
            ))
            .fix("serve it with --mcp PORT"));
        }
    }
    let (pair, tls_notes) = super::tls::resolve(
        opts.tls_cert.as_deref(),
        opts.tls_key.as_deref(),
        &rt.config.serve_tls,
    )?;
    if pair.is_none() {
        if let Some(name) = secure_names.first() {
            return Err(Diagnostic::new(format!(
                "{} is declared secure, so I will only serve it over https, but no certificate is set.",
                name
            ))
            .fix("set tls_cert and tls_key in [serve] of metagente.toml, or use --tls-cert and --tls-key"));
        }
    }
    let tls = match &pair {
        Some(p) => Some(super::tls::load(p)?),
        None => None,
    };
    for note in &tls_notes {
        eprintln!("Note: {}", note);
    }
    if let Some(loaded) = &tls {
        for w in &loaded.warnings {
            eprintln!("Warning: {}", w);
        }
    }
    if let (Some(name), true) = (secure_names.first(), secure_names.len() < agents.len()) {
        eprintln!(
            "Note: {} is declared secure, so every agent in this process is served over https only.",
            name
        );
    }
    let tls_config = tls.map(|l| l.config);
    let scheme = if tls_config.is_some() {
        "https"
    } else {
        "http"
    };
    let handshake = std::time::Duration::from_secs(rt.config.timeout_seconds.max(1));
    let served = Arc::new(Served::new(
        rt.clone(),
        opts.file.clone(),
        opts.agent.clone(),
        agents.clone(),
    ));
    served.watch_listener(
        listener_state(
            &rt.config,
            opts.tls_cert.as_deref(),
            opts.tls_key.as_deref(),
            &agents,
        ),
        opts.tls_cert.clone(),
        opts.tls_key.clone(),
    );
    let host = if opts.public {
        "0.0.0.0".to_string()
    } else {
        rt.config.bind.clone()
    };
    if !is_local(&host) {
        eprintln!(
            "Warning: these agents can be reached from other computers, and there is no login in this version. \
             Anyone who can connect can use every tool these agents declared."
        );
    }
    let a2a_port = match (&opts.a2a, &opts.mcp) {
        (Some(p), _) => Some(*p),
        (None, None) => Some(rt.config.a2a_port),
        (None, Some(_)) => None,
    };
    // 0.1.4: a protected message is never served in the open on a network (D11)
    if let Some(policy) = &rt.config.auth {
        if let Some(what) = policy.first_protected(&agents) {
            if tls_config.is_none() {
                if !is_local(&host) {
                    let port = a2a_port
                        .or(match &opts.mcp {
                            Some(McpMode::Port(p)) => Some(*p),
                            _ => None,
                        })
                        .unwrap_or(rt.config.a2a_port);
                    return Err(Diagnostic::new(format!(
                        "{} is protected, and this address is plain http and open to other computers: {}:{}. A token would travel in the open.",
                        what, host, port
                    ))
                    .fix(format!(
                        "serve with tls_cert and tls_key, or remove {} from [auth] protect",
                        what
                    )));
                }
                eprintln!(
                    "Warning: protected messages are served over plain http on this computer only. The token is not encrypted. For real use, serve with a certificate."
                );
            }
        }
    }
    let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
    eprintln!(
        "Serving {} ({}).",
        if agents.len() == 1 {
            "1 agent"
        } else {
            "agents"
        },
        names.join(", ")
    );
    let stop = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    let bind_error = |what: &str, addr: &str, e: std::io::Error| {
        Diagnostic::new(format!(
            "I could not listen for {} on {}: {}",
            what, addr, e
        ))
        .fix("choose another port, or stop the program that is using it")
    };
    let mut tasks = tokio::task::JoinSet::new();
    if let Some(port) = a2a_port {
        let addr = format!("{}:{}", host, port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| bind_error("A2A", &addr, e))?;
        let shown = if host == "0.0.0.0" {
            "localhost".to_string()
        } else {
            host.clone()
        };
        let base = format!("{}://{}:{}", scheme, shown, port); // 0.1.3: the scheme the listener really uses
        eprintln!(
            "A2A: {}   (agent card: {}/.well-known/agent-card.json)",
            base, base
        );
        let state = Arc::new(crate::a2a::server::A2aState {
            served: served.clone(),
            store: crate::a2a::store::TaskStore::default(),
            base,
        });
        let app = crate::a2a::server::router(state);
        match tls_config.clone() {
            Some(config) => {
                tasks.spawn(super::tls::serve(listener, app, config, handshake)); // 0.1.3
            }
            None => {
                tasks.spawn(async move {
                    let _ = axum::serve(listener, app).await;
                });
            }
        }
    }
    match opts.mcp {
        Some(McpMode::Port(port)) => {
            use rmcp::transport::streamable_http_server::{
                StreamableHttpServerConfig, StreamableHttpService,
                session::local::LocalSessionManager,
            };
            let addr = format!("{}:{}", host, port);
            let listener = tokio::net::TcpListener::bind(&addr)
                .await
                .map_err(|e| bind_error("MCP", &addr, e))?;
            let handler = crate::mcp::server::AgentsMcp::new(served.clone());
            let service: StreamableHttpService<crate::mcp::server::AgentsMcp, LocalSessionManager> =
                StreamableHttpService::new(
                    move || Ok(handler.clone()),
                    Arc::new(LocalSessionManager::default()),
                    StreamableHttpServerConfig::default(),
                );
            let mut app = axum::Router::new().nest_service("/mcp", service);
            // 0.1.4: the door that judges tokens in front of /mcp, only when something is protected (D24)
            if rt
                .config
                .auth
                .as_ref()
                .is_some_and(|p| !p.protect.is_empty())
            {
                app = app.layer(axum::middleware::from_fn_with_state(
                    served.clone(),
                    crate::mcp::gate::gate,
                ));
            }
            eprintln!(
                "MCP: {}://{}:{}/mcp",
                scheme, // 0.1.3
                if host == "0.0.0.0" {
                    "localhost"
                } else {
                    &host
                },
                port
            );
            match tls_config.clone() {
                Some(config) => {
                    tasks.spawn(super::tls::serve(listener, app, config, handshake)); // 0.1.3
                }
                None => {
                    tasks.spawn(async move {
                        let _ = axum::serve(listener, app).await;
                    });
                }
            }
        }
        Some(McpMode::Stdio) => {
            use rmcp::ServiceExt;
            eprintln!("MCP: speaking over standard input and output");
            let handler = crate::mcp::server::AgentsMcp::new(served.clone());
            let running = handler.serve(rmcp::transport::stdio()).await.map_err(|e| {
                Diagnostic::new(format!("I could not start the MCP connection: {}", e))
            })?;
            tokio::select! {
                _ = running.waiting() => {}
                _ = stop => {}
            }
            return Ok(());
        }
        None => {}
    }
    if tasks.is_empty() {
        return Err(
            Diagnostic::new("there is nothing to serve").fix("add --a2a PORT and/or --mcp stdio")
        );
    }
    stop.await;
    eprintln!("Stopped.");
    Ok(())
}
