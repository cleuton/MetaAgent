# Metagente

**Version: v0.1.0**

Metagente is an interpreted fourth generation language (4GL) for building AI agents in minutes, with
little or no programming experience. The interpreter is written in Rust and speaks MCP (Model Context
Protocol) and A2A (Agent to Agent) natively.

## A first agent

```text
agent Weather
  goal "Answer questions about the weather"
  tool weather from mcp "npx -y weather-mcp"
  accepts ask city
  on ask
    forecast = weather.forecast city: city
    reply "In {city} it will be {forecast.summary}"
```

(The MCP server above is illustrative. See `specs/001-metagente-core/contracts/language.md`.)

## Status

The core is built and tested: the interpreter, the built in tools, MCP, dynamic link and A2A. See
[CHANGELOG.md](CHANGELOG.md) and [docs/tutorial.md](docs/tutorial.md).

```text
cargo build --release
target/release/metagente new hello
target/release/metagente run hello.ag greet name=World
```

## Roadmap

| Stage | Scope | State |
|-------|-------|-------|
| v0.1.0 | Constitution, spec, plan, contracts and task list for the core | Done |
| MVP | Tools and tasks (built in and MCP), `run`, `check`, `new`, beginner friendly errors, tutorial (user stories 1 and 2) | Built |
| Safe agents | Per-agent permissions: file folders, named environment variables, declared links (user story 3) | Built |
| Dynamic link | Agents calling agents by name or path, interface checks, cycle detection (user story 4) | Built |
| v1.0 | MCP server, A2A Agent Card, receive and send A2A tasks, local-only `serve` (user story 5) | Built; waiting for a first-time-user session (SC-001) and a tagged release |
| Later | Long running tasks with later result checks, authentication for served agents, central agent registry, scheduling, A2A streaming, `ListTasks` and `CancelTask` | Ideas, each needs its own spec |

A sample that puts it together, [City Briefing](samples/city-briefing/README.md), shows a Concierge agent
asking a Researcher agent over A2A while the Researcher reads a page through an MCP tool, both using Claude
(everything is tested except the run with the live Claude API, which needs your key).

The full task list is in `specs/001-metagente-core/tasks.md`.

## Project documents

| Document | Path |
|----------|------|
| Programming guide (if, loops, results) | `docs/guide.md` |
| Samples (start with `city-briefing`: two agents, A2A, MCP and Claude) | `samples/` |
| Constitution (principles) | `.specify/memory/constitution.md` |
| Feature spec | `specs/001-metagente-core/spec.md` |
| Implementation plan | `specs/001-metagente-core/plan.md` |
| Task list | `specs/001-metagente-core/tasks.md` |
| Language, CLI, configuration and protocol contracts | `specs/001-metagente-core/contracts/` |
| Validation guide | `specs/001-metagente-core/quickstart.md` |

## Versioning

Semantic versioning. The version is kept in this file, in `CHANGELOG.md`, and (once it exists) in
`Cargo.toml`.
