# Changelog

All notable changes to Metagente are recorded here.
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## Unreleased

A guide and a sample were added: `docs/mcp-tools-guide.md` and `multi-agent-samples/mcp-tools/` show how to write an MCP
tool in Python, TypeScript, Go and Java (the same example in each), a tool that reads a database, and `smoke.py`, a
program that tests a tool without an agent. Nothing in the program changed.

## v0.1.5 - 2026-11-10

Several language models for `think`. Everything that worked in 0.1.4 works the same way. The only new text in the
language is `using NAME` after a `think` prompt, which was a syntax error before; the only new text in `metagente.toml`
is a sub-table of `[llm]`.

### Added
- **`think "..." using fast`**: one optional word at the end of the `think` line says which model answers. Without it
  the model of `[llm]` answers, exactly as before. `using` stays an ordinary name everywhere else, so a file that uses it
  as a variable, a message value or a parameter keeps working.
- **`[llm.NAME]`**: named models, sub-tables of the `[llm]` section (no new section). A named model is `anthropic` or
  `openai` at its official address (it never reads an address), with `provider`, `model` or `model_env` (the NAME of a
  variable that holds the model name, read at the first use) and `api_key_env` (the NAME of the variable that holds the
  key, read on every call). The usual model of Anthropic and the usual key variables apply as for `[llm]`.
- **Plain mistakes**: an unknown setting, `model` with `model_env`, a key written in the file, `base_url`, a provider that
  is not `anthropic` or `openai`, a missing provider or model name, a variable name with the wrong shape, a nickname with
  a wrong shape, a setting of `[llm]` or a word of the language as a nickname, and a `using` with an unknown nickname all
  stop `check`, `run` and `serve` with the line and the fix. A message prints nothing from the file except a safe name
  (at most 24 letters, digits, `_` or `-`), so a key pasted where a name belongs is never printed.
- **Warnings**: a sub-table without `[version]` warns that older programs do not know it (use `[version] minimum =
  "0.1.5"`); `check` warns about an unset key or model variable of a model that some `using` points to.
- **`metagente serve`** reads the models at start. A later edit prints one line asking for a restart; an agent edited to
  use a model that was not read at start keeps the last good version.
- **`tool env`** never reads the `api_key_env` of any named model, even when the agent declares it.
- **Failures of a named model** are plain sentences with the nickname and the provider (a status number and the fix for
  it, or the kind of connection problem), and never repeat text from the service, the library or a proxy.
- **OpenAI models that reason by default**: some of them refuse a request that has tools with a 400 that says `reasoning_effort`
  must be `none`. The first request is exactly what 0.1.4 sent; only after that refusal the same request is sent once more
  with `reasoning_effort: "none"`, and the next calls of that model send it at once. Any other answer, and every other
  provider, is left alone. This applies to `[llm]` too, because it is the same code, and it was decided by the maintainer
  after the first live check failed with such a model.
- `metagente new` writes a commented named-model example. Sample [multi-agent-samples/two-models](multi-agent-samples/two-models/README.md).

### Not changed
- `[llm]`, its four settings, its providers (including `openai-compatible` and `base_url`), its messages and the requests it
  sends. `METAGENTE_LLM_PROVIDER` and `METAGENTE_LLM_MODEL` still change `[llm]` only.
- The only differences in output are the version number wherever it appears (the `User-Agent` header, the Agent Card, the
  `[version]` line `metagente new` writes, the version gate message).

### Notes
- A project that uses `[llm.NAME]` should set `[version] minimum = "0.1.5"`: Metagente 0.1.4 ignores the sub-tables with a
  note and fails on `using`.
- The eight live checks against the real Anthropic and OpenAI services are made by hand (`specs/007-multiple-llm-models/validation.md`).

## v0.1.4 - 2026-11-08

Minimum version and authentication of served messages. Everything that worked in 0.1.3 works the same way. No syntax
changed: the `.ag` language is the same.

### Added
- **`[version] minimum`** in `metagente.toml`: three whole numbers, a floor. A Metagente older than that stops `run`,
  `check` and `serve` with a plain message, before anything else in the file is read. `[auth]` without `[version]`
  prints a warning, because Metagente 0.1.3 and older ignore `[auth]` and would serve every message unprotected.
  `metagente new` writes `[version]` and a commented `[auth]` example.
- **`[auth]`**: `validator`, `protect`, `forward`, `cache_seconds` (0 to 300) and `timeout_seconds` (1 to 60). A message in
  `protect` needs `Authorization: Bearer <token>` over A2A and over MCP on http. Metagente hands the token to your
  validator program on standard input, in a clean environment (only `PATH`), and obeys the exit code (0 = accepted). It
  never opens a token. HTTP 401 for no, malformed or refused tokens; HTTP 503 when the validator is broken. Nothing of the
  agent runs before an accepted token. The Agent Card declares the Bearer requirement (A2A 1.0).
- **Passing the token on**: the names in `forward` (remote agents and http(s) MCP tools) receive the caller's token;
  nothing else does. Tokens never mix between requests, never follow a redirect, and never go to stdio tools, the `http`
  tool or the model.
- **Side doors closed**: `link` cannot reach a protected message from a request without an accepted token; `GetTask` reads
  a protected task only with the token that started it.
- **Fail closed**: a typo in `[auth]`, a name that matches nothing, a protected message on plain http on another address,
  or over `--mcp stdio`, stop `serve`. `[version]` and `[auth]` are read at start; a later edit prints a line asking for a restart.
- Sample [multi-agent-samples/auth-chain](multi-agent-samples/auth-chain/README.md), and the
  [authentication guide](docs/authentication-guide.md).

### Known limitations
- **Not tested on Microsoft Windows.** The validator runs in a clean environment (only `PATH` and `SystemRoot`).
  Whether `python`, `node` and other programs start with only those has not been tested on Windows. Linux and macOS are
  the tested systems for this feature until a Windows run is recorded.
- A revoked token stays good until its entry in the cache ends (`cache_seconds`).

### Changed
- The version is 0.1.4. The test that pinned the version number of 0.1.3 now pins 0.1.4.

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

