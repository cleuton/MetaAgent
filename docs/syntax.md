# Metagente syntax reference

A `.ag` file holds one or more agents. Indentation is **two spaces per level**. `#` starts a
comment. Text goes in double quotes, and `{name}` inside text is replaced by the value.

Contents: [Agent](#agent), [Declarations](#declarations), [Handlers](#handlers),
[Statements](#statements), [Expressions](#expressions), [Built in tool actions](#built-in-tool-actions),
[Formal grammar (EBNF)](#formal-grammar-ebnf), [Alphabetical reference](#alphabetical-reference),
[Errors](#errors), [Commands](#commands).

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

## Formal grammar (EBNF)

The grammar below describes the language as documented. Notation: `=` defines a rule, `|` is a
choice, `[ x ]` is optional, `{ x }` is zero or more, `( x )` groups, `"x"` is a literal word.
`NEWLINE`, `INDENT` and `DEDENT` come from the two space indentation rule: `INDENT` is one more
level (two spaces), `DEDENT` is one level less. Comments (`#` to the end of the line) and blank
lines are ignored everywhere and are not shown in the rules.

```ebnf
(* Structure *)
file          = { agent } ;
agent         = "agent" NAME NEWLINE
                INDENT goal { declaration } { handler } DEDENT ;

(* Declarations *)
goal          = "goal" TEXT NEWLINE ;
declaration   = tool_decl | link_decl | remote_decl | accepts_decl ;

tool_decl     = "tool" ( builtin_tool | mcp_tool ) NEWLINE ;
builtin_tool  = "file" [ TEXT ]
              | "http"
              | "state"
              | "clock"
              | "env" TEXT { TEXT } ;
mcp_tool      = NAME "from" "mcp" TEXT [ timeout ] ;

link_decl     = "link" NAME [ "from" TEXT ] NEWLINE ;
remote_decl   = "remote" NAME "at" TEXT NEWLINE ;
accepts_decl  = "accepts" NAME { NAME } NEWLINE ;

(* Handlers *)
handler       = "on" NAME NEWLINE block ;
block         = INDENT statement { statement } DEDENT ;

(* Statements *)
statement     = assignment | call_stmt | reply_stmt | think_stmt
              | if_stmt | for_stmt | fail_stmt ;

assignment    = NAME "=" expression NEWLINE ;
call_stmt     = call NEWLINE ;
reply_stmt    = "reply" expression NEWLINE ;
think_stmt    = think NEWLINE ;
if_stmt       = "if" condition NEWLINE block
                [ "otherwise" NEWLINE block ] ;
for_stmt      = "for" NAME "in" expression NEWLINE block ;
fail_stmt     = "fail" TEXT NEWLINE ;

(* Calls and think *)
call          = NAME "." ACTION { argument } [ timeout ] ;
argument      = NAME ":" primary ;
timeout       = "within" NUMBER "seconds" ;
think         = "think" expression ;

(* Expressions *)
expression    = condition ;
condition     = and_test { "or" and_test } ;
and_test      = not_test { "and" not_test } ;
not_test      = "not" not_test | comparison ;
comparison    = primary [ compare_op primary ] ;
compare_op    = "is" [ "not" ]
              | "is" "more" "than"
              | "is" "less" "than"
              | "contains" ;

primary       = TEXT
              | NUMBER
              | "yes" | "no" | "nothing"
              | list
              | call
              | think
              | path ;
list          = "[" [ expression { "," expression } ] "]" ;
path          = NAME { "." NAME } ;

(* Words *)
NAME          = ( letter | "_" ) { letter | digit | "_" | "-" } ;
ACTION        = ( letter | "_" ) { letter | digit | "_" | "-" | "." } ;
NUMBER        = digit { digit } [ "." digit { digit } ] ;
TEXT          = '"' { text_char | interpolation | escape } '"' ;
interpolation = "{" path "}" ;
escape        = "\n" ;
```

Reading notes:

- A `path` with no arguments, such as `clock.now`, is also a valid `call`. The interpreter tells
  them apart by looking at the declarations: if the first name is a declared tool, linked agent or
  remote agent, it is a call; otherwise it is a record field.
- The words `agent`, `and`, `at`, `contains`, `fail`, `for`, `from`, `goal`, `if`, `in`, `is`,
  `link`, `mcp`, `no`, `not`, `nothing`, `on`, `or`, `otherwise`, `remote`, `reply`, `think`,
  `tool`, `within`, `yes` have a meaning in the language and should not be used as names.
- `tool NAME from mcp` and the built in tools share the `tool` word. `file`, `http`, `state`,
  `clock` and `env` are the built in names.
- Only `\n` is documented as an escape inside text. Escapes for a double quote or for a literal
  `{` are not described yet.
- This grammar is derived from this reference, the tutorial and the programming guide. Where the
  documents are silent (for example, whether parentheses can group a condition), the grammar does
  not allow it.

## Alphabetical reference

Every word and call of the language, in alphabetical order. Each entry has the form, what it does
and a short example.

### `accepts`

Declares a message the agent understands, and the values that come with it. A `# comment` on the
same line becomes its description.

```text
accepts ask city        # the city to look up
```

### `agent`

Starts an agent. A file can hold several. Needs a `goal`.

```text
agent Weather
  goal "Answer questions about the weather"
```

### `and`

Is true when both sides are true.

```text
if n is more than 10 and not word is "skip"
  reply "big"
```

### `at`

Used by `remote` to give the address of a remote A2A agent.

```text
remote Bob at "http://127.0.0.1:8080"
```

### `clock.now`

Current time. Needs `tool clock`. Gives a record with `text` (ISO 8601) and `unix`.

```text
now = clock.now
reply "{now.text} ({now.unix})"
```

### `clock.wait`

Waits for some seconds. Needs `tool clock`. Gives nothing.

```text
clock.wait seconds: 5
```

### `contains`

Is true when a text has a piece of text inside, or a list has an item.

```text
if word contains "ab"
  reply "found"
```

### `env.get`

Reads an environment variable. Needs `tool env` naming that variable. Gives text, or `nothing`.

```text
home = env.get name: "HOME"
```

### `fail`

Stops the agent with your sentence as the error, shown with the line.

```text
if city is nothing
  fail "I need a city"
```

### `file.read`

Reads a text file. Needs `tool file`. Gives text.

```text
text = file.read path: "notes.txt"
```

### `file.write`

Writes a text file. Needs `tool file`. Gives nothing.

```text
file.write path: "report.txt" text: report
```

### `for`

Repeats the indented lines for each item of a list.

```text
for city in ["Lisbon", "Porto", "Faro"]
  answer = Weather.ask city: city
```

### `from`

Used by `link` to give a file path, and by `tool` to say where an MCP tool comes from (`from mcp`).

```text
link Weather from "agents/weather.ag"
tool weather from mcp "npx -y weather-mcp"
```

### `goal`

What the agent is for. Required, one per agent.

```text
goal "Say hello to someone"
```

### `http.get`

Makes a web request. Needs `tool http`. Gives a record with `status`, `text` and `json`.

```text
page = http.get url: "https://example.org"
reply "status {page.status}"
```

### `http.post`

Sends text to a web address. Needs `tool http`. Gives a record with `status`, `text` and `json`.

```text
http.post url: "https://example.org/hook" body: report
```

### `if` and `otherwise`

Chooses between lines. `otherwise` is optional. `nothing`, empty text, an empty list, `no` and `0`
count as false.

```text
if answer is "sunny"
  reply "Take sunglasses"
otherwise
  reply "Take an umbrella"
```

### `is`, `is not`, `is more than`, `is less than`

Compares two values. `"5"` and `5` count as equal.

```text
if n is more than 10
  reply "big"
if word is not "skip"
  reply "go on"
```

### `link`

Declares another Metagente agent to call (dynamic link). Found when the line runs, and checked
against its `accepts`.

```text
link Weather
link Weather from "agents/weather.ag"
```

### `name = expression`

Keeps a value under a name. The name may be used later, including inside text.

```text
city = "Lisbon"
report = "{report}\n- {city}: {answer}"
```

### `not`

Reverses a test.

```text
if not city
  fail "I need a city"
```

### `on`

Starts a handler. `on message` runs when that message arrives and needs a matching `accepts`.
`on start` runs when the agent starts and is not listed in `accepts`.

```text
on ask
  reply "Hello"
on start
  state.set key: "count" value: "0"
```

### `or`

Is true when at least one side is true.

```text
if word contains "ab" or n is less than 0
  reply "match"
```

### `remote`

Declares a remote A2A agent, called like a tool.

```text
remote Bob at "http://127.0.0.1:8080"
...
forecast = Bob.ask city: city
```

### `reply`

Sends the answer back to whoever asked (terminal, another agent, an A2A client) and finishes.

```text
reply "Hello, {name}!"
```

### `state.get`

Reads a value from the agent's memory. Needs `tool state`. Gives the value, or `nothing`.

```text
last = state.get key: "last-sunny"
```

### `state.set`

Saves a value in the agent's memory. Needs `tool state`. Gives nothing.

```text
state.set key: "last-sunny" value: city
```

### `target.action`

Calls a tool, a linked agent or a remote agent, with `key: value` pairs. The action may contain
dashes or dots. Add `within N seconds` to change the time limit.

```text
forecast = weather.forecast city: city
issue = github.create-issue title: "Hi"
answer = weather.Weather.ask city: "Lisbon" within 60 seconds
```

### `think`

Asks the language model. It may use the declared tools (at most 10 steps by default). Its answer is
a value like any other.

```text
summary = think "Summarize this in one sentence:\n{report}"
reply think "{question}"
```

### `tool`

Enables a tool. An agent can use only the tools it declares.

```text
tool file
tool file "data/"
tool http
tool state
tool clock
tool env "HOME" "LANG"
tool weather from mcp "npx -y weather-mcp"
tool weather from mcp "npx -y weather-mcp" within 60 seconds
```

### `within`

Changes the time limit of a call or of an MCP tool line.

```text
answer = Weather.ask city: city within 30 seconds
```

### Values: `yes`, `no`, `nothing`, numbers, text, lists

```text
ok = yes
none = nothing
n = 3
name = "World"
cities = ["Lisbon", "Porto"]
```

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
| `metagente --version` | Show the version. |

Examples:

```text
metagente run hello.ag greet name=World
metagente run team.ag ask city=Lisbon --agent Weather
metagente check hello.ag
metagente new hello
metagente serve weather.ag --a2a 8080
metagente serve weather.ag --mcp stdio
metagente serve weather.ag --mcp 9000
```