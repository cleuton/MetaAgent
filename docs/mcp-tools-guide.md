# Writing tools for Metagente in Python, TypeScript, Go and Java

A Metagente agent can call a **tool**: a small program that does one job and is written in any language. This guide
shows, step by step, how to write that program in **Python, TypeScript, Go and Java**, how to plug it into an agent with
one line, how to run it, and what happens when something goes wrong. The same example runs in all four languages, and
the two agents that use it are exactly the same file in all four.

Everything here was run for real, with the real SDKs, and the commands and outputs are copied from those runs. The
files are also in the sample folder [`multi-agent-samples/mcp-tools`](../multi-agent-samples/mcp-tools/README.md).

## 1. What a tool is, and why you write one

An agent decides what to do and talks to people. A tool does a job for the agent: it reads a file, asks a web address,
adds numbers. Metagente already has a few tools that need no installation (`tool file`, `tool http`, and others). For
anything else, an agent uses a tool that is **a separate program** that Metagente starts and talks to. The language the
two use to talk is called MCP (Model Context Protocol). You do not need to know it: an official SDK for your language
does the talking, and you write only the tool.

**A tool can do anything the language of the tool can do that Metagente cannot.** Metagente has no arithmetic, no
vectors beyond a list of numbers, no database, and no file format beyond text. A tool has whatever its language has.
A tool can:

* read and write **databases** (the last part of this guide shows one),
* do **arithmetic and vectors** (the example of this guide),
* process text in ways Metagente does not,
* read **spreadsheets**, **PDFs** and **images**,
* run **machine learning libraries**,
* talk to **hardware**,
* call **code your company already has**.

In the agent, using a tool is one line, and the agent does not say which language the tool is written in:

```text
tool calc from mcp @parameters.calc_command
```

Which program is `calc_command` is written in `metagente.toml`. Change that one line and the same agent uses a tool
written in another language. That is the point of this guide.

## 2. What to install, and the working folder

You need Metagente and **one** of the four languages (or all four, to try them all). Python 3 is needed in every
chapter, because the test program `smoke.py` is written in Python.

| What | Lowest version | Command that shows it | Where to get it |
|------|----------------|-----------------------|-----------------|
| Metagente | 0.1.5 | `metagente --version` | https://github.com/cleuton/MetaAgent/releases/latest |
| Python, for `smoke.py` (every chapter) | 3.9 | `python3 --version` (`py --version` on Windows) | https://www.python.org/downloads/ |
| Python, for the `mcp` package (Python chapter and the database) | 3.10 | `python3 --version` | https://www.python.org/downloads/ |
| Node.js with npm (TypeScript chapter) | 20 | `node --version` and `npm --version` | https://nodejs.org/en/download |
| Go (Go chapter) | 1.25 | `go version` | https://go.dev/dl/ |
| JDK (Java chapter) | 17 | `java -version` | https://jdk.java.net/ |
| Maven (Java chapter) | 3.8.7 was used | `mvn -version` | https://maven.apache.org/download.cgi |

The lowest versions are the ones the SDKs themselves state. The versions pinned in this guide are Python `mcp` 2.3.0,
npm `@modelcontextprotocol/server` 2.4.0, Go `go-sdk` v1.8.0 and Java `io.modelcontextprotocol.sdk:mcp` 2.1.0. The
outputs below were made with Python 3.14.2, Node.js 25.2.1, Go 1.26.6, OpenJDK 21.0.2 and Maven 3.8.7, on Linux. Where a
command differs on Windows (PowerShell) it is given next to the Linux and macOS one; those Windows forms were not run
for this guide.

Every library is installed inside your project folder (Python's `.venv`, TypeScript's `node_modules`), except Go's and
Java's: Go keeps its downloaded modules in a cache of your user (by default `~/go/pkg/mod`) and Maven in a repository of
your user (by default `~/.m2/repository`). Both are shared by all your projects. Nothing is installed for the whole
computer except the language itself.

### The working folder

Make one folder for everything you do here. Choose a path with no space in it, to keep things simple (Metagente splits the
command that starts a tool on white space and has no shell; the problems table at the end shows the one case where a
space matters). Each language gets a sub folder, and the test program goes in the folder above them, so that
`../smoke.py` always finds it.

**Linux and macOS**

```text
mkdir mcp-tools
cd mcp-tools
```

**Windows (PowerShell)**

```text
mkdir mcp-tools
cd mcp-tools
```

Save this file in it as `smoke.py`. It starts a tool server that you name, does what Metagente does (handshake, list of
tools, calls) and prints one line for each. It uses only what comes with Python.

**File: `smoke.py`**

```python
#!/usr/bin/env python3
"""Test a calc tool server without an agent.

Usage:  python3 ../smoke.py -- <the command that starts the server>

It starts the server, does the handshake that Metagente does, lists the tools,
makes the fixed calls and prints one line for each. A correct server gives the
same lines in every language. Uses only the Python standard library.
"""
import json
import queue
import subprocess
import sys
import threading

PROTOCOL_VERSION = "2026-07-28"  # the version Metagente 0.1.5 sends
WAIT_SECONDS = 60

EMPTY = "values must not be empty"
SIZE = "vectors must have the same size"

# What every correct server announces: name -> (description, {property: (type text, required)})
TOOLS = {
    "dot": ("Dot product of two lists of numbers of the same size.",
            {"a": "array of number", "b": "array of number"}),
    "mean": ("Average of a list of numbers.", {"values": "array of number"}),
    "scale": ("Every number of the list multiplied by the factor.",
              {"factor": "number", "values": "array of number"}),
}

# The fixed calls: (tool, arguments, expected result or the sentence of the error)
CALLS = [
    ("mean", {"values": [2, 4, 4, 4, 5, 5, 7, 9]}, {"result": 5}),
    ("mean", {"values": [1, 2, 3, 4]}, {"result": 2.5}),
    ("dot", {"a": [1, 2, 3], "b": [4, 5, 6]}, {"result": 32}),
    ("scale", {"values": [1, 2, 3], "factor": 2}, {"result": [2, 4, 6]}),
    ("mean", {"values": []}, EMPTY),
    ("dot", {"a": [1, 2], "b": [1]}, SIZE),
    ("dot", {"a": [], "b": []}, EMPTY),
    ("scale", {"values": [], "factor": 2}, EMPTY),
]


def plain(value):
    """A number with no fraction becomes a whole number (5.0 -> 5)."""
    if isinstance(value, bool):
        return value
    if isinstance(value, float) and value == int(value):
        return int(value)
    if isinstance(value, list):
        return [plain(v) for v in value]
    if isinstance(value, dict):
        return {k: plain(v) for k, v in value.items()}
    return value


def show(value):
    return json.dumps(plain(value), sort_keys=True, separators=(", ", ": "))


def compact(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def type_text(prop):
    kind = prop.get("type")
    if kind == "array":
        items = prop.get("items") or {}
        if items.get("type") == "number":
            return "array of number"
        return "array of " + compact(items)
    if kind == "number":
        return "number"
    return compact(kind)


class Server:
    def __init__(self, command):
        self.process = subprocess.Popen(
            command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, universal_newlines=True, bufsize=1)
        RUNNING.append(self.process)
        self.lines = queue.Queue()
        self.next_id = 0
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        for line in self.process.stdout:
            self.lines.put(line)
        self.lines.put(None)

    def send(self, message):
        self.process.stdin.write(json.dumps(message) + "\n")
        self.process.stdin.flush()

    def ask(self, method, params):
        self.next_id += 1
        wanted = self.next_id
        self.send({"jsonrpc": "2.0", "id": wanted, "method": method, "params": params})
        while True:
            try:
                line = self.lines.get(timeout=WAIT_SECONDS)
            except queue.Empty:
                stop("I gave up: the server did not answer `%s` in %d seconds." % (method, WAIT_SECONDS))
            if line is None:
                stop("The server ended before it answered `%s`." % method)
            try:
                message = json.loads(line)
            except ValueError:
                stop("I will not test this server: it wrote text to its standard output that is not "
                     "a protocol message:\n  %s\nA tool must write nothing but protocol there. "
                     "Print messages for yourself to the standard error instead." % line.strip())
            if isinstance(message, dict) and message.get("id") == wanted:
                return message


RUNNING = []


def stop(message):
    print(message)
    for process in RUNNING:
        process.kill()
    sys.exit(2)


def main():
    if "--" not in sys.argv:
        stop("Usage: python3 smoke.py -- <the command that starts the server>")
    command = sys.argv[sys.argv.index("--") + 1:]
    if not command:
        stop("Usage: python3 smoke.py -- <the command that starts the server>")
    try:
        server = Server(command)
    except OSError as error:
        stop("I could not start the server `%s`: %s" % (" ".join(command), error))

    passed = 0
    total = 0

    def report(ok, line):
        nonlocal passed, total
        total += 1
        passed += 1 if ok else 0
        print(line)

    server.ask("initialize", {"protocolVersion": PROTOCOL_VERSION, "capabilities": {},
                              "clientInfo": {"name": "smoke.py", "version": "1.0.0"}})
    server.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
    listed = server.ask("tools/list", {}).get("result", {}).get("tools", [])
    by_name = {tool.get("name"): tool for tool in listed}

    for name in sorted(set(TOOLS) | set(by_name)):
        tool = by_name.get(name)
        if tool is None:
            report(False, "FAIL tool %s: the server does not have it" % name)
            continue
        schema = tool.get("inputSchema") or {}
        required = schema.get("required") or []
        properties = schema.get("properties") or {}
        parts = ["%s: %s (%s)" % (key, type_text(properties[key]), "required" if key in required else "optional")
                 for key in sorted(properties)]
        line = "tool %s: %s | %s" % (name, tool.get("description", ""), ", ".join(parts))
        if name not in TOOLS:
            report(False, "FAIL " + line + " (a tool that the contract does not have)")
            continue
        description, wanted = TOOLS[name]
        expected = "tool %s: %s | %s" % (
            name, description, ", ".join("%s: %s (required)" % (k, wanted[k]) for k in sorted(wanted)))
        report(line == expected, line if line == expected else "FAIL %s\n     expected %s" % (line, expected))

    for name, arguments, expected in CALLS:
        call = "%s %s" % (name, show(arguments))
        answer = server.ask("tools/call", {"name": name, "arguments": arguments})
        result = answer.get("result")
        if result is None:
            report(False, "FAIL %s -> the server answered with a protocol error, not a result: %s"
                   % (call, compact(answer.get("error"))))
            continue
        texts = [c.get("text", "") for c in result.get("content", []) if c.get("type") == "text"]
        if isinstance(expected, str):
            if result.get("isError") is True and any(expected in t for t in texts):
                report(True, '%s -> error containing "%s"' % (call, expected))
            else:
                report(False, 'FAIL %s -> expected an error containing "%s", got %s'
                       % (call, expected, compact(result)))
            continue
        structured = result.get("structuredContent")
        if result.get("isError") or structured is None or len(texts) != 1:
            report(False, "FAIL %s -> expected structuredContent and one text block, got %s" % (call, compact(result)))
            continue
        try:
            same = plain(json.loads(texts[0])) == plain(structured)
        except ValueError:
            same = False
        if not same:
            report(False, "FAIL %s -> structuredContent %s and the text block %s are not the same JSON"
                   % (call, show(structured), json.dumps(texts[0])))
        elif plain(structured) != plain(expected):
            report(False, "FAIL %s -> got %s, expected %s" % (call, show(structured), show(expected)))
        else:
            report(True, "%s -> %s" % (call, show(structured)))

    print("%d of %d checks passed" % (passed, total))
    try:
        server.process.stdin.close()
        server.process.terminate()
    except OSError:
        pass
    sys.exit(0 if passed == total else 1)


main()
```

