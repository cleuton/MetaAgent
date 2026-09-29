# Phase 0 Research: Metagente Core (MVP)

All `NEEDS CLARIFICATION` items from the spec were resolved during `/speckit-specify`
(sync tasks, config driven LLM provider, invisible concurrency). Items below are technical decisions.
Crate versions were read from the crates.io index on 2026-09-29; pin exact versions in `Cargo.toml`
and re-verify APIs when implementing.

## D1. Language surface: indentation based, sentence-like, no types

- **Decision**: Line oriented syntax with indentation for blocks, keywords `agent goal tool link
  remote accepts on reply think if for`, values are text, number, yes/no, list, record. No braces,
  no type annotations, no semicolons.
- **Rationale**: Principles I and VII; a beginner can read it aloud. Reference agent is 7 lines.
- **Alternatives**: braces/C-like (more ceremony); YAML/TOML config style (no logic, weak for
  handlers); embedding an existing language such as Lua (leaks a general purpose language, not a 4GL).

## D2. One call form for everything: `name.action key: value`

- **Decision**: Built in tools, MCP tools, linked agents, and A2A remote agents are all invoked with
  the same call. What a name refers to is set by its declaration (`tool`, `link`, `remote`).
- **Rationale**: Constitution III requires A2A to use "the same syntax used to call a local MCP tool".
  Uniformity also satisfies Principle VII.
- **Alternatives**: separate keywords per kind (`mcp.call`, `a2a.send`): more to learn.

## D3. Interpreter design: hand written lexer and recursive descent parser, tree walking evaluator

- **Decision**: No parser generator. A lexer emits indent/dedent tokens; recursive descent parser
  builds an AST; a tree walking evaluator runs handlers on tokio.
- **Rationale**: Full control of error messages, which is a hard requirement (FR-010, SC-007).
  Performance needs are modest; tool latency dominates.
- **Alternatives**: pest/lalrpop (generic errors, harder to make natural language); bytecode VM
  (unneeded complexity, constitution VI forbids a heavy VM).

## D4. Diagnostics

- **Decision**: All user errors are a `Diagnostic { line, column, message, suggestion }` built by us
  and rendered as plain text: the message, the offending source line with a marker under the place, and the fix. Internal errors are caught at the boundary and shown as "Something went
  wrong inside Metagente" plus a log file path; panics are caught with a hook, never printed raw.
- **Rationale**: FR-010, SC-007, Principle I (no raw Rust traces).
- **Alternatives**: anyhow/miette output directly (exposes internal chain, not plain language); ariadne
  (tried; its box drawing and `<unknown>` file label read as technical, so the excerpt is drawn by
  a small renderer of our own).

## D5. MCP: use `rmcp`, official Rust SDK

