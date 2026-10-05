# Changelog

All notable changes to Metagente are recorded here.
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## v0.1.1 - 2026-10-02

The interpreter now exists. Everything in the specification is implemented and tested.

### Added
- The `metagente` command: `run`, `check`, `new`, `serve`.
- The language: parser, natural language errors with the line and a fix, static checks before running.
- Built in tools: file (folder scoped, symlink safe), http, env (named variables only), state, clock.
- MCP client (stdio and HTTP) with argument checking, and `think` with Anthropic and OpenAI-compatible
  providers chosen only in `metagente.toml`.
- Dynamic link: same file, next to the caller, `agents/` folder or a quoted path; interface checks;
  cycle detection; changes to a linked agent apply without restarting the caller.
- A2A 1.0 server and client (Agent Card, `SendMessage`, `GetTask`) and an MCP server; local only unless
  `--public`. Verified against the official Python A2A SDK.
- Tutorial, syntax reference, examples, and CI for Linux, macOS and Windows.
- Sample `samples/city-briefing`: a Concierge and a Researcher agent that work together over A2A and MCP
  with Claude Sonnet 5.5, with its own `metagente.toml` and README, and an offline test.
- Distribution as a zip for Linux AMD64, and instructions in the README to build the Windows and macOS zips.

## v0.1.0 - 2026-09-29

First tagged version. Specification only; the interpreter is not implemented yet.

### Added
- Project constitution v1.0.0 with seven principles, constraints, workflow and governance
  (`.specify/memory/constitution.md`).
- Core MVP specification `001-metagente-core`: 5 user stories, 20 functional requirements and
  7 success criteria (`specs/001-metagente-core/spec.md`).
- Implementation plan with 16 recorded technical decisions, pinned to MCP spec `2026-07-28` and
  A2A spec `1.0.0`.
- Data model and contracts for the `.ag` language, the command line, configuration, and the MCP and
  A2A surfaces.
- Quickstart validation guide with 13 scenarios.
- Task list of 83 tasks, each traced to a requirement.
- Constitution check added to the plan template (`.specify/templates/plan-template.md`).

