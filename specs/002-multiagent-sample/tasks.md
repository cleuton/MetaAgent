---

description: "Task list for the Multi-Agent Sample (City Briefing)"
---

# Tasks: Multi-Agent Sample (City Briefing)

**Input**: Design documents from `/specs/002-multiagent-sample/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. plan.md (research D8) defines one offline test module that runs the whole flow against a fake model and a fake MCP server, so the sample cannot rot unnoticed.

**Organization**: Tasks are grouped by user story. All three stories are P1. Phases follow build order,
not story number, because the stories depend on each other (see Dependencies). Paths are relative to
the repository root (`/home/cleuton/Documents/projetos/metagente`).

**Rule for every task**: nothing under `src/`, no grammar file, no `Cargo.toml`, and nothing under
`.specify/memory/` may change (SAM-010). If a task seems to need that, stop and report a defect against
spec 001.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1..US3, matching the user stories in spec.md

---

## Phase 1: Setup

- [X] T001 Create the folders `samples/` and `samples/city-briefing/`, and record a baseline of files that must stay unchanged with `sha256sum` over `src/`, `Cargo.toml`, `Cargo.lock` and `.specify/memory/constitution.md` into `specs/002-multiagent-sample/baseline.sha256` (SAM-001, SAM-010)
- [X] T002 [P] Write `samples/README.md`: one paragraph on what samples are and a list with `city-briefing/` (what it shows, how to start) (SAM-001)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Configuration and test fixtures that every story needs

**⚠️ CRITICAL**: No story work can begin until this phase is complete

- [X] T003 [P] Write `samples/city-briefing/metagente.toml` exactly as in contracts/configuration.md: provider `anthropic`, model `claude-sonnet-5-5`, `api_key_env = "ANTHROPIC_API_KEY"`, `timeout_seconds = 90`, `think_max_steps = 10`, `a2a_port = 8080`, `bind = "127.0.0.1"`, each setting with a comment saying what changing it does, and no key (SAM-004, SAM-005, SC-005)
- [X] T004 [P] Extend `tests/integration/common/web.rs` with `start_scripted(replies: Vec<serde_json::Value>)`: a mock Anthropic endpoint (`/v1/messages`) that answers with each reply in order and then repeats the last, and records every request body and header for assertions (test infrastructure for US1, US2, US3; research D8)
- [X] T005 [P] Extend `tests/integration/common/fake_mcp.rs` with a `fetch` tool that mimics the reference server: required `url`, optional `max_length` (number), returns text `Contents of <url>:\n<a few facts about the city in the url>`, and returns an error result for a url containing `Xyzzy` (test infrastructure for US3; research D2, D3)
- [X] T006 Create `tests/integration/sample_city_briefing.rs` with helpers and register `mod sample_city_briefing;` in `tests/integration/main.rs`: `sample_dir()` (path to `samples/city-briefing`), `staged(dir, mcp_command, researcher_url, model_base_url)` which copies the four sample files into a temp folder and replaces only the three strings (`uvx mcp-server-fetch`, `http://127.0.0.1:8080`, and adds `base_url` to the `[llm]` section), and `key_env` handling with a test-only variable name (test infrastructure for US1, US2, US3; research D8)

**Checkpoint**: Fixtures ready, stories can begin

---

## Phase 3: User Story 3 - Researcher gets facts through MCP (Priority: P1)

**Goal**: The Researcher uses an external MCP tool and the clock, and summarizes with the model (SAM-007)

**Independent Test**: quickstart step 3, and the tests below

### Tests for User Story 3

- [X] T007 [P] [US3] Test in `tests/integration/sample_city_briefing.rs`: with the fake MCP server and a scripted model (first a `fetch__fetch` tool call, then a text answer), `researcher.ag` `research city=Lisbon` calls fetch with a url that contains `Lisbon`, and replies with the model's facts followed by `(checked ` and a time (US3-1, SAM-007)
- [X] T008 [P] [US3] Test in `tests/integration/sample_city_briefing.rs`: with the MCP command replaced by a program that does not exist, the reply is the message that names the failed command and says to check that the program is installed (US3-2)
- [X] T009 [P] [US3] Test in `tests/integration/sample_city_briefing.rs`: `researcher.ag` declares exactly one MCP tool and the clock (parse it and compare), and a scripted model asking for `file__read` gets "no tool called `file`" handed back instead of file access (US3-3, SAM-007)
- [X] T010 [P] [US3] Test in `tests/integration/sample_city_briefing.rs`: `city=` (empty) stops with "I need a city" and the line number; and for a city containing `Xyzzy` the fetch error goes back to the model, which (scripted) answers that no facts were found, and that answer is the reply (US3 edge cases)

