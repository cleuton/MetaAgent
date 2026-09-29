# Quickstart: Validating the MVP

Run these once implementation exists. Each scenario maps to a spec acceptance scenario.
Prerequisites: the `metagente` binary on PATH; Node available only if the MCP example uses an
`npx` server (an in-repo fake MCP server is used in CI instead).

## 1. Built in tool (US1-1, SC-004)

```bash
echo "hello" > notes.txt
metagente run examples/read_file.ag summarize
```
Expected: the agent prints the file's content (or its summary); no I/O code in the `.ag` file.

## 2. External MCP tool (US1-2, SC-003)

```bash
metagente run examples/weather.ag ask city=Lisbon
```
Expected: the agent discovers the tool, calls it, replies with a forecast sentence. Passing a wrong
parameter name prints a plain message listing expected parameters.

## 3. Minimal agent size (SC-002)

`wc -l examples/weather.ag` is less than 10.

## 4. Errors are readable (US2, SC-007)

```bash
metagente check examples/broken_undeclared_tool.ag
```
Expected: message naming the line, the missing `tool file` declaration, and the fix; no Rust text.

## 5. Permissions (US3)

Run an agent that reads a file without `tool file`. Expected: refused with the same style message;
file untouched.

An agent with `tool env "HOME"` reading `PATH` is refused; reading the variable named in `api_key_env`
is refused even if declared; a bare `tool env` line fails with a fix hint.

## 6. Dynamic link and cycles (US4, SC-005)

```bash
metagente run examples/planner.ag plan city=Lisbon      # calls Weather
# edit examples/weather.ag, run again: change is picked up, planner.ag unchanged
metagente run examples/cycle_a.ag go                    # A -> B -> A
```
Expected: planner gets Weather's result; after the edit the new behavior appears; the cycle run
stops with "A -> B -> A" message. A call with a wrong message name is rejected before execution.

## 7. A2A interoperability (US5, SC-006)

```bash
metagente serve examples/weather.ag --a2a 8080
curl http://localhost:8080/.well-known/agent-card.json
```
Then send a task with an independent A2A client. Expected: card lists the `ask` skill; task
completes with the forecast. An unknown skill returns a "not supported" error.

## 8. MCP server (US5-2)

```bash
metagente serve examples/weather.ag --mcp stdio
```
Connect any MCP client: `Weather.ask` is listed and callable.

## 9. Timeout (edge case)

Call a fake MCP tool that sleeps 60 s. Expected: after 30 s a plain timeout message; the agent
continues to the next line.

## 10. Clock and think limit (FR-004, FR-020)

- `examples/clock.ag` prints the current time and waits 1 second.
- An agent whose fake model keeps requesting tools stops after 10 steps with the plain message.

## 11. Local only serving and concurrency (FR-018, FR-019)

- `metagente serve examples/weather.ag --a2a 8080` accepts connections on 127.0.0.1 and refuses them
  from another machine; with `--public` it accepts them and prints the no-authentication warning.
- Fire 50 simultaneous A2A requests: all complete correctly.

## 12. Packaging (constitution "Simple packaging")

Copy the binary and an agent folder (with a linked agent) to a clean machine and run it there with no
installation step.

## 13. Beginner time (SC-001)

A usability session with a first time user following docs/tutorial; record time to first working
agent (target under 15 minutes).
