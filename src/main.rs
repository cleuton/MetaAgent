use clap::{Parser, Subcommand};
use metagente::diagnostics::internal;
use metagente::runtime::run::{RunOptions, build_runtime, run_file, show_value};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "metagente", version, about = "Build AI agents in minutes")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run an agent: metagente run weather.ag ask city=Lisbon
    Run {
        /// The .ag file
        file: PathBuf,
        /// What to tell the agent (one of its `accepts` messages)
        message: Option<String>,
        /// Values for the message, written key=value
        params: Vec<String>,
        /// Which agent in the file to run (the first by default)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Keep agents running so other programs can reach them over A2A and MCP
    Serve {
        file: PathBuf,
        #[arg(long)]
        a2a: Option<u16>,
        /// "stdio" or a port number
        #[arg(long)]
        mcp: Option<String>,
        /// Accept connections from other machines (there is no authentication)
        #[arg(long)]
        public: bool,
        /// Which agent to show at the top level (the first by default)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Check a file for problems without running it
    Check { file: PathBuf },
    /// Create a starter agent and a metagente.toml
    New { name: String },
}

fn project_dir_of(file: &std::path::Path) -> PathBuf {
    let _ = file;
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[tokio::main]
async fn main() -> ExitCode {
    internal::install_panic_hook();
    let cli = Cli::parse();
    match cli.command {
        Command::Run {
            file,
            message,
            params,
            agent,
        } => {
            // Lets the test suite prove that internal failures never show Rust text.
            if std::env::var_os("METAGENTE_INTERNAL_TEST_PANIC").is_some() {
                panic!("deliberate internal failure for testing");
            }
            let rt = match build_runtime(&project_dir_of(&file)) {
                Ok(rt) => rt,
                Err(d) => {
                    eprint!("{}", d.render());
                    return ExitCode::from(1);
                }
            };
            for w in &rt.config.warnings {
                eprintln!("Note: {}", w);
            }
            let opts = RunOptions {
                file,
                message,
                params,
                agent,
            };
            match run_file(&opts, rt).await {
                Ok(value) => {
                    if let Some(text) = show_value(&value) {
                        println!("{}", text);
                    }
                    ExitCode::SUCCESS
                }
                Err(d) => {
                    eprint!("{}", d.render());
                    ExitCode::from(1)
                }
            }
        }
        Command::Check { file } => match metagente::runtime::run::load_agents_with(
            &file,
            // 0.1.2: the parameters of the project; a broken metagente.toml was ignored by `check` before, and still is
            &metagente::runtime::config::Config::load_parameters(&project_dir_of(&file))
                .unwrap_or_default(),
        ) {
            Err(d) => {
                eprint!("{}", d.render());
                ExitCode::from(1)
            }
            Ok(agents) => {
                let mut problems = metagente::lang::check::check_all(&agents);
                if let Ok(rt) =
                    build_runtime(&std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
                {
                    rt.linker.register(&agents);
                    problems.extend(metagente::link::check::check_links(
                        &rt.linker,
                        &rt.config.root,
                        &agents,
                    ));
                    problems.sort_by_key(|d| (d.line, d.column));
                }
                if problems.is_empty() {
                    let n = agents.len();
                    println!(
                        "No problems found in {} ({} agent{}).",
                        file.display(),
                        n,
                        if n == 1 { "" } else { "s" }
                    );
                    ExitCode::SUCCESS
                } else {
                    for p in &problems {
                        eprintln!("{}", p.render());
                    }
                    eprintln!(
                        "Found {} problem{}.",
                        problems.len(),
                        if problems.len() == 1 { "" } else { "s" }
                    );
                    ExitCode::from(1)
                }
            }
        },
        Command::New { name } => {
            let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            match metagente::runtime::scaffold::create(&dir, &name) {
                Ok(lines) => {
                    for l in lines {
                        println!("{}", l);
                    }
                    ExitCode::SUCCESS
                }
                Err(d) => {
                    eprint!("{}", d.render());
                    ExitCode::from(1)
                }
            }
        }
        Command::Serve {
            file,
            a2a,
            mcp,
            public,
            agent,
        } => {
            use metagente::runtime::serve::{McpMode, ServeOptions, serve};
            let mode = match mcp.as_deref() {
                None => None,
                Some("stdio") => Some(McpMode::Stdio),
                Some(port) => match port.parse::<u16>() {
                    Ok(p) => Some(McpMode::Port(p)),
                    Err(_) => {
                        eprintln!(
                            "Problem: --mcp needs `stdio` or a port number, but got `{}`.",
                            port
                        );
                        return ExitCode::from(2);
                    }
                },
            };
            let rt = match build_runtime(&project_dir_of(&file)) {
                Ok(rt) => rt,
                Err(d) => {
                    eprint!("{}", d.render());
                    return ExitCode::from(1);
                }
            };
            for w in &rt.config.warnings {
                eprintln!("Note: {}", w);
            }
            match serve(
                ServeOptions {
                    file,
                    a2a,
                    mcp: mode,
                    public,
                    agent,
                },
                rt,
            )
            .await
            {
                Ok(()) => ExitCode::SUCCESS,
                Err(d) => {
                    eprint!("{}", d.render());
                    ExitCode::from(1)
                }
            }
        }
    }
}
