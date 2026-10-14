# Changelog

All notable changes to Metagente are recorded here.
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## v0.1.3 - 2026-10-13

Secure communication by declaration (HTTPS on both ends). Everything that worked in 0.1.2 works the same way.

### Added
- **Secure by declaration.** The `[secure]` section of `metagente.toml` lists the agents, the tools and the model
  that MUST use https (`agents`, `tools`, `llm`). Calling one of them over `http://` is refused before any connection
  is opened, with the line and the fix. An agent in the list is secure in every direction: it is served over https
  only and makes only https calls (remote agents, tools reached over http, the `http` tool, the model), also through
  `link`. Nothing changed in the language: a program asks for https by writing `https://` in the address.
- **`metagente serve` answers over https.** `--tls-cert` and `--tls-key`, or `tls_cert` and `tls_key` in `[serve]`, for
  the A2A and the MCP endpoint, with a PEM certificate and key (a self-signed pair works). With a certificate every
  port answers https only; a secure agent never falls back to http and refuses to start without a certificate. The
  Agent Card advertises `https://` addresses. A change of certificate while serving prints one line asking for a restart.
- **`[network]`**: `allow_self_signed`, `self_signed_hosts`, `llm_allow_self_signed` (the model has its own switch) and
  `ca_file`. Certificates are checked by default, with the certificates of the computer and a bundled set as a
  fallback. Every skipped check prints a warning on every `run`, `serve` and `check`.
- **`[network.proxy]`**: `url`, `username_env`, `password_env` (a login is only ever the name of an environment
  variable), `no_proxy` and `pass_to_tools`; `HTTPS_PROXY`, `HTTP_PROXY` and `NO_PROXY` are used when there is no `url`.
- Plain-language errors for certificates (not trusted, expired, wrong name), downgrades from https to http, proxies,
  and all of the above. `metagente check` validates `[secure]` and `[network]`; `metagente new` writes commented examples.
- **Python interoperability**, tested against the official `a2a-sdk` in both directions: over plain http, over https
  with a self-signed certificate (served by Metagente itself, with nothing in front of it) and through an authenticated
  proxy. `tests/interop/requirements.txt` and `examples/python_interop/`.
- **Sample `multi-agent-samples/https-chain`**: a Planner calls a Researcher over https, which calls a Facts tool over https.
  No model key is needed, and the README shows every message of the "break it on purpose" section.
- Documentation: the README (secure connections, every key, the messages, how to run the Python tests), the tutorial
  (serving over https, a "Secure connections" step) and `docs/syntax.md` (a "Secure connections" section, the errors, the commands).

### Changed
- Source code changed for this release carries a comment starting with `0.1.3:`.
- `metagente --version` reports `0.1.3`.

## v0.1.2 - 2026-10-06

Multi-line text and external parameters. Everything that worked in 0.1.1 works the same way.

### Added
- Text with many lines between `"""` and `"""`, kept exactly as typed (spaces and line breaks included).
- Parameters: the `[parameters]` section of `metagente.toml`, read by agents as `@parameters.name` anywhere text
  in quotes is accepted. `{name}` in a parameter used as text is filled in.
- Agents loaded with `link` read the parameters of the agent that loaded them, not those of their own folder.
- Two plain-language errors: a text with many lines that is not closed, and a parameter that does not exist.
- `metagente check` verifies parameters; `metagente serve` applies a changed parameter on the next request;
  `metagente new` writes a commented `[parameters]` example.
- Examples `examples/multiline.ag` and `examples/parameters.ag` (with `examples/metagente.toml`).
- `docs/syntax.md`: the sections Text and Parameters, the grammar, the reference and the errors are updated.

### Changed
- Source code changed for this release carries a comment starting with `0.1.2:`.

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

