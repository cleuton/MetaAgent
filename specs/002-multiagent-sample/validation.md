# Validation record: City Briefing sample

Checked on 2026-09-29 on Linux (x86_64) with the release build of `metagente` v0.1.0 and `uv` 0.11.29.

## The one thing not done

**The demo has not been run against the live Claude API.** The environment this was built in had an
empty `ANTHROPIC_API_KEY` (Anthropic answered `401: x-api-key header is required`), and no other
credential was used. Everything else below ran for real. To finish task T025, run the README steps
with your key and paste the briefing into the "Tested on" note of `samples/city-briefing/README.md`.

To exercise everything except Claude, the same sample files ran as two real processes with the real
`uvx mcp-server-fetch` server and a real Wikipedia page, with a tiny local program standing in for the
model (it asks for the fetch tool, then echoes what the tool returned). That program is not part of the
project.

## Results by scenario (quickstart.md)

| # | Scenario | How it was checked | Result |
|---|----------|--------------------|--------|
| 1 | Offline flow | `cargo test --test integration sample_city_briefing` (13 tests) | Passed |
| 2 | Real demo, US1/US2/US3 | Two real processes: `metagente serve researcher.ag`, then `metagente run concierge.ag city=Lisbon`; real fetch server, real Wikipedia page, stand-in model. Took 4.7 s the first time, including starting the fetch server | Passed for everything except Claude. **Not run with Claude** |
| 3 | Researcher alone | `metagente run researcher.ag city=Lisbon`: the article text came back, then `(checked 2026-09-29T18:53:08+00:00)` | Passed (stand-in model) |
| 4 | Errors are readable | See the list below | Passed |
| 5 | External A2A client (SC-004) | `curl` for the card and for `SendMessage`; the official Python SDK (`a2a-sdk`) read the card and got a `TASK_STATE_COMPLETED` task from the running Researcher | Passed |
| 6 | Change the model (SC-005) | Automated: `changing_only_the_model_line_changes_the_model_of_both_agents`; both agents sent the new model and the `.ag` files were identical | Passed offline |
| 7 | Local only | `curl` to this computer's other address (192.168.7.26) was refused; automated `serving_the_sample_researcher_is_local_only_and_needs_no_public_option` | Passed |

Failure cases, run against the real processes (all in plain language, with the line and a fix):

- Key variable not set: `the language model could not answer: the variable ANTHROPIC_API_KEY is not set`.
- Researcher stopped: `I could not reach http://127.0.0.1:18080/.well-known/agent-card.json (remote agent Researcher): the connection failed`, fix: `check the address, and that the other agent is being served (metagente serve)`.
- `uv` missing (simulated with a program that does not exist): `I could not start the tool server ...`, fix: `check that the program is installed and the command is spelled right`.
- Empty city: `I need a city` on line 8 of `researcher.ag`.
- Page that does not exist (`Xyzzyplugh`): the real server's `status code 404` was handed to the model, which answered that no facts were found.
- Port already in use: `I could not listen for A2A on 127.0.0.1:...`, fix: `choose another port, or stop the program that is using it`.

## A design mistake found only by running against the real fetch server

The plan chose the Wikipedia summary API (`/api/rest_v1/page/summary/{city}`). The real fetch server
respects `robots.txt`, and Wikipedia forbids automated fetching of that path, so every city would have
come back as "no facts found". The offline tests could not show this. The Researcher now reads the
article page (`/wiki/{city}`), which the server may fetch. Recorded in `research.md` D3.

## Timing (SC-001)

With `metagente` already built and `uv` installed, following the README took under a minute of
typing and waiting: the Researcher started in about 1.5 s, and a briefing came back in about 5 s (with
the stand-in model; Claude adds its own answer time). Building `metagente` (`cargo build --release`)
took 2 to 3 minutes and installing `uv` about a minute. That leaves the total under the 10 minutes of
SC-001 for someone who has the prerequisites, **but no first-time reader has followed the README yet**, so
SC-001 is not proven.

## No change to the interpreter (SAM-010)

`sha256sum -c specs/002-multiagent-sample/baseline.sha256` reports all 49 files unchanged: everything
under `src/`, `Cargo.toml`, `Cargo.lock` and `.specify/memory/constitution.md`.

## Whole project

`cargo test`: 118 tests pass (12 contract, 82 integration, 24 unit). `cargo clippy --all-targets`: no warnings.

## A defect to report against spec 001 (not fixed here, by SAM-010)

A key variable that exists but is **empty** (`export ANTHROPIC_API_KEY=`) is treated as set. The request
then goes out with an empty header and the provider answers `401: x-api-key header is required`, and the fix
line says to "check that the key variable is set". It should say the variable is empty. This is a small
change in `src/llm/providers.rs`, for a later change to the interpreter.
