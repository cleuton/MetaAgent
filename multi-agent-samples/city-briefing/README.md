# City Briefing: two agents, A2A and MCP

Ask about a city and get a short briefing. Two Metagente agents do it together:

```text
you --run--> Concierge --A2A--> Researcher --MCP--> "fetch" tool --> a web page
                  ^                  |
                  |                  +-- clock.now (the time it checked)
                  +---- facts -------+
you <-- briefing (Claude writes it) --- Concierge
```

- The **Researcher** (`researcher.ag`) reads a public page through an MCP tool, checks the clock, and
  asks Claude to write three sentences of facts. It is a small server you leave running.
- The **Concierge** (`concierge.ag`) asks the Researcher for facts over A2A, then asks Claude to turn
  them into a friendly three line briefing. You run it once per question.
- Both use **Claude Sonnet 5.5**. Which model, and where the key comes from, is written only in
  `metagente.toml`. Neither agent file names a provider or a model.

Each agent declares only what it needs, so the Concierge cannot read files and the Researcher cannot
call anything but the fetch tool and the clock.

> **How this was tested:** on 2026-09-29, on Linux, with Metagente v0.1.0. The two agents ran as real
> processes, with the real `uvx mcp-server-fetch` server, a real Wikipedia page and real A2A between them.
> A stand-in program answered in place of Claude, so **the run with the live Claude API is still to be
> recorded**. If you run it, your output may differ from the sample in "A sample question".
> Details: `specs/002-multiagent-sample/validation.md`.

## Prerequisites

1. **Metagente.** From the project folder (the one with `Cargo.toml`), run `cargo build --release`.
   The program is `target/release/metagente`. The commands below say `metagente`; either put that
   folder on your PATH or type the full path.
2. **uv**, which starts the MCP fetch server for you (`uvx mcp-server-fetch`). Install it from
   https://docs.astral.sh/uv/ and check with `uvx --version`. The first run downloads the fetch
   server, which takes about ten seconds.
3. **An Anthropic API key.** Get one from your Anthropic account. Model use is billed to you; one
   briefing is a few short questions to the model.
4. **Two terminals**, both opened in this folder (`samples/city-briefing/`). Metagente finds
   `metagente.toml` in the folder you start it from.

## Configuration

Everything you may want to change is in `metagente.toml`:

```toml
[llm]
provider = "anthropic"
model = "claude-sonnet-5-5"          # change only this line to use another model (both agents follow)
api_key_env = "ANTHROPIC_API_KEY"    # the NAME of the variable that holds your key, never the key itself

[runtime]
timeout_seconds = 90                 # a briefing needs a page fetch and two model answers
think_max_steps = 10                 # how many steps the model may take with its tools

[serve]
a2a_port = 8080                      # where the Researcher listens; the Concierge's address must match
bind = "127.0.0.1"                   # this computer only
```

The file never holds your key. Put the key in an environment variable, **in both terminals** (both
agents talk to Claude):

```bash
export ANTHROPIC_API_KEY=your-key-here
```

Two more things are written inside the agent files, because that is where Metagente declares them:

- `researcher.ag` says which MCP server to start: `tool fetch from mcp "uvx mcp-server-fetch"`.
- `concierge.ag` says where the Researcher lives: `remote Researcher at "http://127.0.0.1:8080"`.

## Running the demo

**Terminal 1**: start the Researcher and leave it running.

```bash
metagente serve researcher.ag
```

It prints where it listens and its agent card address. It only accepts connections from this
computer; the demo never needs the public option.

**Terminal 2**: ask the Concierge about a city.

```bash
metagente run concierge.ag city=Lisbon
```

Wait a few seconds. Stop the Researcher with Ctrl-C when you are done.

## A sample question