### Implementation for User Story 3

- [X] T011 [US3] Write `samples/city-briefing/researcher.ag` per contracts/agents.md: goal, `tool fetch from mcp "uvx mcp-server-fetch"`, `tool clock`, `accepts research city`, `if not city` then `fail "I need a city"`, read the clock, `think` with the prompt of research D2 and D3, reply with the facts and `(checked {now.text})`; fewer than 20 lines; only constructs from `docs/guide.md`; then confirm with `target/release/metagente check samples/city-briefing/researcher.ag` (SAM-007, SAM-008, SC-002)

**Checkpoint**: The Researcher works alone

---

## Phase 4: User Story 2 - Coordinator delegates over A2A (Priority: P1)

**Goal**: The Concierge sends an A2A task to the Researcher and writes the briefing from the reply (depends on US3)

**Independent Test**: quickstart steps 2 and 5

### Tests for User Story 2

- [X] T012 [P] [US2] Test in `tests/integration/sample_city_briefing.rs`: serve the staged `researcher.ag` with the in-process A2A server (`tests/integration/common/a2a.rs`), run the staged `concierge.ag` `brief city=Lisbon` with a scripted model (fetch call, facts, briefing); the result is the briefing text, and the last model request contains the facts the Researcher returned (US2-1)
- [X] T013 [P] [US2] Test in `tests/integration/sample_city_briefing.rs`: with nothing listening at the Researcher's address, the error names `Researcher` and the address tried and points to `metagente serve` (US2-2, SC-003)
- [X] T014 [P] [US2] Test in `tests/integration/sample_city_briefing.rs`: `concierge.ag` declares one remote named `Researcher` and no tool and no link (parse it and compare) (SAM-006)
- [X] T015 [P] [US2] Test in `tests/integration/sample_city_briefing.rs`: start `metagente serve researcher.ag` from the sample folder with a free port in the staged toml and confirm the port answers on `127.0.0.1` and refuses this computer's other network address, with no `--public` anywhere in the sample (US2-3, SAM-009)

### Implementation for User Story 2

- [X] T016 [US2] Write `samples/city-briefing/concierge.ag` per contracts/agents.md: goal, `remote Researcher at "http://127.0.0.1:8080"`, `accepts brief city`, `facts = Researcher.research city: city`, and `reply think` with the prompt of research D4; fewer than 10 lines; then confirm with `target/release/metagente check samples/city-briefing/concierge.ag` (SAM-006, SAM-008, SC-002)

**Checkpoint**: Both agents work together offline

---

## Phase 5: User Story 1 - Run the demo from the samples folder (Priority: P1)

**Goal**: A person follows the README and gets a briefing (depends on US2 and US3)

**Independent Test**: quickstart steps 1, 2, 4 and 6

### Tests for User Story 1

- [X] T017 [P] [US1] Test in `tests/integration/sample_city_briefing.rs`: in the full offline flow, every request the mock model receives says `"model": "claude-sonnet-5-5"` (both agents), and neither `.ag` file contains `anthropic`, `claude` or `model` (US1-2, SAM-004)
- [X] T018 [P] [US1] Test in `tests/integration/sample_city_briefing.rs`: with the key variable unset, running either agent gives a message that names `ANTHROPIC_API_KEY` as not set, without a key or Rust text (US1-3)
- [X] T019 [P] [US1] Test in `tests/integration/sample_city_briefing.rs`: changing only the `model` line of the staged toml makes both agents' requests carry the new model, while the two `.ag` files stay byte for byte the same (SC-005)
- [X] T020 [P] [US1] Test in `tests/integration/sample_city_briefing.rs`: the four files exist (`README.md`, `metagente.toml`, `concierge.ag`, `researcher.ag`), `samples/README.md` links `city-briefing/`, and the city-briefing README has headings for prerequisites, configuration, running, a sample question with expected output, an A2A check, and troubleshooting (SAM-001, SAM-002, SAM-003)
- [X] T021 [P] [US1] Test in `tests/integration/sample_city_briefing.rs`: `researcher.ag` has fewer than 20 lines, `concierge.ag` fewer than 10, and no file in `samples/` matches a key pattern (`sk-` followed by 10 or more characters) or the text of the current `ANTHROPIC_API_KEY` value (SAM-005, SC-002)

