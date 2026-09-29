---

description: "Task list for Metagente Core (MVP)"
---

# Tasks: Metagente Core (MVP)

**Input**: Design documents from `/specs/001-metagente-core/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. plan.md (research D13) defines the testing strategy and the constitution requires a
working beginner example per feature, so examples double as integration tests.

**Organization**: Tasks are grouped by user story. Paths are relative to the repository root
(`/home/cleuton/Documents/projetos/metagente`), single crate layout from plan.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1..US5, matching the user stories in spec.md

---

## Phase 1: Setup

**Purpose**: Project skeleton

- [X] T001 Create `Cargo.toml` (edition 2024, package `metagente`, binary `metagente`) and the module folders from plan.md: `src/{lang,diagnostics,runtime,tools,mcp,a2a,link,llm}/mod.rs`, `tests/{contract,integration,unit}/`, `examples/`, `docs/` (FR-013)
- [X] T002 Add dependencies to `Cargo.toml`: tokio, serde, serde_json, toml, reqwest (rustls, no default OpenSSL), axum, rmcp 3.x, clap; pin exact versions found with `cargo info` (FR-013)
- [X] T003 [P] Add `rustfmt.toml`, clippy config (`#![deny(clippy::unwrap_used)]` outside tests) and `.gitignore` for `target/` (infrastructure)
- [X] T004 [P] Add CI matrix `.github/workflows/build.yml` building release binaries for Linux (musl), macOS, Windows, running `cargo test` (FR-013, SC-004)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Language front end, diagnostics, values, and runtime core that every story needs

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T005 [P] Define `Diagnostic { line, column, message, suggestion, related }` and a plain text renderer (message, source line with marker, fix) in `src/diagnostics/mod.rs` (contracts/language.md "Errors") (FR-010, SC-007)
- [X] T006 [P] Define `Value` (text, number, yes/no, list, record, nothing) in `src/runtime/value.rs` with conversion to/from `serde_json::Value` (FR-002)
- [X] T007 [P] Define the AST (agent, declarations, handlers, statements, expressions) in `src/lang/ast.rs` per contracts/language.md, with source spans on every node (FR-002)
- [X] T008 Implement the indentation aware lexer (2 space indents, `#` comments, text with `{name}` insertion) in `src/lang/lexer.rs`; lexical errors return plain `Diagnostic`s (FR-001, FR-010)
- [X] T009 Implement the recursive descent parser for the full grammar (declarations, `on` handlers, `name.action key: value [within N seconds]`, `reply`, `think`, `if/otherwise`, `for`, `fail`, expressions, and `tool env "NAME" ...` where bare `tool env` is a syntax error with a fix hint) in `src/lang/parser.rs`; syntax errors return plain `Diagnostic`s with a suggested fix (FR-002, FR-010, FR-014)
- [X] T010 [P] Unit tests for lexer and parser (valid files, each error kind, multi-agent file) in `tests/unit/parser_tests.rs` (FR-002, FR-014)
- [X] T011 Define `Capability` set derived from an agent's declarations and `Permissions::check(agent, tool, action)` returning a plain-language `Diagnostic` in `src/runtime/permissions.rs` (FR-011)
- [X] T012 Define `CallContext` (call chain list, task id, deadline) and `Task` state machine (pending, running, completed, failed, timed_out) in `src/runtime/task.rs` (FR-016)
- [X] T013 Implement the `Tool` trait (`name`, `actions`, `call(action, args) -> Result<Value, Diagnostic>`) and the tool registry per agent in `src/tools/mod.rs` (FR-003)
- [X] T014 Implement the tokio based tree walking evaluator: variables, expressions, `if`, `for`, `reply`, `fail`, target calls via the registry with permission check and timeout (default 30 s, `within` override) in `src/runtime/interpreter.rs` (FR-001, FR-016)
- [X] T015 Implement configuration loading (`metagente.toml` incl. `timeout_seconds`, `think_max_steps`, `bind`; environment overrides; unknown key warnings) in `src/runtime/config.rs` per contracts/configuration.md (FR-016, FR-017, FR-019, FR-020)
- [X] T016 Implement the `Llm` trait and a deterministic `FakeLlm` for tests in `src/llm/mod.rs` and `src/llm/fake.rs` (FR-017)
- [X] T017 Implement a panic hook and boundary error mapper so no Rust text or backtrace reaches the user (one plain sentence plus log file path) in `src/diagnostics/internal.rs` (FR-010, SC-007)
- [X] T018 Implement `src/main.rs` with clap subcommands wired to stubs: `run`, `serve`, `check`, `new`, `--version` (contracts/cli.md exit codes) (FR-001)