You will use it in every chapter, **before** you run any agent. That is a habit worth keeping: a mistake in a tool is
easy to see when only the tool is running, and hard to see through an agent. Make each project in its own sub folder of
`mcp-tools`: `python`, `typescript`, `go` and `java` (the chapters tell you when), and later `database`.

## 3. How a tool server talks to Metagente

You will not write any of this, but it helps to see it once. Metagente starts your program and talks to it through its
**standard input and output**, one JSON message per line:

1. Metagente **starts the program**. Nothing else is needed: no port, no address.
2. Metagente sends an `initialize` message and the program answers. This is the **handshake**: each side says who it is.
3. Metagente sends `notifications/initialized`. It is a notification, so it has no answer.
4. Metagente sends `tools/list` and the program answers with its tools: their names, descriptions and what values each
   one takes (the **input schema**).
5. Each time an agent uses the tool, Metagente sends `tools/call` with the name and the values, and the program answers
   with the result.

Metagente does all of this itself. The SDK of your language answers all of it for you. **You only write the tools.**

Here is what really went through the pipes when the agent of this guide ran with the Python tool. It was recorded by a
small program placed between Metagente and the tool that copies every line. The lines are exactly as they went; only the
answer to `tools/list` is shortened, after its first tool. The first line of each pair says who sent it.

**Output: the messages, in the order they happened**

```text
METAGENTE->SERVER {"jsonrpc":"2.0","id":0,"method":"initialize","params":{"capabilities":{},"clientInfo":{"name":"rmcp","version":"3.5.0"},"protocolVersion":"2026-07-28"}}
SERVER->METAGENTE {"jsonrpc":"2.0","id":0,"result":{"capabilities":{"prompts":{"listChanged":false},"resources":{"listChanged":false,"subscribe":false},"tools":{"listChanged":false}},"protocolVersion":"2025-11-25","serverInfo":{"name":"calc","version":"1.0.0"}}}
METAGENTE->SERVER {"jsonrpc":"2.0","method":"notifications/initialized"}
METAGENTE->SERVER {"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{"progressToken":0}}}
SERVER->METAGENTE {"jsonrpc":"2.0","id":1,"result":{"tools":[{"description":"Average of a list of numbers.","inputSchema":{"properties":{"values":{"items":{"type":"number"},"title":"Values","type":"array"}},"required":["values"],"type":"object","title":"meanArguments"},"name":"mean","outputSchema":{"properties":{"result":{"title":"Result","type":"number"}},"required":["result"],"type":"object","title":"Number"}},"... the other tools, with the same shape ..."]}}
METAGENTE->SERVER {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"_meta":{"progressToken":1},"arguments":{"values":[2,4,4,4,5,5,7,9]},"name":"mean"}}
SERVER->METAGENTE {"jsonrpc":"2.0","id":2,"result":{"content":[{"text":"{\n  \"result\": 5.0\n}","type":"text"}],"isError":false,"structuredContent":{"result":5.0}}}
```

In plain words:

* **Line 1, `initialize`.** Metagente says it is a client (`rmcp`, the Rust library it uses), and which version of the
  protocol it speaks (`2026-07-28`).
* **Line 2.** The program answers with its name and version (`calc`, `1.0.0`), what it can do (it has tools), and the
  version of the protocol both will use (`2025-11-25`: the SDK picked one it knows).
* **Line 3, `notifications/initialized`.** Metagente says the handshake is done. No answer is expected.
* **Line 4, `tools/list`.** Metagente asks what tools there are.
* **Line 5.** The answer. For each tool: its `name`, its `description`, and its `inputSchema`, which says that `mean`
  takes one value called `values`, that it is required, and that it is a list (`array`) of numbers. Metagente checks the
  values an agent gives against this schema before it sends them. (This SDK also sends an `outputSchema`, which describes
  the result.)
* **Line 6, `tools/call`.** The agent called `calc.mean` with the list of eight numbers.
* **Line 7.** The answer: a `content` with one text block that holds the result as JSON, `"isError": false`, and the
  same result again as `structuredContent`. Metagente reads `structuredContent`. (The number is `5.0` here; Metagente
  prints `5`.)

## 4. The contract of the example

The example is simple math with vectors. A tool server called `calc` (version `1.0.0`) has three tools. It is the same in
all four languages: the same names, the same values, the same results.

| Tool | Values it takes | What it does |
|------|-----------------|--------------|
| `mean` | `values`: a list of numbers | Average of a list of numbers. |
| `dot` | `a` and `b`: lists of numbers | Dot product of two lists of numbers of the same size. |
| `scale` | `values`: a list of numbers, `factor`: a number | Every number of the list multiplied by the factor. |

Every tool answers with an object that has one field called `result`: `{"result": 5}` or `{"result": [2, 4, 6]}`. It is
sent in two ways at once (as `structuredContent` and as the text of one text block), as the MCP specification
recommends. A wrong input is an **error of the tool**, not a crash and not an invented answer. There are two fixed
sentences:

* an empty list, in any of the three tools: `values must not be empty`,
* `dot` with two lists of different sizes: `vectors must have the same size` (this one is checked first).

| Call | Result |
|------|--------|
| `mean` of `[2, 4, 4, 4, 5, 5, 7, 9]` | `{"result": 5}` |
| `mean` of `[1, 2, 3, 4]` | `{"result": 2.5}` |
| `dot` of `[1, 2, 3]` and `[4, 5, 6]` | `{"result": 32}` |
| `scale` of `[1, 2, 3]` by `2` | `{"result": [2, 4, 6]}` |
| `mean` of `[]` | error: `values must not be empty` |
| `dot` of `[1, 2]` and `[1]` | error: `vectors must have the same size` |
| `dot` of `[]` and `[]` | error: `values must not be empty` |
| `scale` of `[]` by `2` | error: `values must not be empty` |

`smoke.py` makes exactly these calls. When a server is correct it prints exactly these twelve lines (the last one counts
the checks), and it prints the same twelve lines for the four languages:

**Output: what smoke.py prints for a correct server**

