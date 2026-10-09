//! Running agents: the interpreter and everything it needs.

pub mod config;
pub mod interpreter;
pub mod net; // 0.1.3: certificates, proxies and plain-language network failures
pub mod permissions;
pub mod run;
pub mod scaffold;
pub mod serve;
pub mod task;
pub mod think;
pub mod value;

use crate::llm::Llm;
use crate::tools::state::StateMap;
use config::Config;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Shared by every agent in one process.
pub struct Runtime {
    pub config: Config,
    pub llm: Option<Arc<dyn Llm>>,
    pub net: Arc<net::ClientSet>, // 0.1.3: one place for certificates, proxies and clients
    pub mcp: crate::mcp::McpPool,
    pub linker: crate::link::Linker,
    states: Mutex<HashMap<String, StateMap>>,
}

impl Runtime {
    pub fn new(config: Config, llm: Option<Arc<dyn Llm>>) -> Arc<Runtime> {
        let net = Arc::new(net::ClientSet::for_config(&config));
        Self::with_net(config, llm, net)
    }

    /// 0.1.3: like `new`, sharing the network clients with the language model provider that was built with them.
    pub fn with_net(
        config: Config,
        llm: Option<Arc<dyn Llm>>,
        net: Arc<net::ClientSet>,
    ) -> Arc<Runtime> {
        // 0.1.2: agents loaded with `link` read the parameters of the agent that loaded them
        let linker: crate::link::Linker = Default::default();
        linker.set_parameters(config.parameters.clone());
        Arc::new(Runtime {
            config,
            llm,
            net,
            mcp: Default::default(),
            linker,
            states: Mutex::new(HashMap::new()),
        })
    }

    /// The memory of one agent, shared by every request that agent handles.
    pub fn state_for(&self, agent: &str) -> StateMap {
        match self.states.lock() {
            Ok(mut states) => states.entry(agent.to_string()).or_default().clone(),
            Err(_) => Default::default(),
        }
    }

    /// Environment variables that agents may never read.
    pub fn hidden_env(&self) -> Vec<String> {
        let mut hidden = Vec::new();
        if let Some(name) = &self.config.llm.api_key_env {
            hidden.push(name.clone());
        }
        for name in ["ANTHROPIC_API_KEY", "OPENAI_API_KEY"] {
            hidden.push(name.to_string());
        }
        // 0.1.3: the proxy login is as private as the model key
        for name in [
            &self.config.network.proxy.username_env,
            &self.config.network.proxy.password_env,
        ]
        .into_iter()
        .flatten()
        {
            hidden.push(name.clone());
        }
        hidden
    }
}