**Checkpoint**: Foundation ready, user stories can begin

---

## Phase 3: User Story 1 - Run a first agent that uses tools (Priority: P1) 🎯 MVP

**Goal**: An agent invokes a built in tool and an external MCP tool and executes the task (FR-001..005, SC-002..004)

**Independent Test**: quickstart scenarios 1, 2, 3, and 9

### Tests for User Story 1

- [X] T019 [P] [US1] Integration test: `examples/read_file.ag` reads a temp file via `tool file`, and `examples/clock.ag` uses `clock.now` and `clock.wait`, in `tests/integration/builtin_tools.rs` (FR-004, SC-003, SC-004)
- [X] T020 [P] [US1] Integration test: `examples/weather.ag` against an in-process fake MCP server (built with rmcp) including schema mismatch message and mid-call server loss in `tests/integration/mcp_client.rs` (FR-005, SC-003)
- [X] T021 [P] [US1] Integration test: a fake MCP tool that sleeps past the timeout yields a plain timeout message and the agent continues, in `tests/integration/timeout.rs` (FR-016)
- [X] T022 [P] [US1] Integration test: `http.get` and `http.post` against a local test server, and `state.set`/`state.get` keeping values per agent, in `tests/integration/http_state_tools.rs` (FR-004, SC-004)
- [X] T023 [P] [US1] Integration test: the real LLM provider sends the expected request to a local mock server and reads its key from the variable named in `api_key_env`, in `tests/integration/llm_provider.rs` (FR-017)
- [X] T024 [P] [US1] Integration test: `think` with `FakeLlm` calls a declared tool, and a model that keeps requesting tools stops at 10 steps with the plain message (FR-020, FR-017), in `tests/integration/think.rs`
- [X] T025 [P] [US1] Test that `examples/weather.ag` has fewer than 10 lines (SC-002) in `tests/unit/example_size.rs`

### Implementation for User Story 1