```text
tool dot: Dot product of two lists of numbers of the same size. | a: array of number (required), b: array of number (required)
tool mean: Average of a list of numbers. | values: array of number (required)
tool scale: Every number of the list multiplied by the factor. | factor: number (required), values: array of number (required)
mean {"values": [2, 4, 4, 4, 5, 5, 7, 9]} -> {"result": 5}
mean {"values": [1, 2, 3, 4]} -> {"result": 2.5}
dot {"a": [1, 2, 3], "b": [4, 5, 6]} -> {"result": 32}
scale {"factor": 2, "values": [1, 2, 3]} -> {"result": [2, 4, 6]}
mean {"values": []} -> error containing "values must not be empty"
dot {"a": [1, 2], "b": [1]} -> error containing "vectors must have the same size"
dot {"a": [], "b": []} -> error containing "values must not be empty"
scale {"factor": 2, "values": []} -> error containing "values must not be empty"
11 of 11 checks passed
```

## 5. The agents

Two agents use the tool. **`Vectors`** asks the tool to do the math. It has three messages, `average`, `similarity` and
`double`, and each one takes lists. Save it as `vectors.ag` in each project folder:

**File: `python/vectors.ag`**

```text
# Does simple vector math with a tool. Which program is the tool, and in which language, is in metagente.toml.
agent Vectors
  goal "Do simple vector math with a tool written in another language"
  tool calc from mcp @parameters.calc_command
  accepts average values        # a list of numbers
  accepts similarity a b        # two lists of numbers of the same size
  accepts double values         # a list of numbers
  on average
    r = calc.mean values: values
    reply r.result
  on similarity
    r = calc.dot a: a b: b
    reply r.result
  on double
    r = calc.scale values: values factor: 2
    reply r.result
```

Read it aloud. `tool calc from mcp @parameters.calc_command` says: there is a tool called `calc`, it is an MCP server,
and the command that starts it is the parameter `calc_command`, which is in `metagente.toml`. The agent never says which
language the tool is written in. `calc.mean values: values` calls the tool `mean` of `calc`, giving it the value
`values`. The tool answers with an object, and `r.result` is its field `result`.

**`Caller`** sends the vectors. Save it as `caller.ag` next to the first:

**File: `python/caller.ag`**

```text
# Sends vectors to the Vectors agent. `metagente run` can only send text, so the lists are written here.
agent Caller
  goal "Send vectors to the Vectors agent and show what comes back"
  link Vectors
  accepts go
  accepts oops
  on go
    m = Vectors.average values: [2, 4, 4, 4, 5, 5, 7, 9]
    d = Vectors.similarity a: [1, 2, 3] b: [4, 5, 6]
    s = Vectors.double values: [1, 2, 3]
    reply "mean {m}, dot {d}, doubled {s}"
  on oops
    reply Vectors.similarity a: [1, 2] b: [1]
```

Why a second agent? `metagente run` can only give an agent **text**. You cannot type a list on the command line: if you
write `values=1,2,3`, the agent receives the text `"1,2,3"`, not a list of three numbers, and the tool refuses it (the
section "Break it on purpose" shows it). A list reaches an agent from a literal written in a file (`[1, 2, 3]`), from a
field of a record that holds a list, or from another agent that calls it. So `Caller` writes the lists and calls
`Vectors`. Any other agent that sends lists can call `Vectors` the same way. `link Vectors` finds `vectors.ag` in the same
folder.

Three things about the language that this example meets:

1. **A list comes from a literal or from another agent, never from the command line.**
2. **A number in a list is written with digits only.** There is no minus sign in a literal: `[1, -2]` stops with
   `I do not understand the character `-``. The example has no negative numbers.
3. **The `accepts` lines come before the `on` handlers**, as in the documented grammar.

## 6. The four chapters

Each chapter makes the same tool in one language. Do the one you know, or all four. Every chapter ends with the same
answer from the same two agents.

### 6.1 Python

**Check what you have** (you need 3.10 or newer, because of the `mcp` package):

```text
python3 --version
```

On Windows use `py --version`. If `python3` is not found on your computer, use the name that works (it may be `python`).
Start in the `mcp-tools` folder. After the `cd python` below, run every command **from the `python` folder**: Metagente starts the tool from the folder where you start
it, so the relative path of the tool (`.venv/bin/python server.py`) is read from there.

**Make the project folder and the virtual environment.** A virtual environment keeps the packages of this project inside
the project folder.

**Linux and macOS**

```text
mkdir python
cd python
python3 -m venv .venv
```

**Windows (PowerShell)**

```text
mkdir python
cd python
py -m venv .venv
```

**Install the SDK.** Save this file as `requirements.txt`. It pins one exact version of the official `mcp` package.

**File: `python/requirements.txt`**

```text
mcp==2.3.0
```

**Linux and macOS**

```text
.venv/bin/pip install -r requirements.txt
```

**Windows (PowerShell)**

```text
.venv\Scripts\pip install -r requirements.txt
```

**Write the tool.** Save this as `server.py`:

**File: `python/server.py`**

```python
from typing import TypedDict

from mcp.server import MCPServer
from mcp.server.mcpserver.exceptions import ToolError

# The name and the version are what this server tells Metagente about itself.
# log_level keeps the SDK's routine messages (they go to the standard error) out of the way.
mcp = MCPServer("calc", version="1.0.0", log_level="WARNING")


# Every tool answers with an object that has one field, "result".
class Number(TypedDict):
    result: float


class Numbers(TypedDict):
    result: list[float]


@mcp.tool()
def mean(values: list[float]) -> Number:
    """Average of a list of numbers."""
    if not values:
        raise ToolError("values must not be empty")
    return {"result": sum(values) / len(values)}


@mcp.tool()
def dot(a: list[float], b: list[float]) -> Number:
    """Dot product of two lists of numbers of the same size."""
    if len(a) != len(b):
        raise ToolError("vectors must have the same size")
    if not a:
        raise ToolError("values must not be empty")
    return {"result": sum(x * y for x, y in zip(a, b))}


@mcp.tool()
def scale(values: list[float], factor: float) -> Numbers:
    """Every number of the list multiplied by the factor."""
    if not values:
        raise ToolError("values must not be empty")
    return {"result": [v * factor for v in values]}


if __name__ == "__main__":
    # Talk to Metagente through standard input and output.
    # Never print anything else to standard output: it would break the conversation.
    mcp.run()
```

What each part does:

* `MCPServer("calc", version="1.0.0", log_level="WARNING")` makes the server. The name and the version are what it tells
  Metagente about itself. `log_level` keeps the SDK's routine messages out of the way.
* `class Number(TypedDict)` and `class Numbers(TypedDict)` say that a tool answers with an object that has one field,
  `result`. The SDK turns that into both the `structuredContent` and the text block of the answer.
* `@mcp.tool()` turns a function into a tool. The **name** of the function is the name of the tool, the **docstring** is
  its description, and the **types** of the arguments (`list[float]`, `float`) become the input schema. That is why
  `values: list[float]` is a list of numbers.
* `raise ToolError("...")` makes a wrong input an error of the tool. The SDK sends the text with the error flag set, and
  Metagente prints it as `the tool ... failed`.
* `mcp.run()` talks to Metagente through standard input and output. **Never print anything else to the standard
  output** (no `print`): it is where the conversation goes. For messages to yourself use the standard error
  (`print(..., file=sys.stderr)`); it shows on your terminal.

**Write the agents and the settings.** Save `vectors.ag` and `caller.ag` from part 5 in this folder, and this file as
`metagente.toml`:

**File: `python/metagente.toml`**

```toml
# Which program is the tool "calc". The agents do not know the language; change this line to change it.
[version]
minimum = "0.1.5"

[parameters]
calc_command = ".venv/bin/python server.py"
# Windows: calc_command = '.venv\Scripts\python.exe server.py'
```

`[version] minimum = "0.1.5"` makes an older Metagente stop with a plain message instead of running this wrongly. The
line `calc_command` is the program that is the tool. The line that starts with `#` is the same command for Windows.

**Test the tool, without any agent:**

**Linux and macOS**

```text
python3 ../smoke.py -- .venv/bin/python server.py
```

**Windows (PowerShell)**

```text
py ..\smoke.py -- .venv\Scripts\python.exe server.py
```

It prints the twelve lines of part 4 (`11 of 11 checks passed`). If it does not, fix the tool before going on.

**Run the agent:**

**Command**

```text
metagente run caller.ag go
```
**Output**

```text
mean 5, dot 32, doubled [2, 4, 6]
```

That is a tool written in Python, used by an agent.

### 6.2 TypeScript

**Check what you have** (Node.js 20 or newer, with npm):

```text
node --version
npm --version
```

Start in the `mcp-tools` folder (if you are inside another project folder, go up with `cd ..`). Run every command after
that from the `typescript` folder.

```text
mkdir typescript
cd typescript
```

