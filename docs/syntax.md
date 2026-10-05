# Metagente syntax reference

A `.ag` file holds one or more agents. Indentation is **two spaces per level**. `#` starts a
comment. Text goes in double quotes, and `{name}` inside text is replaced by the value.

## Agent

```text
agent Name
  goal "What this agent is for"        # required
  <declarations>
  <handlers>
```

## Declarations

| Line | Meaning |
|------|---------|
| `goal "text"` | The purpose of the agent. Required. |
| `tool file` / `tool file "data/"` | Read and write files inside the project folder (where `metagente.toml` is, or where you started Metagente), or only inside the folder you name. Paths that lead outside, including through `..` or links, are refused. |
| `tool http` | Web requests. |
| `tool state` | The agent's memory. |
| `tool clock` | Current time and waiting. |
| `tool env "NAME" "NAME"` | Read only the named environment variables. The variable holding the model key can never be read. |
| `tool NAME from mcp "command or address"` | An external MCP server, called as `NAME.action`. |
| `link Name` / `link Name from "path.ag"` | Another Metagente agent (dynamic link). |
| `remote Name at "https://..."` | A remote A2A agent. |
| `accepts message param param` | A message the agent understands, and its values. A `# comment` on the same line becomes its description. |

An agent can use only what it declares.

## Handlers

```text
on message
  <statements>
on start          # optional, runs when the agent is started; not listed in `accepts`
  <statements>
```

Every `on message` needs a matching `accepts message`.

## Statements

| Statement | Meaning |
|-----------|---------|
| `name = expression` | Keep a value. |
| `target.action key: value key: value` | Call a tool, linked agent, or remote agent. The action may contain dashes or dots (`gh.create-issue`, `weather.Weather.ask`). Add `within 30 seconds` at the end to change the time limit. |
| `reply expression` | Answer and finish. |
| `think "prompt"` | Ask the language model, which may use the declared tools (at most 10 steps by default). |
| `if condition` / `otherwise` | Choose. |
| `for item in list` | Repeat. |
| `fail "message"` | Stop with a clear error. |

## Expressions

- Text `"..."`, numbers, `yes`, `no`, `nothing`, lists `[1, 2, 3]`.
- Names (letters, digits, `_` and `-`), and fields of records: `forecast.summary`.
- Comparisons: `is`, `is not`, `is more than`, `is less than`, `contains`.
- Combine with `and`, `or`, `not`.
- A call with no values is written `clock.now`.

## Built in tool actions

| Call | Result |
|------|--------|
| `file.read path: "a.txt"` | text |
| `file.write path: "a.txt" text: "..."` | nothing |
| `http.get url: "..."`, `http.post url: "..." body: ...` | record with `status`, `text`, `json` |
| `env.get name: "HOME"` | text, or nothing |
| `state.set key: "k" value: v`, `state.get key: "k"` | nothing, value |
| `clock.now` | record with `text` (ISO 8601) and `unix` |
| `clock.wait seconds: 5` | nothing |

## Errors

Every problem says where it is, what went wrong, and how to fix it. Problems that come from outside
Metagente (an MCP server, a remote agent, the operating system) are passed on in plain words and
name their source.

## Commands

| Command | Does |
|---------|------|
| `metagente run FILE.ag MESSAGE key=value ...` | Run an agent. Add `--agent Name` to pick one from a file with several. |
| `metagente check FILE.ag` | Look for problems without running. |
| `metagente new NAME` | Create a starter agent and `metagente.toml`. |
| `metagente serve FILE.ag --a2a PORT --mcp stdio` | Keep agents running for other programs. Local only unless `--public`. |
