# Data Model: Metagente Core (MVP)

Runtime and language level entities; no database. Field names are conceptual.

## Agent (definition)
| Field | Description |
|-------|-------------|
| name | Unique within its file; used by `link` |
| goal | Text describing purpose; passed to the LLM by `think` and shown on the Agent Card |
| tools | Declared built in and MCP tools (each with a local name) |
| links | Declared local agents (dynamic link targets) |
| remotes | Declared remote A2A agents (name + address) |
| accepts | Public interface: named messages/tasks with parameters, each with a short description |
| handlers | One per accepted message, plus optional `on start` |
| source | File path + line span (for diagnostics and reload) |

Rules: names unique per file; every handler name appears in `accepts`, except the optional `on start`; every call target used in a
handler must be declared (`tool`, `link`, or `remote`); at least a goal is required (error suggests
adding `goal "..."`).

## Agent instance
Created per run or per incoming request. Holds: call context and the tools the agent declared. Discarded
at the end of the request. The agent's memory (`tool state`) is not part of the instance: it belongs to
the agent and is shared by all its requests while the process runs. MCP server connections are also
shared by the whole process and reconnect on the next call after a failure.

## Task record (A2A)
When an agent is reached over A2A, the finished task (id, state, reply) is kept in memory for 10
minutes after it reaches a terminal state, or until the `serve` process stops, so `GetTask` can
return it. After that, `GetTask` returns the spec's "task not found" error.

## Tool
| Field | Description |
|-------|-------------|
| local_name | Name used in calls (`weather`, `file`) |
| kind | builtin(file, http, env, state, clock) or mcp |
| actions | Built in: fixed list. MCP: discovered at connection, each with a parameter schema |
| permission | Scope granted by the declaration (for example folder for file) |

## Task (call in progress)
| Field | Description |
|-------|-------------|
| id | Correlation id (also the A2A task id when arriving over A2A) |
| target | Tool action, linked agent message, or remote agent message |
| arguments | Named values |
| deadline | Now + timeout (default 30 s) |
| state | pending -> running -> completed / failed / timed_out |
| result | Value or Diagnostic-style error |

State transitions are one way; a timeout cancels the underlying call.

## Call context
Ordered list of agent names currently executing in this chain (cycle detection), plus the task id
and remaining deadline.

## MCP connection
Command or URL, transport (stdio or HTTP), discovered tools with schemas, status (connected,
unavailable). Loss of connection during a call fails that call with a clear message; reconnect on
next call.

## Agent Card (A2A)
Generated from the Agent definition: name, goal as description, service URL, one skill per `accepts`
entry (name, description, parameters), supported input/output modes (text, JSON). Exact fields
follow the A2A spec version recorded in contracts/protocols.md.

## Dynamic link
| Field | Description |
|-------|-------------|
| reference | Name or quoted path as written |
| resolved_path | Result of D11 resolution order |
| loaded_mtime | Used to reload changed files |
| interface | The target's `accepts` section, used to validate calls |

## Diagnostic
`line`, `column`, `message` (plain language), `suggestion`, optional `related` (for cycles or
missing declarations). The only error shape shown to users.

## Value
Text, number, yes/no, list, record (named fields), nothing. No user declared types.
