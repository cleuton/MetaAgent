//! Running an agent file from start to finish (used by `metagente run`).

use super::Runtime;
use super::config::Config;
use super::interpreter::Agent;
use super::task::{CallContext, new_task_id};
use super::value::Value;
use crate::diagnostics::{Diagnostic, MgResult};
use crate::lang::{AgentDef, Params, parse_file_with}; // 0.1.2: parse with parameters
use crate::tools::Args;
use std::path::Path;
use std::sync::Arc;

/// Reads and parses a `.ag` file.
pub fn load_agents(path: &Path) -> MgResult<Vec<Arc<AgentDef>>> {
    // 0.1.2: no parameters, as in 0.1.1
    load_agents_with(path, &Params::default())
}

/// 0.1.2: like `load_agents`; `@parameters.name` takes its value from `params`.
pub fn load_agents_with(path: &Path, params: &Params) -> MgResult<Vec<Arc<AgentDef>>> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        let why = if e.kind() == std::io::ErrorKind::NotFound {
            "the file does not exist".to_string()
        } else {
            e.to_string()
        };
        Diagnostic::new(format!("I could not open {}: {}", path.display(), why))
            .fix("check the file name and the folder you are in")
    })?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok(
        parse_file_with(&name, Some(path.to_path_buf()), &text, params)?
            .into_iter()
            .map(Arc::new)
            .collect(),
    )
}

pub struct RunOptions {
    pub file: std::path::PathBuf,
    pub message: Option<String>,
    pub params: Vec<String>,
    pub agent: Option<String>,
}

pub fn parse_params(params: &[String]) -> MgResult<Args> {
    let mut args = Args::new();
    for p in params {
        let Some((key, value)) = p.split_once('=') else {
            return Err(Diagnostic::new(format!("`{}` is not a key=value pair", p))
                .fix("write values like: city=Lisbon"));
        };
        args.insert(key.trim().to_string(), Value::Text(value.to_string()));
    }
    Ok(args)
}

pub fn pick_agent(
    agents: &[Arc<AgentDef>],
    wanted: Option<&str>,
    file: &Path,
) -> MgResult<Arc<AgentDef>> {
    match wanted {
        Some(name) => agents
            .iter()
            .find(|a| a.name == name)
            .cloned()
            .ok_or_else(|| {
                let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
                Diagnostic::new(format!("{} has no agent called `{}`", file.display(), name))
                    .fix(format!("it has: {}", names.join(", ")))
            }),
        None => agents.first().cloned().ok_or_else(|| {
            Diagnostic::new("this file has no agent in it")
                .fix("start with a line like: agent Helper")
        }),
    }
}

/// Builds the runtime for a project: configuration, and the model when one is set up.
pub fn build_runtime(start: &Path) -> MgResult<Arc<Runtime>> {
    let config = Config::load(start)?;
    let llm = crate::llm::providers::from_config(&config)?;
    Ok(Runtime::new(config, llm))
}

pub async fn run_file(opts: &RunOptions, rt: Arc<Runtime>) -> MgResult<Value> {
    let agents = load_agents_with(&opts.file, &rt.config.parameters)?; // 0.1.2: the project's parameters
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
                "({} more problem{} in this file; run `metagente check {}` to see them all)",
                extra,
                if extra == 1 { "" } else { "s" },
                opts.file.display()
            ));
        }
        return Err(first);
    }
    let def = pick_agent(&agents, opts.agent.as_deref(), &opts.file)?;
    // `run file.ag question=...` : the first word is a value, so the message was left out.
    let (given_message, params) = match &opts.message {
        Some(m) if m.contains('=') => (
            None,
            std::iter::once(m.clone())
                .chain(opts.params.iter().cloned())
                .collect::<Vec<_>>(),
        ),
        other => (other.clone(), opts.params.clone()),
    };
    let message = match &given_message {
        Some(m) => m.clone(),
        None if def.accepts.len() == 1 => def.accepts[0].message.clone(),
        None => {
            let names: Vec<&str> = def.accepts.iter().map(|a| a.message.as_str()).collect();
            return Err(Diagnostic::new(format!("agent {} needs to be told what to do", def.name)).fix(format!(
                "add the message after the file name, for example: metagente run {} {}   (it accepts: {})",
                opts.file.display(),
                names.first().copied().unwrap_or("message"),
                names.join(", ")
            )));
        }
    };
    let args = parse_params(&params)?;
    let agent = Agent::new(rt, def)?;
    let ctx = CallContext::new(new_task_id());
    agent.start(&ctx).await?;
    agent.handle(&message, args, &ctx).await
}

/// How a reply reads on the terminal.
pub fn show_value(value: &Value) -> Option<String> {
    match value {
        Value::Nothing => None,
        Value::Text(t) => Some(t.clone()),
        other => Some(
            serde_json::to_string_pretty(&other.to_json()).unwrap_or_else(|_| other.to_display()),
        ),
    }
}