- **Decision**: `rmcp` 3.x for the client (spawn stdio server, connect HTTP, list tools, validate
  arguments against each tool's JSON schema, call) and for exposing agents as an MCP server
  (stdio and streamable HTTP).
- **Rationale**: Official, tracks the spec (Constitution "Open protocols"); avoids hand rolling
  transport and handshake, which the user must never see (Principle III).
- **Alternatives**: hand written JSON-RPC (more code, drift risk); other community crates (less
  authoritative).
- **Note**: rmcp 3.x implements MCP `2026-07-28` and remains compatible with `2025-11-25` and earlier.

## D6. A2A: implement a thin module over axum + serde, do not depend on the `a2a` 0.1.0 crate

- **Decision**: Implement Agent Card serving, `message/send` (JSON-RPC over HTTP), task state
  reporting, and an A2A client ourselves, following the public A2A specification.
- **Rationale**: The only dedicated crates found (`a2a` 0.1.0, `a2a-agents` 0.7.0,
  `inference-gateway-adk` 0.14.0) are early or framework-shaped; MVP needs a small subset and full
  control over spec conformance (SC-006, interop with non-Metagente agents).
- **Alternatives**: adopt a crate (faster start, unknown conformance and maintenance).
- **Risk**: the A2A spec has changed between versions. Target A2A 1.0.0 (Agent Card at
  `/.well-known/agent-card.json`, `SendMessage` method, `application/a2a+json`), pinned in
  `contracts/protocols.md`; contract tests pin the wire shapes. Streaming and push notifications are
  out of MVP scope.

## D7. LLM provider: config only, thin trait

- **Decision**: An internal `Llm` trait with a request/response shape covering "goal + message +
  tool descriptions → text or tool call". Providers are selected in `metagente.toml` or environment
  variables (see contracts/configuration.md); never named in `.ag` files. A deterministic fake
  provider is used in tests.
- **Rationale**: Spec FR-017; keeps agent files portable.
- **Alternatives**: `rig-core` (0.42.0) as abstraction: capable but brings its own agent concepts
  that compete with ours; revisit if provider count grows. Provider choice inside the `.ag` file:
  rejected by the spec answer.

## D8. Concurrency: tokio, hidden from the user

- **Decision**: Every incoming A2A/MCP request runs as its own tokio task with its own agent
  instance state; a handler body is executed sequentially. There is no user visible concurrency
  keyword.
- **Rationale**: FR-018, Principle I and VI.
- **Alternatives**: actor per agent with mailbox (more machinery than the MVP needs); blocking
  threads (poor for I/O bound MCP/A2A).

## D9. Tasks: synchronous with timeout

- **Decision**: A call awaits the result. Default timeout 30 s, override with `within 60 seconds` on a
  call or `timeout` in configuration. Timeout produces a Diagnostic-style runtime error and the tool
  call is cancelled.
- **Rationale**: FR-016 (spec answer: sync only in MVP). MCP tools that themselves run long simply
  hit the timeout.
- **Alternatives**: async task handles (deferred to a later spec).

## D10. Permissions: declared = granted

- **Decision**: An agent's capability set is exactly its `tool` declarations. Built in tools check
  the set at dispatch; `tool file` allows the project folder only, `tool file "data/"` narrows
  further; `tool env "NAME" ...` allows only the named variables (bare `tool env` is a syntax error
  with a fix hint) and the variable named in `api_key_env` is never readable; `tool http` allows outbound HTTP; `link`/`remote` allow only the declared targets. An
  undeclared use fails with "This agent did not declare `tool file`. Add the line `tool file`
  under the agent to allow it." A static pass in `metagente check` catches most cases before run.
- **Rationale**: FR-011 and the constitution's secure by default constraint, with no extra concept
  to learn.
- **Alternatives**: separate permission manifest (more ceremony, violates Principle I).

## D11. Dynamic link resolution and cycle detection

- **Decision**: `link Name` resolves in order: agent defined in the same file, `Name.ag` next to the
  calling file (relative path), `Name.ag` in the project `agents/` folder. A quoted path is used as
  is. Targets are loaded on first call and re-read when the file's modification time changes, so
  edits apply without restarting the caller (SC-005). The interface (`accepts` section) is checked
  before running. The call chain is a list of agent names carried in the call context; a repeated
  name aborts with a message showing the cycle (A -> B -> A).
- **Rationale**: FR-008, FR-009, FR-012, FR-015.
- **Alternatives**: central registry (deferred; adds setup for beginners).

## D12. Single binary distribution

- **Decision**: rustls everywhere (no OpenSSL), `cargo build --release` per target in a CI matrix;
  Windows and macOS builds native, Linux builds static (musl) where dependencies allow.
- **Rationale**: FR-013, Constitution portability constraint.

## D13. Testing strategy

- **Decision**: Unit tests for lexer, parser, permissions; integration tests that run real `.ag`
  files against an in-process fake MCP server (rmcp) and a fake LLM; contract tests asserting A2A
  and MCP wire shapes; an interop test against an independent A2A client for SC-006.
- **Rationale**: Constitution Workflow requires a working example per feature; examples double as
  integration tests.

## D14. Serving is local by default

- **Decision**: `metagente serve` binds to `127.0.0.1`. Listening on other interfaces requires the
  explicit `--public` flag, which prints a plain warning that no authentication exists in the MVP.
- **Rationale**: FR-019; served agents can hold file or http permissions, so an open port would be a
  remote capability with no login.
- **Alternatives**: token authentication in the MVP (deferred to a later spec); bind to all
  interfaces by default (rejected as unsafe).

## D15. `think` step limit

- **Decision**: `think` runs a loop of model call, tool call, model call. It stops at 10 steps
  (`think_max_steps` in configuration) with the message "The agent used N steps and did not finish.
  Make the goal more specific or raise think_max_steps." A malformed tool request from the model is
  reported to the model once as an error, then counted as a step.
- **Rationale**: FR-020; prevents runaway cost and loops.
- **Alternatives**: no limit (unsafe); time limit only (call timeouts already exist, but they do
  not bound spend).

## D16. Clock tool

- **Decision**: `clock.now` (returns ISO 8601 text and a `unix` number) and `clock.wait seconds: N`
  (sleeps, bounded by the call timeout). Scheduling repeated runs is left to a later spec; the
  constitution names "clock or scheduler" and the clock satisfies the minimal set for the MVP.
- **Rationale**: Constitution Principle IV; FR-004.
