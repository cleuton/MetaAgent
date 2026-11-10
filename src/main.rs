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
    // 0.1.4: the help of serve, new and --public now mentions [auth] and [version]
    /// Keep agents running so other programs can reach them over A2A and MCP (messages in [auth] need a bearer token)
    Serve {
        file: PathBuf,
        #[arg(long)]
        a2a: Option<u16>,
        /// "stdio" or a port number
        #[arg(long)]
        mcp: Option<String>,
        /// Accept connections from other machines (protect messages with [auth] in metagente.toml)
        #[arg(long)]
        public: bool,
        /// Which agent to show at the top level (the first by default)
        #[arg(long)]
        agent: Option<String>,
        /// 0.1.3: PEM certificate to serve over https (wins over tls_cert in metagente.toml; needs --tls-key or tls_key)
        #[arg(long = "tls-cert", value_name = "FILE")]
        tls_cert: Option<PathBuf>,
        /// 0.1.3: PEM private key to serve over https (wins over tls_key in metagente.toml)
        #[arg(long = "tls-key", value_name = "FILE")]
        tls_key: Option<PathBuf>,
    },
    /// Check a file for problems without running it
    Check { file: PathBuf },
    /// Create a starter agent and a metagente.toml (with [version] and an [auth] example)
    New { name: String },
}

/// 0.1.3: certificate checks that are off are announced on every run, serve and check, and the agent cannot silence them.
fn print_network_warnings(net: &metagente::runtime::config::NetworkConfig) {
    for w in metagente::runtime::net::startup_warnings(net) {
        eprintln!("Warning: {}", w);
    }
}

/// 0.1.4: warnings about [version] and [auth] are shown on every run, serve and check.
fn print_auth_warnings(warnings: &[String]) {
    for w in warnings {
        eprintln!("Warning: {}", w);
    }
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
            print_network_warnings(&rt.config.network); // 0.1.3
            print_auth_warnings(&rt.config.auth_warnings); // 0.1.4
            print_auth_warnings(&rt.config.model_warnings); // 0.1.5: [llm.NAME] without [version]
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
        Command::Check { file } => {
            // 0.1.4: [version] and [auth] are read first, and their mistakes stop `check` like they stop run and serve
            let auth_check =
                match metagente::runtime::config::Config::auth_for_check(&project_dir_of(&file)) {
                    Ok(c) => c,
                    Err(d) => {
                        eprint!("{}", d.render());
                        return ExitCode::from(1);
                    }
                };
            // 0.1.5: [llm.NAME] is read like run and serve read it, so the same mistakes stop `check`
            let models_check = match metagente::runtime::config::Config::models_for_check(
                &project_dir_of(&file),
            ) {
                Ok(m) => m,
                Err(d) => {
                    eprint!("{}", d.render());
                    return ExitCode::from(1);
                }
            };
            match metagente::runtime::run::load_agents_with(
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
                    // 0.1.3: [network] problems are errors; certificate checks that are off are warnings
                    let mut network_problems = Vec::new();
                    match metagente::runtime::config::Config::network_for_check(&project_dir_of(
                        &file,
                    )) {
                        Err(d) => network_problems.push(d),
                        Ok(net) => {
                            print_network_warnings(&net);
                            if let Some(w) = metagente::runtime::net::pass_to_tools_warning(&net) {
                                eprintln!("Warning: {}", w);
                            }
                            let (notes, problems) =
                                metagente::runtime::net::check_lines(&net, &|n| {
                                    std::env::var(n).ok()
                                });
                            for n in &notes {
                                println!("Note: {}", n);
                            }
                            network_problems.extend(problems);
                        }
                    }
                    let mut problems = metagente::lang::check::check_all(&agents);
                    problems.extend(network_problems);
                    // 0.1.5: every `using NAME` must name a model of [llm.NAME]; the models a project uses but whose
                    // variables are not set here are only a warning (check may run where the keys are not)
                    print_auth_warnings(&models_check.warnings);
                    problems.extend(metagente::runtime::models::check_uses(
                        &agents,
                        &models_check.models,
                    ));
                    let used =
                        metagente::runtime::models::used_models(&agents, &models_check.models);
                    print_auth_warnings(&metagente::runtime::models::unset_warnings(
                        &used,
                        &models_check.models,
                        &|n| std::env::var(n).ok(),
                    ));
                    // 0.1.4: [auth] against the agents of the project, and the warnings and notes of [version] and [auth]
                    print_auth_warnings(&auth_check.warnings);
                    if let Some(policy) = &auth_check.policy {
                        let report =
                            metagente::runtime::auth_check::validate(policy, &auth_check.params);
                        print_auth_warnings(&report.warnings);
                        for n in &report.notes {
                            println!("Note: {}", n);
                        }
                        if metagente::runtime::auth_check::validator_program_exists(policy)
                            == Some(false)
                        {
                            eprintln!(
                                "Warning: the validator program in [auth] was not found: {}",
                                policy.validator.clone().unwrap_or_default()
                            );
                        }
                        problems.extend(report.errors);
                    }
                    // 0.1.3: the secure list: names that match nothing, local tools that cannot be secured, http addresses
                    let secure =
                        metagente::runtime::secure::check_project(&agents, &project_dir_of(&file));
                    for w in &secure.warnings {
                        eprintln!("Warning: {}", w);
                    }
                    for n in &secure.notes {
                        println!("Note: {}", n);
                    }
                    problems.extend(secure.problems);
                    if let Ok(rt) = build_runtime(
                        &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                    ) {
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
            }
        }
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
            tls_cert, // 0.1.3
            tls_key,  // 0.1.3
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
            print_network_warnings(&rt.config.network); // 0.1.3
            print_auth_warnings(&rt.config.auth_warnings); // 0.1.4
            print_auth_warnings(&rt.config.model_warnings); // 0.1.5: [llm.NAME] without [version]
            match serve(
                ServeOptions {
                    file,
                    a2a,
                    mcp: mode,
                    public,
                    agent,
                    tls_cert, // 0.1.3
                    tls_key,  // 0.1.3
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