### Implementation for User Story 1

- [X] T022 [US1] Write `samples/city-briefing/README.md`: what the demo shows (with the flow picture from data-model.md), prerequisites (the `metagente` binary from `cargo build --release`, `uv`, an Anthropic API key), configuration (what each line of `metagente.toml` does; export the key in both terminals), running (terminal 1 `metagente serve researcher.ag`, terminal 2 `metagente run concierge.ag city=Lisbon`), a sample question with the shape of the expected answer, the external A2A check (the two `curl` commands of quickstart step 5), and a troubleshooting table built from the failure table in data-model.md with the exact messages; English, no Rust jargon (SAM-003, SC-001)
- [X] T023 [P] [US1] Update `README.md`: add a `Samples` row (`samples/`) to the project documents table and a line in the roadmap section saying the multi-agent sample exists (project rule: the README always has a current roadmap) (SAM-001)
- [X] T024 [P] [US1] Update `CHANGELOG.md`: under "Unreleased", add an "Added" line for the City Briefing sample; do not change any version number anywhere (SAM-010)

**Checkpoint**: All three stories work offline

---

## Phase 6: Polish & Cross-Cutting

- [ ] T025 Run the real demo once with the real key and the real fetch server (`uvx mcp-server-fetch`) following the README literally in a copy of `samples/city-briefing/`: the Lisbon briefing, the Researcher alone, and the failures of quickstart step 4 (key unset, Researcher stopped, empty city, `Xyzzyplugh`, port busy); record the real outputs, timings and date in `specs/002-multiagent-sample/validation.md` (US1, US2, US3, SC-001, SC-003)
- [X] T026 [P] Run the external A2A check of quickstart step 5 against the real Researcher with `curl`, and with the official Python SDK client (`tests/interop/a2a_sdk_client.py` adapted only in a scratch copy) to confirm the card and a `research` task; record in `specs/002-multiagent-sample/validation.md` (SC-004)
- [X] T027 Time the README steps end to end in a clean folder, following the text exactly; record the time in `specs/002-multiagent-sample/validation.md` and add "Tested on <date>, Linux, metagente v0.1.0" to `samples/city-briefing/README.md`; state that a first-time reader has not yet followed it (SC-001)
- [X] T028 [P] Verify SAM-010: run `sha256sum -c specs/002-multiagent-sample/baseline.sha256` and confirm nothing under `src/`, `Cargo.toml`, `Cargo.lock` or the constitution changed; record in `specs/002-multiagent-sample/validation.md` (SAM-010)
- [X] T029 Run the whole suite with `cargo test` and `cargo clippy --all-targets`, and finish `specs/002-multiagent-sample/validation.md` with a table of every quickstart scenario, how it was checked, and what remains open (all)

---

## Dependencies & Execution Order

- Setup (Phase 1) -> Foundational (Phase 2) -> US3 -> US2 -> US1 -> Polish.
- US3 (Researcher) needs only the fixtures. US2 (Concierge) needs the Researcher file and the A2A fixtures. US1 needs both agents and writes the README and the sample-wide tests.
- Within a story: tests first (they must fail), then the file that makes them pass.
- Tasks that edit the same file are sequential: `tests/integration/sample_city_briefing.rs` (T006, then T007 to T010, T012 to T015, T017 to T021), `README.md` (T023 only), `specs/002-multiagent-sample/validation.md` (T025 to T029).

## Parallel Opportunities

- Phase 2: T003, T004, T005 together, then T006.
- US3 tests T007 to T010 are written one after another in one file, but can be drafted in parallel and merged.
- US1: T023 and T024 together with T022.
- Polish: T026 and T028 together.

## Implementation Strategy

1. **First slice**: Phases 1 to 3. The Researcher runs alone against the fake servers (quickstart step 3).
2. Add the Concierge (Phase 4): the two-agent flow works offline.
3. Add the README and sample-wide checks (Phase 5).
4. Only then spend model credits: the real run and the recorded results (Phase 6).

## Notes

- The Researcher and Concierge prompts are the only "code" that needs care; keep them as short as the contracts show.
- If a construct is missing, do not add it: report it as a defect against spec 001 (SAM-010).
- Commit after each task or logical group.