**Describe the project.** Save this as `package.json`. It names the packages, with exact versions. `"type": "module"`
matters: the SDK is made of ES modules. `zod` is how you describe the values a tool takes. `typescript` is the compiler
and `@types/node` helps it know Node.js.

**File: `typescript/package.json`**

```json
{
  "name": "calc-server",
  "version": "1.0.0",
  "private": true,
  "type": "module",
  "scripts": {
    "build": "tsc"
  },
  "dependencies": {
    "@modelcontextprotocol/server": "2.4.0",
    "zod": "4.6.7"
  },
  "devDependencies": {
    "@types/node": "26.6.6",
    "typescript": "7.0.2"
  }
}
```

Save this as `tsconfig.json`. It tells the compiler to read `src` and write the result to `dist`:

**File: `typescript/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "outDir": "dist",
    "rootDir": "src",
    "strict": true,
    "skipLibCheck": true,
    "types": ["node"]
  },
  "include": ["src"]
}
```

**Install the packages** (they go into the `node_modules` folder of this project; npm also writes `package-lock.json`,
which records every exact version):

```text
npm install
```

**Write the tool.** Make a folder `src` and save this as `src/server.ts`:

**File: `typescript/src/server.ts`**

```typescript
import { McpServer } from '@modelcontextprotocol/server';
import { serveStdio } from '@modelcontextprotocol/server/stdio';
import * as z from 'zod/v4';

// Every tool answers with an object that has one field, "result".
// It is sent twice: as structuredContent, and as the text of one text block.
function answer(result: number | number[]) {
  const structuredContent = { result };
  return { content: [{ type: 'text' as const, text: JSON.stringify(structuredContent) }], structuredContent };
}

// A wrong input is an error of the tool: the text, with isError set.
function failure(message: string) {
  return { content: [{ type: 'text' as const, text: message }], isError: true };
}

// The name and the version are what this server tells Metagente about itself.
function createServer(): McpServer {
  const server = new McpServer({ name: 'calc', version: '1.0.0' });

  server.registerTool(
    'mean',
    {
      description: 'Average of a list of numbers.',
      inputSchema: z.object({ values: z.array(z.number()) }),
    },
    async ({ values }) => {
      if (values.length === 0) return failure('values must not be empty');
      return answer(values.reduce((sum, v) => sum + v, 0) / values.length);
    },
  );

  server.registerTool(
    'dot',
    {
      description: 'Dot product of two lists of numbers of the same size.',
      inputSchema: z.object({ a: z.array(z.number()), b: z.array(z.number()) }),
    },
    async ({ a, b }) => {
      if (a.length !== b.length) return failure('vectors must have the same size');
      if (a.length === 0) return failure('values must not be empty');
      return answer(a.reduce((sum, x, i) => sum + x * b[i], 0));
    },
  );

  server.registerTool(
    'scale',
    {
      description: 'Every number of the list multiplied by the factor.',
      inputSchema: z.object({ values: z.array(z.number()), factor: z.number() }),
    },
    async ({ values, factor }) => {
      if (values.length === 0) return failure('values must not be empty');
      return answer(values.map((v) => v * factor));
    },
  );

  return server;
}

// Talk to Metagente through standard input and output.
// Never print anything else to standard output (console.log): it would break the conversation.
// For messages to yourself use console.error.
void serveStdio(createServer);
```

What each part does:

* `answer(...)` builds the answer of a tool: an object with one field, `result`, sent twice: as `structuredContent`, and
  as the text of one text block.
* `failure(...)` makes a wrong input an error of the tool: the text, with `isError` set.
* `new McpServer({ name: 'calc', version: '1.0.0' })` is the name and version the server tells Metagente.
* `server.registerTool(name, { description, inputSchema }, handler)` adds a tool. `z.array(z.number())` says "a list of
  numbers"; the SDK turns it into the input schema, and checks the values before your function runs.
* `serveStdio(createServer)` talks to Metagente through standard input and output. **Never use `console.log`**: it
  writes to the standard output, where the conversation goes. For messages to yourself use `console.error`.

**Compile it** (the tool is started as plain `node`, so the TypeScript is compiled once, now, and never at call time):

```text
npm run build
```

**Write the agents and the settings.** Save `vectors.ag` and `caller.ag` from part 5 in this folder, and this file as
`metagente.toml`:

**File: `typescript/metagente.toml`**

```toml
# Which program is the tool "calc". The agents do not know the language; change this line to change it.
[version]
minimum = "0.1.5"

[parameters]
calc_command = "node dist/server.js"
# Windows: calc_command = 'node dist\server.js'
```

**Test the tool, without any agent:**

**Linux and macOS**

```text
python3 ../smoke.py -- node dist/server.js
```

**Windows (PowerShell)**

```text
py ..\smoke.py -- node dist\server.js
```

It prints the same twelve lines. **Run the agent:**

**Command**

```text
metagente run caller.ag go
```
**Output**

```text
mean 5, dot 32, doubled [2, 4, 6]
```

The agents did not change. Only the tool did.

### 6.3 Go

**Check what you have** (Go 1.25 or newer):

```text
go version
```

Start in the `mcp-tools` folder (if you are inside another project folder, go up with `cd ..`). Run every command after
that from the `go` folder.

**Make the module and add the SDK.** `go mod init` starts a module, `go get` adds the official SDK at one exact version
(it writes `go.mod` and `go.sum`):

```text
mkdir go
cd go
go mod init calc-server
go get github.com/modelcontextprotocol/go-sdk@v1.8.0
```

**Write the tool.** Save this as `main.go`:

**File: `go/main.go`**

```go
package main

import (
	"context"
	"errors"
	"log"

	"github.com/google/jsonschema-go/jsonschema"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

// Every tool answers with an object that has one field, "result".
type Number struct {
	Result float64 `json:"result"`
}

type Numbers struct {
	Result []float64 `json:"result"`
}

type MeanInput struct {
	Values []float64 `json:"values"`
}

type DotInput struct {
	A []float64 `json:"a"`
	B []float64 `json:"b"`
}

type ScaleInput struct {
	Values []float64 `json:"values"`
	Factor float64   `json:"factor"`
}

// A wrong input is an error of the tool: the SDK sends the text with isError set.
func mean(ctx context.Context, req *mcp.CallToolRequest, in MeanInput) (*mcp.CallToolResult, Number, error) {
	if len(in.Values) == 0 {
		return nil, Number{}, errors.New("values must not be empty")
	}
	sum := 0.0
	for _, v := range in.Values {
		sum += v
	}
	return nil, Number{sum / float64(len(in.Values))}, nil
}

func dot(ctx context.Context, req *mcp.CallToolRequest, in DotInput) (*mcp.CallToolResult, Number, error) {
	if len(in.A) != len(in.B) {
		return nil, Number{}, errors.New("vectors must have the same size")
	}
	if len(in.A) == 0 {
		return nil, Number{}, errors.New("values must not be empty")
	}
	sum := 0.0
	for i := range in.A {
		sum += in.A[i] * in.B[i]
	}
	return nil, Number{sum}, nil
}

func scale(ctx context.Context, req *mcp.CallToolRequest, in ScaleInput) (*mcp.CallToolResult, Numbers, error) {
	if len(in.Values) == 0 {
		return nil, Numbers{}, errors.New("values must not be empty")
	}
	out := make([]float64, len(in.Values))
	for i, v := range in.Values {
		out[i] = v * in.Factor
	}
	return nil, Numbers{out}, nil
}

// The SDK would work out the input schema from the structs above, but it describes a list as
// "null or array". These schemas say what we mean: a list of numbers, and a number.
// Each one is made new every time, because a schema cannot appear twice in a tree.
func numbers() *jsonschema.Schema {
	return &jsonschema.Schema{Type: "array", Items: &jsonschema.Schema{Type: "number"}}
}

func number() *jsonschema.Schema {
	return &jsonschema.Schema{Type: "number"}
}

func input(required []string, properties map[string]*jsonschema.Schema) *jsonschema.Schema {
	return &jsonschema.Schema{Type: "object", Properties: properties, Required: required}
}

func main() {
	// The name and the version are what this server tells Metagente about itself.
	server := mcp.NewServer(&mcp.Implementation{Name: "calc", Version: "1.0.0"}, nil)
	mcp.AddTool(server, &mcp.Tool{
		Name:        "mean",
		Description: "Average of a list of numbers.",
		InputSchema: input([]string{"values"}, map[string]*jsonschema.Schema{"values": numbers()}),
	}, mean)
	mcp.AddTool(server, &mcp.Tool{
		Name:        "dot",
		Description: "Dot product of two lists of numbers of the same size.",
		InputSchema: input([]string{"a", "b"}, map[string]*jsonschema.Schema{"a": numbers(), "b": numbers()}),
	}, dot)
	mcp.AddTool(server, &mcp.Tool{
		Name:        "scale",
		Description: "Every number of the list multiplied by the factor.",
		InputSchema: input([]string{"values", "factor"}, map[string]*jsonschema.Schema{"values": numbers(), "factor": number()}),
	}, scale)

	// Talk to Metagente through standard input and output.
	// Never print anything else to standard output (fmt.Println): it would break the conversation.
	// For messages to yourself use log, which writes to the standard error.
	if err := server.Run(context.Background(), &mcp.StdioTransport{}); err != nil {
		log.Fatal(err)
	}
}
```

