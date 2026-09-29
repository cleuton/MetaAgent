# Contract: MCP and A2A surfaces

Both follow the public specifications with no proprietary extensions (constitution).

**Pinned versions** (checked 2026-09-29):
- **MCP**: specification `2026-07-28`, the latest stable release. The `rmcp` 3.x SDK implements it
  and stays compatible with `2025-11-25` and earlier, so older MCP servers keep working.
- **A2A**: specification `1.0.0` (latest published release, repository tag `v1.0.1`), using the
  JSON-RPC binding with media type `application/a2a+json`.

## MCP client (consuming external tools)

- Declared by `tool NAME from mcp "command or url"`. A command runs as a stdio server; a URL uses
  HTTP transport.
- On first use: initialize, list tools, cache each tool's input schema.
- On each call: arguments are validated against the schema before sending; a mismatch produces a
  plain message listing the expected parameters.
- Server unavailable mid-call: the call fails with a plain message; the agent continues.

## MCP server (exposing agents)

- `metagente serve --mcp` exposes each agent as tools: one MCP tool per `accepts` entry named
  `Agent.message`, description from the comment, input schema from the parameter names (text typed
  unless a value constraint is given).
- Tool calls run the matching handler; the reply becomes the tool result.
- Network transports listen on 127.0.0.1 unless `--public` is given (FR-019).

## A2A server (receiving tasks)

- `metagente serve --a2a PORT` (127.0.0.1 unless `--public`) serves, per agent, an Agent Card at the spec's well-known path and a
  JSON-RPC endpoint. Card path: `/.well-known/agent-card.json`. Methods supported in the MVP: `SendMessage`, `GetTask`. Finished task records are kept for 10 minutes (see data-model.md "Task record").
- Every incoming request runs as its own task with its own agent instance (FR-018).
- Card fields come from the agent (see data-model.md); one skill per `accepts` entry.
- A message naming an unknown skill returns a spec conformant "not supported" error (spec User
  Story 5, scenario 3).
- Task lifecycle reported with the spec states `TASK_STATE_SUBMITTED`, `TASK_STATE_WORKING`, `TASK_STATE_COMPLETED`, `TASK_STATE_FAILED`. `SendMessage` blocks until a terminal state (the spec default), which matches the synchronous MVP.
- Out of MVP: `ListTasks`, `CancelTask`, streaming, push notifications, authenticated extended cards.

## A2A client (sending tasks)

- `remote Name at "url"` fetches the Agent Card on first use, sends the message as a task, waits for
  the final result within the timeout, returns the reply as the call value.

## Dynamic link (local, Metagente only)

- Not a network protocol. In process call after interface validation against the target's `accepts`.
  Chain tracked for cycle detection (see research D11).
