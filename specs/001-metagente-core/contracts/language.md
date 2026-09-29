# Contract: The `.ag` language (MVP)

Line oriented, indentation (2 spaces) starts a block, `#` starts a comment. A file holds one or more
agents (FR-014).

## Reference agent (7 lines, SC-002)

The MCP server `weather-mcp` and the shape of its result are illustrative. Automated tests use an
in-repository fake MCP server with the same shape.

```text
agent Weather
  goal "Answer questions about the weather"
  tool weather from mcp "npx -y weather-mcp"
  accepts ask city
  on ask
    forecast = weather.forecast city: city
    reply "In {city} it will be {forecast.summary}"
```

## Declarations (inside `agent`)

| Line | Meaning | Grants |
|------|---------|--------|
| `goal "text"` | Purpose of the agent (required) | nothing |
| `tool file` / `tool file "data/"` | Built in file tool. Without a folder it may use the project folder (where `metagente.toml` is, or where Metagente was started); with one, only that folder, and relative paths start there | file access |
| `tool http`, `tool state`, `tool clock` | Other built in tools | network / memory / time |
| `tool env "NAME" "NAME"` | Reads the named environment variables only. `tool env` with no names is an error. The variable holding the model key is never readable | those variables |
| `tool NAME from mcp "command or url"` | External MCP server, called as `NAME.action` | that server |
| `link Name` / `link Name from "path.ag"` | Local Metagente agent (dynamic link) | calling it |
| `remote Name at "https://..."` | Remote A2A agent | calling it |
| `accepts message param param` | Public interface: a message and its parameters | none |

Optional description after `accepts`: `accepts ask city  # weather for a city`.

## Handlers

```text
on <message>            # must appear in `accepts`
  <statements>
on start                # optional, runs when the agent is started; not listed in `accepts`
```

## Statements

| Statement | Meaning |
|-----------|---------|
| `name = expression` | Keep a value |
| `target.action key: value key: value` | Call a tool, linked agent, or remote agent. Value is the result. Optional `within 60 seconds` at the end |
| `reply expression` | Send the answer back to the caller |
| `think "prompt"` | Ask the language model, using the agent goal and its declared tools. Value is text. Stops after at most 10 steps (configurable) |
| `if condition` / `otherwise` | Choice |
| `for item in list` | Repeat |
| `fail "message"` | Stop with a clear error |

## Expressions

Text `"..."` with `{name}` insertion, numbers, `yes`/`no`, lists `[1, 2]`, records via
`record.field`, comparisons (`is`, `is not`, `is more than`, `is less than`, `contains`), `and`, `or`,
`not`.

## Built in tool actions

| Call | Result |
|------|--------|
| `file.read path: "a.txt"` | text |
| `file.write path: "a.txt" text: "..."` | nothing |
| `http.get url: "..."` / `http.post url: "..." body: ...` | record with `status`, `text`, `json` |
| `env.get name: "HOME"` | text or nothing (refused for undeclared names) |
| `clock.now` | record with `text` (ISO 8601) and `unix` |
| `clock.wait seconds: 5` | nothing |
| `state.set key: "k" value: v` / `state.get key: "k"` | nothing / value |

## Errors (all natural language)

Every error is: where (file, line, column, source snippet), what went wrong in plain words, how to
fix it. Examples (exact wording finalized in implementation, structure is the contract):

```text
Problem on line 5 of weather.ag: this agent uses `file` but never declared it.
Fix: add the line `tool file` under `agent Weather`.
```
```text
Problem: Weather asked Planner, and Planner asked Weather again. (Weather -> Planner -> Weather)
Fix: make one of them answer without calling the other.
```

Errors that come from outside Metagente (an external MCP server, a remote A2A agent, the operating
system) are relayed in plain words, name their source, and suggest a fix when one exists.

## Not in the language (MVP)

Classes, modules, imports, user types, threads/async keywords, async/long running task handles,
provider selection.