What each part does:

* `Number` and `Numbers` are the answers: an object with one field, `result`. The SDK sends it both as
  `structuredContent` and as the text of one text block.
* `mean`, `dot` and `scale` are the tools. Each one takes a struct with the values and returns the result, or an
  `error`. **An error makes a wrong input an error of the tool**: the SDK sends its text with the error flag set.
* `numbers()`, `number()` and `input(...)` write the input schemas by hand. The SDK could work them out from the structs,
  but it describes a list as "null or array". These say what we mean: a list of numbers. Each schema is made new each
  time because the same schema cannot appear twice in one tree.
* `mcp.NewServer(&mcp.Implementation{Name: "calc", Version: "1.0.0"}, nil)` is the name and version the server tells
  Metagente. `mcp.AddTool` adds a tool: its name, its description, its schema and its function.
* `server.Run(ctx, &mcp.StdioTransport{})` talks to Metagente through standard input and output. **Never print to the
  standard output** (no `fmt.Println`). For messages to yourself use `log`, which writes to the standard error.

**Fetch what the code needs and build the program** (once; the tool is started as the finished program, never with
`go run`):

**Linux and macOS**

```text
go mod tidy
go build -o calc-server .
```

**Windows (PowerShell)**

```text
go mod tidy
go build -o calc-server.exe .
```

**Write the agents and the settings.** Save `vectors.ag` and `caller.ag` from part 5 in this folder, and this file as
`metagente.toml`:

**File: `go/metagente.toml`**

```toml
# Which program is the tool "calc". The agents do not know the language; change this line to change it.
[version]
minimum = "0.1.5"

[parameters]
calc_command = "./calc-server"
# Windows: calc_command = '.\calc-server.exe'
```

**Test the tool, without any agent:**

**Linux and macOS**

```text
python3 ../smoke.py -- ./calc-server
```

**Windows (PowerShell)**

```text
py ..\smoke.py -- .\calc-server.exe
```

It prints the same twelve lines. **Run the agent:**

**Command**

```text
metagente run caller.ag go
```
**Output**

```text
mean 5, dot 32, doubled [2, 4, 6]
```

### 6.4 Java

**Check what you have** (JDK 17 or newer, and Maven):

```text
java -version
mvn -version
```

Start in the `mcp-tools` folder (if you are inside another project folder, go up with `cd ..`). Run every command after
that from the `java` folder.

**Make the folders.** Maven expects the source under `src/main/java`:

**Linux and macOS**

```text
mkdir -p java/src/main/java/calc
cd java
```

**Windows (PowerShell)**

```text
mkdir java\src\main\java\calc
cd java
```

**Describe the build.** Save this as `pom.xml`. It pins the official SDK (`io.modelcontextprotocol.sdk:mcp` 2.1.0),
builds one runnable jar with everything inside (`target/calc-server.jar`), and adds `slf4j-nop`, so the SDK's logging
prints nothing:

