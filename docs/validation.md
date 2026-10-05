# Validation record

What was checked against `specs/001-metagente-core/quickstart.md`, how, and what is still open.
Run on Linux (x86_64) on 2026-09-29 with the debug and release builds. The automated tests are run
with `cargo test`; the independent A2A client test needs `pip install a2a-sdk` and
`METAGENTE_INTEROP_PYTHON=<python with a2a-sdk>` (CI does this).

## Quickstart scenarios

| # | Scenario | How it was checked | Result |
|---|----------|--------------------|--------|
| 1 | Built in tool (US1-1, SC-004) | Ran `metagente run read_file.ag summarize` with a `notes.txt`; test `builtin_tools::read_file_example_reads_a_temp_file` | Passed: the file's text was printed, no I/O code in the agent |
| 2 | External MCP tool (US1-2, SC-003) | Test `mcp_client::weather_example_calls_an_external_mcp_tool` runs the real `examples/weather.ag` text against an MCP server built with the same SDK; a Metagente agent also calls another served by `metagente serve --mcp` (`mcp_server::a_metagente_agent_can_use_another_one_served_over_mcp_http`) | Passed. The `npx -y weather-mcp` package in the example is illustrative and was not run. A wrong parameter name lists what the tool expects (`mcp_client::wrong_parameter_name_lists_what_the_tool_expects`) |
| 3 | Minimal agent size (SC-002) | `wc -l examples/weather.ag` = 7; test `example_size` | Passed (fewer than 10 lines) |
| 4 | Errors are readable (US2, SC-007) | `metagente check broken_undeclared_tool.ag`; golden tests `diagnostics_golden::*` | Passed: line, plain message, fix, no Rust text |
| 5 | Permissions (US3) | `broken_permission.ag` refused, `tool env "HOME"` cannot read `PATH`, bare `tool env` fails with a hint; tests `permissions::*`, `env_scope::*` (including a symlink that leads out of the folder) | Passed |
| 6 | Dynamic link and cycles (US4, SC-005) | Ran the planner twice with `weather.ag` edited in between (output changed, planner untouched); `cycle_a.ag` stops with the circle shown; a wrong message name is rejected before it runs; tests `dynamic_link::*`, `link_interface::*`, `link_cycle::*` | Passed |
| 7 | A2A interoperability (US5, SC-006) | Card and tasks fetched with curl; test `a2a_interop` runs the official Python A2A SDK (`a2a-sdk` 1.2.0) against the server: it discovers the card, sends a message, gets the task back, and gets a "Not supported" error for an unknown skill | Passed |
| 8 | MCP server (US5-2) | Test `mcp_server::an_mcp_client_lists_and_calls_agents_served_over_stdio` (an rmcp client spawns `metagente serve --mcp stdio`) | Passed: `Weather.ask` is listed and callable |
| 9 | Timeout (edge case) | Test `timeout::a_slow_tool_times_out_with_a_plain_message_and_the_agent_keeps_working` (5 s tool, 1 s limit) | Passed |
| 10 | Clock and think limit | Tests `builtin_tools::clock_example_reports_the_time_and_waits`, `think::*` (10 step default) | Passed |
| 11 | Local only serving and concurrency | Tests `serve_bind::*` (this machine has a second network address, so the outside checks really ran) and `serve_concurrency` (50 requests that each wait one second finished in under 10 seconds; one after another they would need 50) | Passed |
| 12 | Packaging | `scripts/packaging-check.sh` copies the binary and two linked agents into a bare `debian:stable-slim` container with no CA certificates and runs them; `https` also worked there | Passed after a fix (see below) |
| 13 | Beginner time (SC-001) | Needs a first time user | **Not done** |

## A bug found by the packaging check

In the bare container the first run failed with "Something went wrong inside Metagente": the web
client could not load the computer's CA certificates and gave up, even for agents that never use the
web. The client is now built so it falls back to the certificates bundled in the binary
(`src/runtime/web.rs`), and never stops startup. This is the kind of failure "nothing to install"
has to survive.

## Other checks

- Performance goals (test `perf`): a trivial agent runs from a cold start in under 100 ms; a 500 line
  file parses in under 50 ms.
- Single binary: the Linux release build needs only `libc`, `libm` and `libgcc_s` (no OpenSSL). The
  CI workflow checks this for Linux (static musl), macOS and Windows; only the Linux check has been
  run so far.
- Every file in `examples/` is checked by `examples_check`: valid ones pass `check`, `broken_*` ones
  fail with a plain message.

## Still open

1. **SC-001 (T080).** Nobody who has never programmed has followed `docs/tutorial.md` yet. To run the
   session: give a first time user only the tutorial and a computer with the `metagente` binary; ask
   them to think aloud; start a timer when they open the tutorial; stop it when their own agent
   (not the sample) runs and answers; note every place they hesitated or read a message twice;
   success is under 15 minutes. Record the time and the notes here.
2. macOS and Windows CI steps (build, test, library check) have not been run on those systems.
3. The example MCP package `weather-mcp` does not exist; use any MCP server you have.
