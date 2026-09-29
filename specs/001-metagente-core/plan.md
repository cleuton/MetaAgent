# Implementation Plan: Metagente Core (MVP)

**Branch**: `001-metagente-core` | **Date**: 2026-09-29 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/001-metagente-core/spec.md`

## Summary

Build the Metagente interpreter: a single Rust binary that reads `.ag` files (an indentation based,
sentence-like language), and runs the agents in them. The MVP core loop is "an agent invokes a tool
and executes a task": built in tools (file, http, env, state, clock) and external MCP tools are called
through one uniform line, `name.action key: value`. The same call form reaches other local agents
(dynamic link) and remote A2A agents. Agents can be served over MCP and A2A, with permissions
derived from what each agent declares. Tasks are synchronous with a timeout, the LLM provider is
configured outside agent code, and concurrency is handled invisibly by the runtime
(see research.md for each decision).

## Technical Context

**Language/Version**: Rust (stable, edition 2024; toolchain in this environment is 1.92)

**Primary Dependencies**: tokio (async runtime), serde/serde_json/toml (data + config), reqwest with
rustls (HTTP tool, LLM calls, A2A client), axum (A2A + MCP HTTP serving), rmcp (official MCP Rust
SDK, client and server), clap (CLI)

**Storage**: N/A. The per agent state store is in memory for the MVP; files are accessed only through
the file tool

**Testing**: cargo test (unit for lexer/parser/permissions; integration with a fake MCP server and a
fake LLM; contract tests for A2A JSON shapes and the Agent Card)

**Target Platform**: Linux, macOS, Windows (x86_64 and aarch64), one static binary each

**Project Type**: CLI / language interpreter (single crate with modules)

**Performance Goals**: cold start to first agent line under 100 ms; parse a 500 line `.ag` file
under 50 ms; runtime overhead per tool call under 5 ms excluding the tool itself

**Constraints**: no OpenSSL or system libraries required (single binary); no Rust types, stack
traces, or panics visible to the user; default tool call timeout 30 s

**Scale/Scope**: single host, tens of concurrently served A2A/MCP requests, projects of up to a
few dozen agents; served agents listen on 127.0.0.1 unless `--public` is given

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Verified against `.specify/memory/constitution.md` (Metagente v1.0.0). Result: **PASS**, no violations.

- [x] **I. Simplicity**: the grammar (contracts/language.md) has no types, classes, or concurrency
      keywords; the reference weather agent is 7 lines; every error carries line, plain message, fix
- [x] **II. Agent-centric**: the only top level construct is `agent`; tools, links, and remotes are
      declared inside it
- [x] **III. MCP/A2A**: MCP client and server plus A2A card/receive/send are core modules; a call to
      an MCP tool, a linked agent, or an A2A agent is the same one line
- [x] **IV. Built-in tools**: file, http, env, state, clock each enable with one line (`tool file`); no
      others added to the core
- [x] **V. Dynamic link**: each agent declares an `accepts` section; the linker validates a call
      against it before running (FR-009) and tracks the call chain for cycles (FR-012)
- [x] **VI. Rust**: tokio/serde used freely internally; the user never sees Rust concepts
- [x] **VII. Readability**: one call form for tools, links, and remotes; one way to reply, ask, think
- [x] **Constraints**: portable (rustls, no system deps); per-agent permission set derived from
      declarations; MCP/A2A follow public specs; whole project distributable as a folder of `.ag`
      files plus the binary
- [x] **Workflow**: `examples/` ships a beginner `.ag` per feature; grammar changes update parser,
      example, and docs together (enforced in tasks)

**Re-check after Phase 1 design**: PASS. The design added no new abstraction layers visible to users.

## Project Structure

### Documentation (this feature)

```text
specs/001-metagente-core/
├── plan.md              # This file
├── research.md          # Phase 0 decisions
├── data-model.md        # Phase 1 entities and state
├── quickstart.md        # Phase 1 validation guide
├── contracts/
│   ├── language.md      # .ag grammar and semantics
│   ├── cli.md           # command line contract
│   ├── configuration.md # metagente.toml and environment
│   └── protocols.md     # MCP + A2A surfaces exposed and consumed
└── tasks.md             # Phase 2 output (/speckit-tasks, not created here)
```

### Source Code (repository root)

```text
Cargo.toml
src/
├── main.rs              # CLI entry (run, check, serve)
├── lang/                # lexer (indentation aware), parser, ast
├── diagnostics/         # natural language errors + source snippets
├── runtime/             # interpreter, agent instances, call chain, permissions, timeouts
├── tools/               # built in tools: file, http, env, state, clock
├── mcp/                 # MCP client (external tools) and server (expose agents)
├── a2a/                 # Agent Card, receive task server, send task client
├── link/                # dynamic link: resolve, interface check, cycle detection
└── llm/                 # provider trait, configuration driven providers, fake for tests

tests/
├── contract/            # A2A + MCP wire shapes, Agent Card
├── integration/         # end to end .ag runs against fake MCP server / fake LLM
└── unit/

examples/                # beginner .ag files, one per feature (constitution Workflow)
docs/                    # syntax documentation and tutorial
.github/workflows/      # CI: cross-platform build, examples check, grammar-change check
```

**Structure Decision**: one crate with modules, not a workspace. The MVP does not need separate
publishable crates, and one crate keeps building a single binary per platform (FR-013) trivial.
Modules can be promoted to crates later without changing behavior.

## Complexity Tracking

No constitution violations to justify.
