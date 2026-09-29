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
}

impl Served {
    pub fn new(
        rt: Arc<Runtime>,
        file: PathBuf,
        default_agent: Option<String>,
        agents: Vec<Arc<AgentDef>>,
    ) -> Served {
        rt.linker.register(&agents);
        Served {
            rt,
            file,
            default_agent,
            last_good: Mutex::new(agents),
        }
    }

    /// The current agents. If the file was edited into something broken, the last good version stays.
    pub fn agents(&self) -> Vec<Arc<AgentDef>> {
        match self.rt.linker.load(&self.file) {
            Ok(agents) if crate::lang::check::check_all(&agents).is_empty() => {
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
}

fn is_local(host: &str) -> bool {
    host == "127.0.0.1" || host == "localhost" || host == "::1"
}

/// Serves the agents until the user presses Ctrl-C (or, for MCP over stdio, until the client leaves).
pub async fn serve(opts: ServeOptions, rt: Arc<Runtime>) -> MgResult<()> {
    let agents = super::run::load_agents(&opts.file)?;
    rt.linker.register(&agents);
    let mut problems = crate::lang::check::check_all(&agents);
    problems.extend(crate::link::check::check_links(
        &rt.linker,
        &rt.config.root,
        &agents,
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
    let served = Arc::new(Served::new(
        rt.clone(),
        opts.file.clone(),
        opts.agent.clone(),
        agents.clone(),
    ));
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
        let base = format!("http://{}:{}", shown, port);
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
        tasks.spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
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
            let app = axum::Router::new().nest_service("/mcp", service);
            eprintln!(
                "MCP: http://{}:{}/mcp",
                if host == "0.0.0.0" {
                    "localhost"
                } else {
                    &host
                },
                port
            );
            tasks.spawn(async move {
                let _ = axum::serve(listener, app).await;
            });
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