**File: `java/pom.xml`**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
  <modelVersion>4.0.0</modelVersion>

  <groupId>calc</groupId>
  <artifactId>calc-server</artifactId>
  <version>1.0.0</version>
  <packaging>jar</packaging>

  <properties>
    <maven.compiler.release>17</maven.compiler.release>
    <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
  </properties>

  <dependencies>
    <dependency>
      <groupId>io.modelcontextprotocol.sdk</groupId>
      <artifactId>mcp</artifactId>
      <version>2.1.0</version>
    </dependency>
    <!-- The SDK logs through SLF4J. This one logs nothing, so nothing is printed. -->
    <dependency>
      <groupId>org.slf4j</groupId>
      <artifactId>slf4j-nop</artifactId>
      <version>2.0.16</version>
    </dependency>
  </dependencies>

  <build>
    <finalName>calc-server</finalName>
    <plugins>
      <plugin>
        <groupId>org.apache.maven.plugins</groupId>
        <artifactId>maven-compiler-plugin</artifactId>
        <version>3.14.0</version>
      </plugin>
      <plugin>
        <groupId>org.apache.maven.plugins</groupId>
        <artifactId>maven-shade-plugin</artifactId>
        <version>3.6.0</version>
        <executions>
          <execution>
            <phase>package</phase>
            <goals><goal>shade</goal></goals>
            <configuration>
              <createDependencyReducedPom>false</createDependencyReducedPom>
              <transformers>
                <transformer implementation="org.apache.maven.plugins.shade.resource.ManifestResourceTransformer">
                  <mainClass>calc.Server</mainClass>
                </transformer>
                <transformer implementation="org.apache.maven.plugins.shade.resource.ServicesResourceTransformer"/>
              </transformers>
              <filters>
                <filter>
                  <artifact>*:*</artifact>
                  <excludes>
                    <exclude>META-INF/*.SF</exclude>
                    <exclude>META-INF/*.DSA</exclude>
                    <exclude>META-INF/*.RSA</exclude>
                  </excludes>
                </filter>
              </filters>
            </configuration>
          </execution>
        </executions>
      </plugin>
    </plugins>
  </build>
</project>
```

**Write the tool.** Save this as `src/main/java/calc/Server.java`:

**File: `java/src/main/java/calc/Server.java`**

```java
package calc;

import io.modelcontextprotocol.json.McpJsonDefaults;
import io.modelcontextprotocol.server.McpServer;
import io.modelcontextprotocol.server.transport.StdioServerTransportProvider;
import io.modelcontextprotocol.spec.McpSchema.CallToolRequest;
import io.modelcontextprotocol.spec.McpSchema.CallToolResult;
import io.modelcontextprotocol.spec.McpSchema.ServerCapabilities;
import io.modelcontextprotocol.spec.McpSchema.Tool;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

public class Server {

    // The input schemas: a list of numbers, and a number.
    static Map<String, Object> numbers() {
        return Map.of("type", "array", "items", Map.of("type", "number"));
    }

    static Map<String, Object> number() {
        return Map.of("type", "number");
    }

    static Map<String, Object> input(List<String> required, Map<String, Object> properties) {
        return Map.of("type", "object", "properties", properties, "required", required);
    }

    // The arguments arrive as plain Java values: a list of numbers is a List of Number.
    static List<Double> list(CallToolRequest request, String name) {
        List<Double> values = new ArrayList<>();
        for (Object value : (List<?>) request.arguments().get(name)) {
            values.add(((Number) value).doubleValue());
        }
        return values;
    }

    // Every tool answers with an object that has one field, "result".
    // It is sent twice: as structuredContent, and as the text of one text block.
    static CallToolResult answer(Object result) throws Exception {
        Map<String, Object> object = Map.of("result", result);
        return CallToolResult.builder()
                .addTextContent(McpJsonDefaults.getMapper().writeValueAsString(object))
                .structuredContent(object)
                .build();
    }

    // A wrong input is an error of the tool: the text, with isError set.
    static CallToolResult failure(String message) {
        return CallToolResult.builder().addTextContent(message).isError(true).build();
    }

    static CallToolResult mean(CallToolRequest request) throws Exception {
        List<Double> values = list(request, "values");
        if (values.isEmpty()) return failure("values must not be empty");
        double sum = 0;
        for (double v : values) sum += v;
        return answer(sum / values.size());
    }

    static CallToolResult dot(CallToolRequest request) throws Exception {
        List<Double> a = list(request, "a");
        List<Double> b = list(request, "b");
        if (a.size() != b.size()) return failure("vectors must have the same size");
        if (a.isEmpty()) return failure("values must not be empty");
        double sum = 0;
        for (int i = 0; i < a.size(); i++) sum += a.get(i) * b.get(i);
        return answer(sum);
    }

    static CallToolResult scale(CallToolRequest request) throws Exception {
        List<Double> values = list(request, "values");
        double factor = ((Number) request.arguments().get("factor")).doubleValue();
        if (values.isEmpty()) return failure("values must not be empty");
        List<Double> scaled = new ArrayList<>();
        for (double v : values) scaled.add(v * factor);
        return answer(scaled);
    }

    public static void main(String[] args) {
        // Talk to Metagente through standard input and output.
        // Never print anything else to System.out: it would break the conversation.
        var transport = new StdioServerTransportProvider(McpJsonDefaults.getMapper());

        // The name and the version are what this server tells Metagente about itself.
        McpServer.sync(transport)
                .serverInfo("calc", "1.0.0")
                .capabilities(ServerCapabilities.builder().tools(false).build())
                .toolCall(
                        Tool.builder("mean", input(List.of("values"), Map.of("values", numbers())))
                                .description("Average of a list of numbers.").build(),
                        (exchange, request) -> {
                            try { return mean(request); } catch (Exception e) { return failure(e.toString()); }
                        })
                .toolCall(
                        Tool.builder("dot", input(List.of("a", "b"), Map.of("a", numbers(), "b", numbers())))
                                .description("Dot product of two lists of numbers of the same size.").build(),
                        (exchange, request) -> {
                            try { return dot(request); } catch (Exception e) { return failure(e.toString()); }
                        })
                .toolCall(
                        Tool.builder("scale", input(List.of("values", "factor"), Map.of("values", numbers(), "factor", number())))
                                .description("Every number of the list multiplied by the factor.").build(),
                        (exchange, request) -> {
                            try { return scale(request); } catch (Exception e) { return failure(e.toString()); }
                        })
                .build();
    }
}
```

What each part does:

* `numbers()`, `number()` and `input(...)` write the input schemas: a list of numbers, a number, and an object with
  required values.
* `list(request, name)` reads a list of numbers out of the arguments, which arrive as plain Java values.
* `answer(...)` builds the answer: an object with one field, `result`, sent as the text of one text block and as
  `structuredContent`. `failure(...)` makes a wrong input an error of the tool: the text, with `isError` set.
* `mean`, `dot` and `scale` are the tools. `main` makes the server (name `calc`, version `1.0.0`) and adds each tool with
  its schema, its description and its function.
* `StdioServerTransportProvider` talks to Metagente through standard input and output. **Never print to `System.out`**:
  it is where the conversation goes.

**Build the jar** (once; the tool is started with `java -jar`, never with `mvn exec`):

```text
mvn -q package
```

**Write the agents and the settings.** Save `vectors.ag` and `caller.ag` from part 5 in this folder, and this file as
`metagente.toml`:

**File: `java/metagente.toml`**

```toml
# Which program is the tool "calc". The agents do not know the language; change this line to change it.
[version]
minimum = "0.1.5"

[parameters]
calc_command = "java -jar target/calc-server.jar"
# Windows: calc_command = 'java -jar target\calc-server.jar'
```

**Test the tool, without any agent:**

**Linux and macOS**

```text
python3 ../smoke.py -- java -jar target/calc-server.jar
```

**Windows (PowerShell)**

```text
py ..\smoke.py -- java -jar target\calc-server.jar
```

It prints the same twelve lines. **Run the agent:**

**Command**

```text
metagente run caller.ag go
```
**Output**

```text
mean 5, dot 32, doubled [2, 4, 6]
```

### 6.5 Change the language, change nothing else

With the four projects built, you can use any of the tools from the same two agents without editing them. Make a folder
`switch` next to the four, and copy into it only the two agents and the settings of the Python project:

**Linux and macOS**

```text
cd ..
mkdir switch
cp python/vectors.ag python/caller.ag python/metagente.toml switch/
cd switch
```

**Windows (PowerShell)**

```text
cd ..
mkdir switch
copy python\vectors.ag, python\caller.ag, python\metagente.toml switch\
cd switch
```

Open `metagente.toml` in `switch` and change the `calc_command` line to the tool of another project. The path starts with
`../` because the tool is in a sister folder, and it is read from the folder where you start `metagente` (here, `switch`):

**Change: in `switch/metagente.toml`, the line `calc_command`, to use the TypeScript tool**

```text
calc_command = "node ../typescript/dist/server.js"
```

**Command**

```text
metagente run caller.ag go
```
**Output**

```text
mean 5, dot 32, doubled [2, 4, 6]
```

**Change: the same line, to use the Go tool**

```text
calc_command = "../go/calc-server"
```

**Command**

```text
metagente run caller.ag go
```
**Output**

```text
mean 5, dot 32, doubled [2, 4, 6]
```

**Change: the same line, to use the Java tool**

```text
calc_command = "java -jar ../java/target/calc-server.jar"
```

**Command**

```text
metagente run caller.ag go
```
**Output**

```text
mean 5, dot 32, doubled [2, 4, 6]
```

The same two agents, byte for byte, gave the same answer with a tool in Python, TypeScript, Go and Java. That is what a
tool is for: the agent says **what** (`calc.mean values: values`), and `metagente.toml` says **which program**.

## 7. Break it on purpose

The best way to learn what a message means is to cause it. Each case below changes one thing, runs one command, and shows
exactly what came out. **Do not change your real project.** Make a copy outside the `mcp-tools` folder, with its own new
virtual environment (never copy a `.venv`), and change the copy. The test program goes next to it, so `../smoke.py`
works from the copy as it does from the real project.

**Linux and macOS** (from the `mcp-tools` folder)

```text
mkdir -p ../mcp-tools-try/python
cp smoke.py ../mcp-tools-try/
cp python/server.py python/requirements.txt python/vectors.ag python/caller.ag python/metagente.toml ../mcp-tools-try/python/
cd ../mcp-tools-try/python
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
```

**Windows (PowerShell)** (from the `mcp-tools` folder)

```text
mkdir ..\mcp-tools-try\python
copy smoke.py ..\mcp-tools-try\
copy python\server.py, python\requirements.txt, python\vectors.ag, python\caller.ag, python\metagente.toml ..\mcp-tools-try\python\
cd ..\mcp-tools-try\python
py -m venv .venv
.venv\Scripts\pip install -r requirements.txt
```

After each case, put the changed line back as it was. The messages start with `Problem on line N of FILE:`, then the
line with a mark under the place, then `Fix:`.

### (a) A wrong command

**Change: in `metagente.toml`**

Before:

```text
calc_command = ".venv/bin/python server.py"
```

After:

```text
calc_command = ".venv/bin/pythn server.py"
```

**Command**

```text
metagente run vectors.ag average values=1,2,3
```

**Output**

```text
Problem on line 9 of vectors.ag: I could not start the tool server `.venv/bin/pythn server.py`: No such file or directory (os error 2)
  9 |     r = calc.mean values: values
    |         ^
Fix: check that the program is installed and the command is spelled right
```

Metagente could not start the program. The error comes before the tool sees any value, so the text `1,2,3` plays no
part. **Fix:** check that the program exists (did you make the virtual environment? did you build the tool?) and that
the command is spelled right.

### (b) A misspelled tool

**Change: in `vectors.ag`**

Before:

```text
    r = calc.mean values: values
```

After:

```text
    r = calc.meen values: values
```

**Command**

```text
metagente check vectors.ag
```

**Output**

```text
No problems found in vectors.ag (1 agent).
```

**Command**

```text
metagente run vectors.ag average values=1,2,3
```

**Output**

```text
Problem on line 9 of vectors.ag: `calc` has no action called `meen`
  9 |     r = calc.meen values: values
    |         ^
Fix: did you mean `calc.mean`? (`calc` can do: mean, dot, scale)
```

`metagente check` says all is well. It does **not** start a tool server, so it cannot know the tools of an MCP tool; the
mistake is found when the line runs. The message lists the tools in the order the server announced them, and suggests
the closest one. **Fix:** write `calc.mean`.

### (c) A missing value, and a value the tool does not take

**Change: in `vectors.ag`**

Before:

```text
    r = calc.mean values: values
```

After:

```text
    r = calc.mean
```

**Command**

```text
metagente run vectors.ag average values=1,2,3
```

**Output**

```text
Problem on line 9 of vectors.ag: `mean` needs a value for `values`
  9 |     r = calc.mean
    |         ^
Fix: `mean` expects: values: array
```

`calc.mean` with no values is a valid call, so `check` accepts it, but Metagente checks the values against the tool's
schema before sending and says what the tool expects. **Fix:** give the value, `values: values`.

**Change: in `vectors.ag`**  (a second change, on the same line)

Before:

```text
    r = calc.mean values: values
```

After:

```text
    r = calc.mean values: values valuez: values
```

**Command**

```text
metagente run vectors.ag average values=1,2,3
```

**Output**

```text
Problem on line 9 of vectors.ag: `mean` does not take `valuez`
  9 |     r = calc.mean values: values valuez: values
    |         ^
Fix: did you mean `values`? `mean` expects: values: array
```

The missing values are checked first, so the unknown name is reported with the right value present. **Fix:** remove
`valuez`.

### (d) Text where a list is expected

This case changes no file. Run it in the real project folder of any language (the copy works too):

**Command**

```text
metagente run vectors.ag average values=1,2,3
```

**Output**

```text
Problem on line 9 of vectors.ag: the tool `mean` failed: Error executing tool mean: 1 validation error for meanArguments
values
  Input should be a valid list [type=list_type, input_value='1,2,3', input_type=str]
    For further information visit https://errors.pydantic.dev/2.14/v/list_type
  9 |     r = calc.mean values: values
    |         ^
Fix: check the values you sent, or the tool's own documentation
```

`values=1,2,3` on the command line is the **text** `1,2,3`, not a list, and a list cannot be typed there. Metagente
sends the text as it is, and the tool refuses it. Every SDK refuses it, with its own words after `the tool `mean` failed:`:

| Language | What the SDK wrote |
|----------|--------------------|
| Python | `Error executing tool mean: 1 validation error for meanArguments` ... `Input should be a valid list [type=list_type, input_value='1,2,3', input_type=str]` (shown in full above) |
| TypeScript | `Input validation error: Invalid arguments for tool mean: values: Invalid input: expected array, received string` |
| Go | `validating "arguments": validating root: validating /properties/values: type: 1,2,3 has type "string", want "array"` |
| Java | `Tool (mean) input validation failed: Validation failed: JSON schema validation errors: [/values: string found, array expected]` |

**Fix:** send a list from an agent, as `Caller` does (`values: [2, 4, 4, 4, 5, 5, 7, 9]`).

### (e) The tool's own error

This case changes no file either. `caller.ag` has a message, `oops`, that sends two vectors of different sizes:

**Command**

```text
metagente run caller.ag oops
```

**Output**

```text
Problem on line 13 of caller.ag: Vectors could not answer `similarity`
  13 |     reply Vectors.similarity a: [1, 2] b: [1]
     |           ^
  Vectors could not answer:
    Problem on line 12 of vectors.ag: the tool `dot` failed: Error executing tool dot: vectors must have the same size
      12 |     r = calc.dot a: a b: b
         |         ^
    Fix: check the values you sent, or the tool's own documentation
```

The error is nested. The outer problem is in `caller.ag` (`Vectors could not answer `similarity``), and inside it is the
problem of `vectors.ag`: **the sentence that the tool wrote**, `vectors must have the same size`, reaches you. The
sentence is the same in all four languages; what each SDK adds in front of it differs:

| Language | The inner problem |
|----------|-------------------|
| Python | `the tool `dot` failed: Error executing tool dot: vectors must have the same size` |
| TypeScript | `the tool `dot` failed: vectors must have the same size` |
| Go | `the tool `dot` failed: vectors must have the same size` |
| Java | `the tool `dot` failed: vectors must have the same size` |

**Fix:** send two lists of the same size.

### (f) A tool that prints to its standard output

The standard output of a tool is where the conversation goes, so a tool must write nothing else there. Put `print("hello")`
as the first line of `server.py` in the copy:

**Change: in `server.py`**  (a new first line)

Before:

```text
from typing import TypedDict
```

After:

```text
print("hello")
from typing import TypedDict
```

**Command**

```text
python3 ../smoke.py -- .venv/bin/python server.py
```

**Output**

```text
tool dot: Dot product of two lists of numbers of the same size. | a: array of number (required), b: array of number (required)
tool mean: Average of a list of numbers. | values: array of number (required)
tool scale: Every number of the list multiplied by the factor. | factor: number (required), values: array of number (required)
mean {"values": [2, 4, 4, 4, 5, 5, 7, 9]} -> {"result": 5}
mean {"values": [1, 2, 3, 4]} -> {"result": 2.5}
dot {"a": [1, 2, 3], "b": [4, 5, 6]} -> {"result": 32}
scale {"factor": 2, "values": [1, 2, 3]} -> {"result": [2, 4, 6]}
mean {"values": []} -> error containing "values must not be empty"
dot {"a": [1, 2], "b": [1]} -> error containing "vectors must have the same size"
dot {"a": [], "b": []} -> error containing "values must not be empty"
scale {"factor": 2, "values": []} -> error containing "values must not be empty"
11 of 11 checks passed
```

**Command**

```text
metagente run vectors.ag average values=1,2,3
```

**Output**

```text
Problem on line 9 of vectors.ag: the tool `mean` failed: Error executing tool mean: 1 validation error for meanArguments
values
  Input should be a valid list [type=list_type, input_value='1,2,3', input_type=str]
    For further information visit https://errors.pydantic.dev/2.14/v/list_type
  9 |     r = calc.mean values: values
    |         ^
Fix: check the values you sent, or the tool's own documentation
Exception ignored while flushing sys.stdout:
BrokenPipeError: [Errno 32] Broken pipe
```

**Command**

```text
metagente run caller.ag go
```

**Output**

```text
mean 5, dot 32, doubled [2, 4, 6]
Exception ignored while flushing sys.stdout:
BrokenPipeError: [Errno 32] Broken pipe
```

Nothing seems wrong, and that is the trap. Python keeps what `print` writes in a buffer and sends it only when the
program ends, so it did not get in the way. At the end the pipe was already closed, and Python complained on the
standard error (`Exception ignored while flushing sys.stdout`; the standard error of a tool **shows on your terminal**).
Now make the line push its text out at once, `print("hello", flush=True)`:

**Change: in `server.py`**

Before:

```text
print("hello")
```

After:

```text
print("hello", flush=True)
```

**Command**

```text
python3 ../smoke.py -- .venv/bin/python server.py
```

**Output**

```text
I will not test this server: it wrote text to its standard output that is not a protocol message:
  hello
A tool must write nothing but protocol there. Print messages for yourself to the standard error instead.
```

**Command**

```text
metagente run caller.ag go
```

**Output**

```text
mean 5, dot 32, doubled [2, 4, 6]
```

`smoke.py` refuses the server on purpose, and says why. Metagente 0.1.5 itself skips a line it cannot read, so the agent
still worked here; do not count on that. The protocol keeps the standard output for its messages. A client that is
stricter would fail, and a stray line that **looks like a message** breaks the conversation: when a server printed
`{"jsonrpc":"2.0","id":0,"result":{}}` before it started, Metagente stopped at the handshake with
`I could not start the tool server`. **Fix:** print for yourself
to the **standard error** (`print("hello", file=sys.stderr)` in Python, `console.error` in TypeScript, `log` in Go),
which shows on your terminal.

### (g) A call that takes too long

Metagente waits 30 seconds for a call (the start of the program, the handshake and the call all count). To see the
message, make the tool slow and the wait short. In `server.py` add `import time` at the top and `time.sleep(5)` as the
first line of `mean`, and end the call line in `vectors.ag` with `within 2 seconds`:

**Change: in `server.py`**  (a new first line)

Before:

```text
from typing import TypedDict
```

After:

```text
import time
from typing import TypedDict
```

**Change: in `server.py`**  (a new line, the first of `mean`)

Before:

```text
    """Average of a list of numbers."""
```

After:

```text
    """Average of a list of numbers."""
    time.sleep(5)
```

**Change: in `vectors.ag`**

Before:

```text
    r = calc.mean values: values
```

After:

```text
    r = calc.mean values: values within 2 seconds
```

**Command**

```text
metagente run caller.ag go
```

**Output**

```text
Problem on line 8 of caller.ag: Vectors could not answer `average`
  8 |     m = Vectors.average values: [2, 4, 4, 4, 5, 5, 7, 9]
    |         ^
  Vectors could not answer:
    Problem on line 9 of vectors.ag: `calc.mean` did not finish within 2 seconds
      9 |     r = calc.mean values: values within 2 seconds
        |         ^
    Fix: try again, or allow more time by ending the line with: within 60 seconds
```

The `Fix` suggests `within 60 seconds` as an example of how to ask for more time. Now allow `within 20 seconds`:

**Change: in `vectors.ag`**

Before:

```text
    r = calc.mean values: values within 2 seconds
```

After:

```text
    r = calc.mean values: values within 20 seconds
```

**Command**

```text
metagente run caller.ag go
```

**Output**

```text
mean 5, dot 32, doubled [2, 4, 6]
```

The run finished, after the five seconds of the sleep. This is how you give a tool that is slow to start (a Java
program on a cold disk, for instance) more time. `within` ends a **call** line. Undo all the changes of this case.

## 8. Beyond math: a database

Metagente has no database, and it does not need one: a tool can have one. This last example is a Python tool that reads a
small SQLite database, and an agent that asks it questions. It uses only what comes with Python for the data; the tool
itself is a server like the one of chapter 6.1.

**What the tool does, and what it does not.** The tool has two tools, `orders_of` (the items and quantities that a
customer ordered) and `total_items` (their sum). Each one runs a **fixed query that the tool owns**: the customer is only
ever a value of the query (the `?`), never part of the SQL text, and the database is opened **for reading only**. No SQL
text ever travels from the agent to the tool. That is on purpose: an agent that could send SQL could ask for anything the
database holds, or change it. A tool should offer the questions that are safe to ask, not the whole database.

Make a `database` folder next to the others, with its own virtual environment. Start in the `mcp-tools` folder:

**Linux and macOS**

```text
mkdir database
cd database
python3 -m venv .venv
```

**Windows (PowerShell)**

```text
mkdir database
cd database
py -m venv .venv
```

Save the same `requirements.txt` as in the Python chapter (the same pinned `mcp`), then install it:

**File: `database/requirements.txt`**

```text
mcp==2.3.0
```

**Linux and macOS**

```text
.venv/bin/pip install -r requirements.txt
```

**Windows (PowerShell)**

```text
.venv\Scripts\pip install -r requirements.txt
```

Save this as `make_db.py`. It makes `shop.db` next to itself, with a table of orders and five rows:

**File: `database/make_db.py`**

```python
import sqlite3
from pathlib import Path

# shop.db is made next to this file. An old one is replaced.
path = Path(__file__).parent / "shop.db"
path.unlink(missing_ok=True)

db = sqlite3.connect(path)
db.execute(
    "CREATE TABLE orders (id INTEGER PRIMARY KEY, customer TEXT NOT NULL, item TEXT NOT NULL, quantity INTEGER NOT NULL)"
)
db.executemany(
    "INSERT INTO orders (customer, item, quantity) VALUES (?, ?, ?)",
    [
        ("Ana", "pencil", 3),
        ("Ana", "notebook", 2),
        ("Bruno", "pencil", 1),
        ("Ana", "eraser", 5),
        ("Carla", "notebook", 4),
    ],
)
db.commit()
db.close()
print("made", path.name)
```

Run it:

**Linux and macOS**

```text
python3 make_db.py
```

**Windows (PowerShell)**

```text
py make_db.py
```

```text
made shop.db
```

Save the tool as `server.py`:

**File: `database/server.py`**

```python
import sqlite3
from pathlib import Path
from typing import TypedDict

from mcp.server import MCPServer

# log_level keeps the SDK's routine messages (they go to the standard error) out of the way.
mcp = MCPServer("shop", version="1.0.0", log_level="WARNING")

# The database is next to this file. It is opened for reading only.
DATABASE = "file:" + str(Path(__file__).parent / "shop.db") + "?mode=ro"


class Item(TypedDict):
    item: str
    quantity: int


class Items(TypedDict):
    result: list[Item]


class Total(TypedDict):
    result: int


# The queries are fixed. The customer is only ever a value of the query (the "?"), never part of the SQL text.
@mcp.tool()
def orders_of(customer: str) -> Items:
    """The items and quantities that a customer ordered."""
    db = sqlite3.connect(DATABASE, uri=True)
    rows = db.execute("SELECT item, quantity FROM orders WHERE customer = ? ORDER BY id", (customer,)).fetchall()
    db.close()
    return {"result": [{"item": item, "quantity": quantity} for item, quantity in rows]}


@mcp.tool()
def total_items(customer: str) -> Total:
    """The total quantity that a customer ordered."""
    db = sqlite3.connect(DATABASE, uri=True)
    (total,) = db.execute("SELECT COALESCE(SUM(quantity), 0) FROM orders WHERE customer = ?", (customer,)).fetchone()
    db.close()
    return {"result": total}


if __name__ == "__main__":
    # Talk to Metagente through standard input and output. Print nothing else there.
    mcp.run()
```

Save the agent as `shop.ag`. It has two messages, `items` and `total`, each with the value `customer`:

**File: `database/shop.ag`**

```text
# Reads orders from a database through a tool. The database is the business of the tool, not of Metagente.
agent Shop
  goal "Answer questions about the orders in a small database"
  tool shop from mcp @parameters.shop_command
  accepts items customer        # the name of a customer
  accepts total customer        # the name of a customer
  on items
    r = shop.orders_of customer: customer
    reply r.result
  on total
    r = shop.total_items customer: customer
    reply r.result
```

and the settings as `metagente.toml`. It is the same as the Python one, with `shop_command` in place of `calc_command`:

**File: `database/metagente.toml`**

```toml
# Which program is the tool "shop". The agent does not know the language; change this line to change it.
[version]
minimum = "0.1.5"

[parameters]
shop_command = ".venv/bin/python server.py"
# Windows: shop_command = '.venv\Scripts\python.exe server.py'
```

Ask:

**Command**

```text
metagente run shop.ag items customer=Ana
```
**Output**

```text
[
  {
    "item": "pencil",
    "quantity": 3
  },
  {
    "item": "notebook",
    "quantity": 2
  },
  {
    "item": "eraser",
    "quantity": 5
  }
]
```
**Command**

```text
metagente run shop.ag total customer=Ana
```
**Output**

```text
10
```
A customer with no orders gives an empty answer, and a total of zero:

**Command**

```text
metagente run shop.ag items customer=Nobody
```
**Output**

```text
[]
```
**Command**

```text
metagente run shop.ag total customer=Nobody
```
**Output**

```text
0
```
A name written to look like an attack is only a name. Nobody has it, so the answer is empty, and the text never became
part of the SQL:

**Command**

```text
metagente run shop.ag items "customer=Ana' OR '1'='1"
```
**Output**

```text
[]
```
The same pattern works for anything a language can reach: another database, a spreadsheet, a PDF, an image, a machine
learning model, a device, a library of your company. The agent keeps asking in a line or two, and the tool does the
work.

### Clean up

Everything this guide made is inside two folders: `mcp-tools` (the four projects, `database` and `switch`) and
`mcp-tools-try` (the copy of "Break it on purpose"). From the `database` folder:

**Linux and macOS**

```text
cd ../..
rm -rf mcp-tools mcp-tools-try
```

**Windows (PowerShell)**

```text
cd ..\..
Remove-Item -Recurse -Force mcp-tools, mcp-tools-try
```

That removes the virtual environments, `node_modules`, `dist`, the Go program, `target` and the database. Two things live
**outside** those folders, in your user's folders, shared by all your projects: Go's downloaded modules and Maven's
downloaded libraries. They are left alone; deleting them only makes the next build download them again. To see where they
are:

```text
go env GOMODCACHE
mvn help:evaluate -Dexpression=settings.localRepository -q -DforceStdout
```

The first prints the Go module cache (by default `~/go/pkg/mod`) and the second the local Maven repository (by default
`~/.m2/repository`).

## 9. Safety: what a tool can do

A tool is a program, and it runs with **all the permissions of the person who runs `metagente`**. It can read any file
you can read, use the network, open a database, and write wherever you can write. Say it plainly, and decide with that
in mind:

* The declarations `tool file`, `tool http` and `tool env` limit what the **agent** can ask Metagente's built-in tools to
  do. They do **not** limit the program of an MCP tool. A tool does not ask Metagente for permission to read a file; it
  just reads it.
* A tool receives the **whole environment** of `metagente`, including variables that hold keys, such as a model key.
* What a tool writes to its standard error shows on your terminal, so do not write secrets there.
* So: run only tools you wrote or that you trust, and keep keys out of the environment when you try a tool you do not
  know.

Metagente starts the program you name and talks to it. It does not look inside it.

## 10. A tool, a linked agent, or `think`?

A short rule of thumb:

* **Arithmetic, databases, files, and anything that needs a library** belong in a **tool**. That is what this guide is
  about.
* **A decision between steps** belongs in an **agent** (another agent you `link`).
* **Judging or writing text** belongs to **`think`**, which asks a language model.

## 11. Common problems

| What you see | What it means | What to do |
|--------------|---------------|------------|
| `there is no parameter called `calc_command`` | `metagente` read a `metagente.toml` that has no `calc_command`, or none at all. It looks in the folder where you **started** it, then upward, not next to the `.ag` file | Run `metagente` from the folder that holds the `metagente.toml` (the project folder, or `switch`) |
| `I could not start the tool server `.venv/bin/python server.py`: No such file or directory (os error 2)` | The program of `calc_command` does not exist where you are | Do the step you skipped: make the virtual environment (Python), `npm run build` (TypeScript), `go build -o calc-server .` (Go), `mvn -q package` (Java) |
| `I could not start the tool server `node dist/server.js`: connection closed: initialize response` | The program started and stopped at once (for example `dist` is not built). Its own message shows on your terminal above | Read that message, and build or fix the tool |
| `` `calc.mean` did not finish within 30 seconds `` | The tool is slow, or stuck, or did not start in time | Fix the tool, or give it more time: end the call line with `within 60 seconds` |
| The tool prints to its standard output | The conversation is broken (or only luck keeps it working): `smoke.py` says `I will not test this server: it wrote text to its standard output that is not a protocol message` | Print to the standard error instead |
| `the tool `mean` failed: ...` and a message about the type of `values` | A list arrived as text: you typed `values=1,2,3` on the command line | Send the list from an agent (`Caller`) |
| `Vectors could not answer `similarity`` | The tool refused a call; the problem of the tool is inside it | Read the inner problem and its `Fix` |
| `I do not understand the character `-`` | A negative number in a list in an `.ag` file: there is no minus sign in a literal | Pass the numbers another way, or avoid them in a literal |
| `I could not start the tool server ...: No such file or directory (os error 2)`, and the path in `calc_command` is absolute and has a space | Metagente splits the command on white space, so the path was cut in two. (The relative commands of this guide work in a folder with a space; only an absolute path is cut) | Put each path in double quotes: `calc_command = '"/my folder/project/.venv/bin/python" "/my folder/project/server.py"'` (a TOML single-quoted text holding the double quotes) |