You should see something like this (Claude's words will differ each time):

```text
Lisbon is the sunny capital of Portugal, built on seven hills beside the Tagus river.
It is known for trams, tiled buildings, custard tarts and a long history of explorers.
Wander the old Alfama streets, and try a tart before you leave.
```

Three short lines about the city, using only the facts the Researcher found. To see what the
Researcher alone produces, run it directly (no Concierge, no Terminal 1 needed):

```bash
metagente run researcher.ag city=Lisbon
```

It prints three sentences of facts, then the time it checked, for example
`(checked 2026-09-29T18:04:11+00:00)`.

## How it works

`researcher.ag`:

```text
agent Researcher
  goal "Find facts about a city and summarize them"
  tool fetch from mcp "uvx mcp-server-fetch"
  tool clock
  accepts research city  # find facts about a city
  on research
    if not city
      fail "I need a city"
    now = clock.now
    facts = think "Use the fetch tool to read https://en.wikipedia.org/wiki/{city} and write three short sentences of facts about {city}. If the page cannot be read, say that no facts were found."
    reply "{facts}\n(checked {now.text})"
```

`concierge.ag`:

```text
agent Concierge
  goal "Give a visitor a short briefing about a city"
  remote Researcher at "http://127.0.0.1:8080"
  accepts brief city  # a short briefing about a city
  on brief
    facts = Researcher.research city: city
    reply think "Write a friendly briefing of three lines for a visitor to {city}, using only these facts:\n{facts}"
```

Read them out loud: that is all they do. `Researcher.research city: city` is one line that sends a task
to another agent over A2A. `think` asks Claude, which may use only the tools that agent declared.

## Checking the Researcher from outside (A2A)

Any A2A client can talk to the Researcher, not only the Concierge. With Terminal 1 running, from any
terminal:

```bash
# Who is it and what can it do? (the agent card)
curl http://127.0.0.1:8080/.well-known/agent-card.json

# Ask it for facts
curl -X POST http://127.0.0.1:8080/a2a -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"SendMessage","params":{"message":{"messageId":"m1","role":"ROLE_USER","parts":[{"text":"research city=Lisbon"}]}}}'
```

The card lists one skill, `research`. The second command answers with a task in state
`TASK_STATE_COMPLETED` whose artifact holds the facts. Because the Researcher listens on this computer
only, the same commands from another computer are refused.

## Things to try

- Change `model` in `metagente.toml` and run again. Both agents use the new model; no `.ag` file changes.
- Ask about another city: `metagente run concierge.ag city=Porto`.
- Ask for a place that has no page (`city=Xyzzyplugh`): the Researcher says no facts were found.

## Troubleshooting

| You see | What it means | What to do |
|---------|---------------|------------|
| `the language model could not answer: the variable ANTHROPIC_API_KEY is not set` | The terminal you ran in has no key | `export ANTHROPIC_API_KEY=...` in **that** terminal (both need it) |
| `I could not reach http://127.0.0.1:8080/.well-known/agent-card.json (remote agent Researcher)` | The Researcher is not running | Start Terminal 1: `metagente serve researcher.ag` |
| `I could not start the tool server `uvx mcp-server-fetch`` | `uv` is not installed or not on your PATH | Install uv (see Prerequisites), then check `uvx --version` |
| `I need a city` | The city was empty | Pass one: `city=Lisbon` |
| `I could not listen for A2A on 127.0.0.1:8080` | Something else uses that port | Stop it, or change `a2a_port` in `metagente.toml` **and** the address in `concierge.ag` |
| `did not finish within 90 seconds` | The model or the page was slow | Try again, or raise `timeout_seconds` in `metagente.toml` |
| `the agent used 10 steps and did not finish` | The model kept asking for tools | Try again; the limit is `think_max_steps` in `metagente.toml` |
| The answer says no facts were found | The page for that name does not exist | Try another spelling, or a bigger city |
| `the language model could not answer: ... 401` or `... 404` | The key is wrong, or the model name is not one you can use | Check the key, and `model` in `metagente.toml` |

If something else goes wrong, run `metagente check researcher.ag` and `metagente check concierge.ag`:
they point at the exact line of any mistake and say how to fix it.