- [X] T026 [P] [US1] Implement the `file` tool (`read`, `write`) in `src/tools/file.rs` (FR-004)
- [X] T027 [P] [US1] Implement the `http` tool (`get`, `post`, result record with `status`, `text`, `json`) in `src/tools/http.rs` (FR-004)
- [X] T028 [P] [US1] Implement the `env` tool (`get`, only for names on the agent's declared allow list, refusing all others) in `src/tools/env.rs` (FR-004, FR-011)
- [X] T029 [P] [US1] Implement the per agent in-memory `state` tool (`set`, `get`) in `src/tools/state.rs` (FR-004)
- [X] T030 [P] [US1] Implement the `clock` tool (`now` returning text and unix, `wait seconds:` bounded by the call timeout) in `src/tools/clock.rs` (FR-004)
- [X] T031 [US1] Implement the MCP client: connect (stdio command or URL), list tools, cache input schemas, validate arguments before sending, call, map failures to plain messages, in `src/mcp/client.rs` (FR-005)
- [X] T032 [US1] Register MCP tools as `Tool` implementations for `tool NAME from mcp "..."` declarations in `src/mcp/tool.rs` (FR-005)
- [X] T033 [US1] Implement `think` in the interpreter using the agent goal, declared tool descriptions, and the configured `Llm`, with the "needs a language model" message when none is configured, a step limit (default 10, `think_max_steps`) that stops with a plain message, and one retry report for a malformed model tool request (research D15), in `src/runtime/think.rs` (FR-017, FR-020)
- [X] T034 [US1] Implement a configuration driven real LLM provider (anthropic and openai-compatible over reqwest, key read from `api_key_env`) in `src/llm/providers.rs` (FR-017)
- [X] T035 [US1] Implement `metagente run FILE.ag MESSAGE [key=value ...] [--agent NAME]` end to end in `src/main.rs` and `src/runtime/run.rs` (FR-001, FR-003)
- [X] T036 [P] [US1] Write beginner examples `examples/read_file.ag`, `examples/clock.ag` and `examples/weather.ag` (weather agent exactly as in contracts/language.md) (SC-002, SC-003)

**Checkpoint**: User Story 1 works alone; this is the shippable MVP

---

## Phase 4: User Story 2 - Understandable errors and a beginner friendly start (Priority: P1)

**Goal**: Every error is plain language with line and fix; a beginner reaches a working agent in under 15 minutes (FR-010, SC-001, SC-007)

**Independent Test**: quickstart scenarios 4 and 10

### Tests for User Story 2

- [X] T037 [P] [US2] Golden tests: each error kind (syntax, undeclared name, bad parameter, unknown message, missing goal, missing LLM) matches the Diagnostic structure and contains no Rust identifiers or "panicked" in `tests/unit/diagnostics_golden.rs` (FR-010, SC-007)
- [X] T038 [P] [US2] Test that `metagente run` on a file that triggers an internal panic prints one plain sentence and a log path in `tests/integration/internal_error.rs` (SC-007)

### Implementation for User Story 2

- [X] T039 [US2] Implement semantic checks (every handler in `accepts` except the optional `on start`, every call target declared, goal present, duplicate names) producing plain Diagnostics in `src/lang/check.rs` (FR-002, FR-010)
- [X] T040 [US2] Implement `metagente check FILE.ag` (parse plus semantic checks, runs nothing) in `src/main.rs` (FR-010)
- [X] T041 [US2] Implement `metagente new NAME` creating a starter agent and `metagente.toml` in `src/main.rs` and `src/runtime/scaffold.rs` (SC-001)
- [X] T042 [P] [US2] Add broken examples `examples/broken_undeclared_tool.ag` and `examples/broken_syntax.ag` for tests and docs (SC-007)
- [X] T043 [P] [US2] Write the getting started tutorial `docs/tutorial.md` (first agent in under 15 minutes, no Rust or protocol knowledge assumed) (SC-001)
- [X] T044 [P] [US2] Write the syntax reference `docs/syntax.md` mirroring contracts/language.md (SC-001)

**Checkpoint**: Stories 1 and 2 both work

---

## Phase 5: User Story 3 - Safe by default permissions (Priority: P2)

**Goal**: An agent can only use what it declared (FR-011)

**Independent Test**: quickstart scenario 5

### Tests for User Story 3

- [X] T045 [P] [US3] Integration test: reading a file without `tool file` is refused with the fix message and the file is untouched; `tool file "data/"` blocks paths outside `data/` and `..` escapes, in `tests/integration/permissions.rs` (FR-011)
- [X] T046 [P] [US3] Integration test: `tool env "HOME"` cannot read `PATH`; the variable named in `api_key_env` is unreadable even if declared; bare `tool env` fails with a fix hint (FR-011), in `tests/integration/env_scope.rs`

### Implementation for User Story 3

- [X] T047 [US3] Implement folder scoping for `tool file` (default project folder, optional narrower folder, symlink and `..` escape protection) in `src/tools/file.rs` and `src/runtime/permissions.rs` (FR-011)
- [X] T048 [US3] Hide the variable named by `api_key_env` from the `env` tool even when declared, in `src/tools/env.rs` and `src/runtime/permissions.rs` (FR-011)
- [X] T049 [US3] Restrict `link`/`remote` calls to declared targets and MCP calls to declared servers in `src/runtime/permissions.rs` (FR-011)
- [X] T050 [US3] Add the static permission pass to `metagente check` (catch undeclared tool use before running) in `src/lang/check.rs` (FR-011)
- [X] T051 [P] [US3] Add `examples/broken_permission.ag` demonstrating the refusal (FR-011)

**Checkpoint**: Stories 1..3 work

---

## Phase 6: User Story 4 - One agent invokes another, dynamic link (Priority: P2)

**Goal**: Agent A calls Agent B by name or path, validated and cycle safe (FR-008, FR-009, FR-012, FR-014, FR-015, SC-005)

**Independent Test**: quickstart scenario 6

### Tests for User Story 4

- [X] T052 [P] [US4] Integration test: planner calls weather via `link`, weather file is edited between runs and the change is picked up with no edit to planner, in `tests/integration/dynamic_link.rs` (FR-008, SC-005)
- [X] T053 [P] [US4] Integration test: call with an unknown message name is rejected before execution with the interface mismatch message in `tests/integration/link_interface.rs` (FR-009)
- [X] T054 [P] [US4] Integration test: A calls B calls A stops with the "A -> B -> A" message; missing target names where it looked, in `tests/integration/link_cycle.rs` (FR-012)

### Implementation for User Story 4

- [X] T055 [US4] Implement link resolution in the order: same file, relative `Name.ag`, project `agents/` folder, or explicit quoted path, in `src/link/resolve.rs` (FR-015)
- [X] T056 [US4] Implement lazy load with mtime based reload of target agents in `src/link/loader.rs` (FR-008, SC-005)
- [X] T057 [US4] Implement interface validation of a call against the target's `accepts` section in `src/link/interface.rs` (FR-009)
- [X] T058 [US4] Implement call chain tracking and cycle detection in `src/link/cycle.rs` and wire into `src/runtime/interpreter.rs` (FR-012)
- [X] T059 [US4] Register linked agents as call targets (`Name.message key: value`) in `src/link/tool.rs` (FR-008)
- [X] T060 [P] [US4] Write examples `examples/planner.ag`, `examples/cycle_a.ag`, `examples/cycle_b.ag` (FR-008, FR-012)

**Checkpoint**: Stories 1..4 work

---

## Phase 7: User Story 5 - Interoperate with the outside world, MCP server and A2A (Priority: P3)

**Goal**: Serve agents over MCP and A2A and call remote A2A agents (FR-006, FR-007, SC-006)

**Independent Test**: quickstart scenarios 7 and 8

### Tests for User Story 5

- [X] T061 [P] [US5] Contract test: Agent Card JSON at `/.well-known/agent-card.json` matches A2A 1.0.0 required fields, one skill per `accepts` entry, in `tests/contract/agent_card.rs` (FR-007, SC-006)
- [X] T062 [P] [US5] Contract test: `SendMessage` and `GetTask` request/response shapes and `TASK_STATE_*` values, media type `application/a2a+json`, in `tests/contract/a2a_wire.rs` (FR-007)
- [X] T063 [P] [US5] Integration test: unknown skill returns the spec conformant "not supported" error in `tests/integration/a2a_unsupported.rs` (FR-007)
- [X] T064 [P] [US5] Integration test: an rmcp client connects to `serve --mcp`, lists `Weather.ask`, calls it, in `tests/integration/mcp_server.rs` (FR-006)
- [X] T065 [P] [US5] Integration test: an agent calls a `remote` served by a second Metagente instance (A2A client, FR-007), in `tests/integration/a2a_client.rs`
- [X] T066 [P] [US5] Integration test: 50 simultaneous A2A requests to `serve` all complete correctly (FR-018), in `tests/integration/serve_concurrency.rs`
- [X] T067 [P] [US5] Integration test: `serve` accepts connections on 127.0.0.1 only, and with `--public` accepts others and prints the no-authentication warning (FR-019), in `tests/integration/serve_bind.rs`
- [X] T068 [US5] Interop test: an independent A2A client discovers the card and completes a task (SC-006) in `tests/integration/a2a_interop.rs`

### Implementation for User Story 5

- [X] T069 [P] [US5] Implement Agent Card generation from an agent definition in `src/a2a/card.rs` (FR-007)
- [X] T070 [US5] Implement the A2A server (axum): card endpoint, JSON-RPC `SendMessage` (blocking to terminal state) and `GetTask` (finished task records kept in memory for 10 minutes, then "task not found"), task lifecycle, one tokio task and one agent instance per request, in `src/a2a/server.rs` (FR-007, FR-018)
- [X] T071 [US5] Implement the A2A client for `remote Name at "url"` (fetch card on first use, send task, wait within timeout) in `src/a2a/client.rs` (FR-007)
- [X] T072 [US5] Register remote agents as call targets in `src/a2a/tool.rs` (FR-007)
- [X] T073 [US5] Implement the MCP server exposing each `accepts` entry as tool `Agent.message` (stdio and HTTP), one tokio task and one agent instance per request, in `src/mcp/server.rs` (FR-006, FR-018)
- [X] T074 [US5] Implement `metagente serve FILE.ag [--a2a PORT] [--mcp stdio|PORT]` in `src/main.rs` binding to 127.0.0.1 by default, with a `--public` flag that prints the no-authentication warning (FR-019), and graceful Ctrl-C shutdown
- [X] T075 [P] [US5] Write example `examples/remote_weather.ag` calling a remote A2A agent (FR-007)

**Checkpoint**: All five stories work

---

## Phase 8: Polish & Cross-Cutting

- [X] T076 [P] Verify single binary and no system libraries on Linux, macOS, Windows (FR-013, SC-004): add a CI step that runs `ldd`/`otool -L`/`dumpbin` checks in `.github/workflows/build.yml`
- [X] T077 [P] Run every `examples/*.ag` in CI through `check` (valid ones must pass, `broken_*` must fail with a Diagnostic) in `tests/integration/examples_check.rs` (SC-003, SC-007)
- [X] T078 [P] Measure the performance goals from plan.md (cold start under 100 ms, 500 line parse under 50 ms) with a benchmark in `tests/integration/perf.rs` (plan performance goals)
- [X] T079 Walk through every scenario in `specs/001-metagente-core/quickstart.md` and record results in `docs/validation.md` (SC-001 to SC-007)
- [ ] T080 Run a usability session with a first time user and record time to first working agent (SC-001) in `docs/validation.md`
- [X] T081 [P] Add a CI check `.github/workflows/grammar-check.yml` failing any change to `src/lang/` that does not also change `examples/` and `docs/syntax.md` (constitution Workflow) (constitution Workflow)
- [X] T082 Verify simple packaging: copy the binary and an agent folder with a linked agent to a clean machine or container and run it with no installation step, recording the result in `docs/validation.md` (constitution Simple packaging)
- [X] T083 [P] Update `docs/syntax.md` and `docs/tutorial.md` for anything that changed during implementation (FR-010, SC-001)

---

## Dependencies & Execution Order

- Setup (Phase 1) -> Foundational (Phase 2) -> user stories -> Polish.
- US1 needs only Foundational. It is the MVP.
- US2 needs Foundational; its semantic checks (T039) build on the parser and do not require US1, but its golden tests use US1's `run`.
- US3 builds on US1 tools (T047 edits `src/tools/file.rs`).
- US4 needs Foundational and the interpreter (T014); independent of US3.
- US5 needs US1 (MCP client/interpreter) and US4 only for shared call target registration; MCP server (T073) and A2A (T069..T072) are independent of each other.

Within a story: tests first (they must fail), then implementation. Tasks touching the same file are sequential (`src/tools/file.rs`: T026 then T047; `src/main.rs`: T018, T035, T040, T041, T074; `src/runtime/interpreter.rs`: T014 then T058).

## Parallel Opportunities

- Phase 2: T005, T006, T007 together, then T010 alongside T011..T013.
- US1: T019..T025 together; T026..T030 together; T036 anytime after T009.
- US4: T052..T054 together; T060 alongside.
- US5: T061..T067 together; T069 alongside T070; T073 alongside T070..T072 (different files).

## Implementation Strategy

1. **MVP first**: Phases 1, 2, and 3 (US1). Stop and run quickstart scenarios 1, 2, 3, 9, 10.
   The MVP release is US1 and US2; language version 1.0 is complete only after US5 ships (spec "Release staging").
2. Add US2 (beginner experience), validate scenarios 4 and 10 early with a real beginner.
3. Add US3, then US4, validating after each.
4. Add US5 last; it is the largest surface and depends on the protocol pins in contracts/protocols.md.
5. Polish, then the full quickstart run.

## Notes

- Grammar changes MUST update parser, one example, and docs in the same change (constitution Workflow).
- No task may introduce Rust types or stack traces into user output (Principle I).
- Commit after each task or logical group.
