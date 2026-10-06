# Metagente

**Build AI agents in a few lines, not a few hundred.**

Metagente is a small language for building AI agents. You describe what the agent is for, which tools it may
use and what it answers; the interpreter does the rest. It speaks **MCP** (Model Context Protocol) and **A2A**
(Agent to Agent) natively, so your agents can use any MCP tool and talk to other agents out of the box.

**Demonstration video**
[![Metagente demo](https://img.youtube.com/vi/zrNYqy84Ak8/maxresdefault.jpg)](https://www.youtube.com/watch?v=zrNYqy84Ak8)

This demonstration is in [**multi-agent-samples**](./multi-agent-samples/metagente-demo-recrutador/)

The interpreter is a single binary written in Rust. No Python environment, no framework to learn.

<!-- DEMO: replace with a GIF of writing weather.ag and running it (10 to 15 seconds). -->
![Demo](docs/demo.gif)

```text
agent Weather
  goal "Answer questions about the weather"
  tool weather from mcp "npx -y weather-mcp"
  accepts ask city
  on ask
    forecast = weather.forecast city: city
    reply "In {city} it will be {forecast.summary}"
```

That is a complete agent: a goal, an MCP tool, an input and a reply.

## Try it in 30 seconds

1. Download the zip for your system from the [latest release](https://github.com/cleuton/MetaAgent/releases/latest):
   `metagente-linux-amd64.zip`, `metagente-macos-arm64.zip` or `metagente-windows-amd64.zip`.
2. Unzip it and open a terminal in the folder.
3. Run the first sample:

```text
./bin/metagente run samples/clock.ag now
```

On Windows use `.\bin\metagente.exe` instead. Then create your own agent:

```text
./bin/metagente new hello
./bin/metagente run hello.ag greet name=World
```

Prefer to build from source? You need [Rust](https://rustup.rs):

```text
git clone https://github.com/cleuton/MetaAgent
cd MetaAgent
cargo build --release
target/release/metagente run examples/clock.ag now
```

## Why Metagente?

Frameworks such as LangChain or CrewAI are powerful, but they assume you are a programmer working inside a
Python project. Metagente makes a different trade:

| | Python agent frameworks | Metagente |
|---|---|---|
| What you write | Python code using a library | A short declarative agent file |
| Install | Python, virtual env, packages | One binary |
| Tools | Framework specific wrappers | Any MCP server |
| Agents talking to agents | Usually in-process | A2A, across processes and machines |
| Safety | Up to your code | Per-agent permissions for folders, env vars and links |
| Errors | Stack traces | Beginner friendly messages |

If you need fine control in Python, use a framework. If you want a working agent quickly, or you want to give
non-programmers a way to build agents, Metagente is for you.

## What works today

- **Language and interpreter:** goals, tools, inputs, replies, `if`, loops and results.
- **Built in tools and MCP tools:** plug in any MCP server.
- **Safe agents:** each agent declares which folders, environment variables and other agents it may use.
- **Dynamic link:** agents call agents by name or path, with interface checks and cycle detection.
- **A2A and MCP server:** expose an agent as an MCP server or with an A2A Agent Card, and send or receive A2A tasks.
- **CLI:** `new`, `check`, `run` and `serve`.
- **Multi-line text** (new in 0.1.2): a prompt with many lines between `"""` and `"""`, kept exactly as typed.
- **External parameters** (new in 0.1.2): addresses and prompts live in `metagente.toml` and agents read them as `@parameters.name`.

### New in 0.1.2

A prompt with many lines, written as normal lines. It reaches the model exactly as typed
([examples/multiline.ag](examples/multiline.ag)):

```text
agent Recruiter
  goal "Screen candidate resumes"
  accepts screen resume
  on screen
    reply think """You are a technical recruiter.
Read the resume below and list its three strongest points.

Resume:
{resume}"""
```

An address and a prompt that change from one computer to another, kept out of the agent. They are entries of
`[parameters]` in `metagente.toml`, in the agents' folder ([examples/parameters.ag](examples/parameters.ag)):

```toml
[parameters]
a2a_leitor = "http://127.0.0.1:8080"
prompt1 = "you are a recruiter..."
```

```text
agent Screener
  goal "Screen a resume with the help of a remote reader"
  remote leitor at @parameters.a2a_leitor
  accepts screen candidate
  on screen
    resume = leitor.read candidate: candidate
    reply think @parameters.prompt1
```

An agent loaded with `link` reads the `metagente.toml` of the agent that loaded it. Details are in
[docs/syntax.md](docs/syntax.md#parameters).

## A complete example

[City Briefing](samples/city-briefing/README.md) puts it all together: a Concierge agent asks a Researcher
agent over A2A, and the Researcher reads a web page through an MCP tool. Both use Claude. Bring your own API key.

## Learn more

| Topic | Where |
|-------|-------|
| Tutorial, step by step | [docs/tutorial.md](docs/tutorial.md) |
| Programming guide (if, loops, results) | [docs/guide.md](docs/guide.md) |
| Language syntax, including multi-line text and parameters (new in 0.1.2) | [docs/syntax.md](docs/syntax.md) |
| Samples | [samples/](samples/) |
| Changelog | [CHANGELOG.md](CHANGELOG.md) |

## Roadmap

| Stage | State |
|-------|-------|
| Core language, built in and MCP tools, CLI, tutorial | Built |
| Safe agents (per-agent permissions) | Built |
| Dynamic link between agents | Built |
| Multi-line text and external parameters (v0.1.2) | Built |
| v1.0: MCP server, A2A Agent Card, A2A tasks, `serve` | Built, release pending |
| **Project Barracuda:** an agent server invoked via A2A or a frontend, with a queue for batch triggering | Next |
| Long running tasks, authentication for served agents, agent registry, scheduling, A2A streaming | Ideas |

## How it is built

Metagente is developed with spec-driven development using [GitHub Spec Kit](https://github.com/github/spec-kit).
Every feature starts as a spec, so you can read why things are the way they are:

| Document | Path |
|----------|------|
| Constitution (principles) | `.specify/memory/constitution.md` |
| Feature spec | `specs/001-metagente-core/spec.md` |
| Implementation plan | `specs/001-metagente-core/plan.md` |
| Task list | `specs/001-metagente-core/tasks.md` |
| Language, CLI, configuration and protocol contracts | `specs/001-metagente-core/contracts/` |
| Validation guide | `specs/001-metagente-core/quickstart.md` |
| v0.1.2 feature spec (multi-line text and parameters) | `specs/004-multiline-text-and-parameters/spec.md` |
| v0.1.2 plan, tasks and contracts | `specs/004-multiline-text-and-parameters/` |

## Contributing

Issues, ideas and pull requests are welcome. If you build an agent with Metagente, open an issue and show it;
good ones become samples. 

If Metagente is useful to you, a star on the repository helps other people find it.

## Version

**v0.1.2.** Semantic versioning. The version is kept in this file, in `CHANGELOG.md` and in `Cargo.toml`.