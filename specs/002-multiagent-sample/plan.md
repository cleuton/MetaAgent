# Implementation Plan: Multi-Agent Sample (City Briefing)

**Branch**: `002-multiagent-sample` | **Date**: 2026-09-29 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/002-multiagent-sample/spec.md`

## Summary

Add a `samples/` folder with one demo, `city-briefing`: a **Concierge** agent that delegates fact finding
to a **Researcher** agent over A2A, and a Researcher that reads a public page through an external MCP
server (`uvx mcp-server-fetch`) and stamps the answer with the built in clock. Both reason with Claude
Sonnet 5.5, chosen only in `metagente.toml`. The deliverables are two short `.ag` files, a
`metagente.toml`, two READMEs, and one offline test that runs the whole flow against a fake model
server and a fake MCP server. Nothing in the interpreter, the grammar, or the constitution changes
(SAM-010): every construct comes from the tutorial and `docs/guide.md`.

## Technical Context

**Language/Version**: Metagente `.ag` files (spec 001, interpreter v0.1.0), TOML, Markdown

**Primary Dependencies**: the `metagente` binary; `uv` (`uvx mcp-server-fetch`) for the MCP server;
an Anthropic API key in `ANTHROPIC_API_KEY`

**Storage**: N/A

**Testing**: one offline Rust integration test (`cargo test`) using the existing fake MCP server and
mock web server helpers, plus a recorded manual run with the real model and the real fetch server

**Target Platform**: Linux, macOS, Windows; two terminals on one machine

**Project Type**: sample project (documentation and example files)

**Performance Goals**: a beginner reaches a working briefing in under 10 minutes (SC-001); one
briefing takes a few seconds of model time plus the page fetch

**Constraints**: each agent under 20 lines, the concierge under 10 (SC-002); no API key in any file
(SAM-005); local only serving, no `--public` (SAM-009); constructs only from the tutorial and guide
(SAM-008)

**Scale/Scope**: 2 agents, 1 config file, 2 READMEs, 1 test

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Verified against `.specify/memory/constitution.md` (Metagente v1.0.0). Result: **PASS**, no violations.

- [x] **I. Simplicity**: the drafted agents were checked with `metagente check`: Researcher 11 lines,
      Concierge 7 lines, no syntax beyond the guide
- [x] **II. Agent-centric**: the whole demo is two `agent` blocks
- [x] **III. MCP/A2A**: MCP is used as a client (fetch server), A2A is used to serve the Researcher and
      to call it from the Concierge, both in one line each
- [x] **IV. Built-in tools**: the Researcher enables the clock with `tool clock`
- [x] **V. Dynamic link**: not used here on purpose. The two agents are separate processes, so the demo
      uses A2A, which is the constitution's protocol for remote agents
- [x] **VI. Rust**: nothing Rust shows up in the sample
- [x] **VII. Readability**: one way to call each thing (`Researcher.research city: city`)
- [x] **Constraints**: each agent declares only what it uses; serving stays on `127.0.0.1`; the key is
      only ever a variable name
- [x] **Workflow**: this feature is itself a complete beginner example; it needs no grammar change

**Re-check after Phase 1 design**: PASS. No new tool, syntax or setting was needed.

## Project Structure

### Documentation (this feature)

```text
specs/002-multiagent-sample/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── agents.md          # what each agent accepts, calls and declares
│   └── configuration.md   # the metagente.toml of the sample
└── tasks.md               # /speckit-tasks
```

### Source Code (repository root)

```text
samples/
├── README.md                    # lists the samples
└── city-briefing/
    ├── README.md                # prerequisites, configuration, running, A2A check, troubleshooting
    ├── metagente.toml           # Anthropic + claude-sonnet-5-5, key variable name, timeouts, port
    ├── concierge.ag             # coordinates: asks the Researcher, writes the briefing
    └── researcher.ag            # uses the fetch MCP tool and the clock, summarizes with the model

tests/integration/
└── sample_city_briefing.rs      # the whole flow offline: fake model, fake MCP server, in-process A2A

README.md                        # gains a "Samples" row and roadmap note (project rule)
```

**Structure Decision**: samples live in `samples/` at the repository root, one folder per sample, each
runnable on its own from inside its folder (that is where `metagente.toml` is found). The test lives
with the other integration tests and reads the real sample files, so the sample cannot rot unnoticed.

## Complexity Tracking

No constitution violations to justify.
