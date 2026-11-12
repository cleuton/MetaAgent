# Tools in Python, TypeScript, Go and Java

An agent can call a **tool**: a small program, written in any language, that does one job. This sample has the same
tool written four times (Python, TypeScript, Go and Java) and **the same two agents**, byte for byte, in each language
folder. The agent says `tool calc from mcp @parameters.calc_command`; which program that is, and in which language, is
one line of `metagente.toml`. Change that line, and the same agent uses a tool in another language.

The example is simple math with vectors. A tool server called `calc` has three tools: `mean` (the average of a list of
numbers), `dot` (the dot product of two lists) and `scale` (every number of a list times a number). The agent
`Vectors` asks the tool, and the agent `Caller` sends it the vectors. No language model and no key are needed.

**A tool can do anything that Metagente does not do.** Metagente has no arithmetic, no vectors and no database. A tool
has whatever its language has: databases, spreadsheets, PDFs, images, machine learning libraries, hardware, code your
company already has. The fifth project, `database`, is a tool that reads a small SQLite database.

## The projects

| Folder | What it is |
|--------|------------|
| [python](python/) | The `calc` tool in Python (`mcp` 2.3.0) |
| [typescript](typescript/) | The `calc` tool in TypeScript (`@modelcontextprotocol/server` 2.4.0) |
| [go](go/) | The `calc` tool in Go (`go-sdk` v1.8.0) |
| [java](java/) | The `calc` tool in Java (`io.modelcontextprotocol.sdk:mcp` 2.1.0, built with Maven) |
| [database](database/) | A Python tool that reads orders from a database: what Metagente cannot do |
| `smoke.py` | Tests any `calc` tool without an agent. It prints the same twelve lines for the four languages |

Each folder has its own `README.md` with only the commands to copy and paste. The explanation of every line, how a tool
talks to Metagente, the full list of messages when something goes wrong, and "break it on purpose" are in the
[guide](../../docs/mcp-tools-guide.md).

## How to pick

Pick the language you know. Each project is complete and independent: it does not need the others. To try them all, build
each one and then change only `calc_command` (the guide shows how, with a `switch` folder).

## Before you start

| What | Lowest version | Command that shows it |
|------|----------------|-----------------------|
| Metagente | 0.1.5 | `metagente --version` |
| Python, for `smoke.py` | 3.9 | `python3 --version` (`py --version` on Windows) |
| Python, for the `mcp` package | 3.10 | `python3 --version` |
| Node.js with npm | 20 | `node --version` |
| Go | 1.25 | `go version` |
| JDK, and Maven | 17 | `java -version`, `mvn -version` |

You need only the language you try, plus Python 3.9 or newer for `smoke.py`. Where to get each one is in the guide.

## What is not committed

The things you make when you build (`.venv`, `node_modules`, `dist`, `target`, the Go program, the jar, `shop.db`) are
listed in `.gitignore` and are never part of the sample.

## A word of care

A tool is a program that runs with all your permissions, and it sees the whole environment of `metagente`. Run only tools
you wrote or that you trust. The guide says more.
