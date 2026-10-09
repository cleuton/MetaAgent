# Your first agent in 15 minutes

You do not need to know how to program. If you can write a list, you can write an agent.

## 1. Get Metagente

Metagente is one file, `metagente`. If you have it, skip to step 2. To build it yourself, install
Rust from https://rustup.rs and run this in the project folder:

```text
cargo build --release
```

The result is `target/release/metagente`. Put it somewhere on your PATH, or type its full path.

Check that it works:

```text
metagente --version
```

## 2. Make an agent

An agent is a small text file that ends in `.ag`. Let Metagente make one for you:

```text
metagente new hello
```

This creates `hello.ag` and a settings file `metagente.toml`. Open `hello.ag`:

```text
agent Hello
  goal "Say hello to someone"
  accepts greet name
  on greet
    reply "Hello, {name}!"
```

Read it out loud. That is all it does:

- `agent Hello` names the agent.
- `goal` says what it is for.
- `accepts greet name` says what other people can ask it: a message called `greet` that comes
  with a `name`.
- `on greet` says what to do when that message arrives.
- `reply` sends the answer back. Names inside `{ }` are filled in.

Lines that belong to something are moved in by **two spaces**.

## 3. Run it

```text
metagente run hello.ag greet name=World
```

You should see `Hello, World!`.

## 4. Check it without running it

```text
metagente check hello.ag
```

Try breaking it: change `goal` to `gaol` and check again. Metagente points at the line and tells
you what to do:

```text
Problem on line 2 of hello.ag: I do not know the word `gaol` here
  2 |   gaol "Say hello to someone"
    |   ^
Fix: did you mean `goal`?
```

Errors always say where, what, and how to fix it.

## 5. Give it a tool

Agents get things done with **tools**. An agent can only use the tools it says it wants. Make a
file called `notes.txt` with some text, then create `reader.ag`:

```text
agent Reader
  goal "Read a file and show what is in it"
  tool file
  accepts show
  on show
    text = file.read path: "notes.txt"
    reply text
```

```text
metagente run reader.ag show
```

`tool file` is the permission. Delete that line and run again: Metagente refuses and tells you which
line to add back. That is how you stay in control of what an agent may do.

The built in tools:

| Tool | Lines you can write |
|------|---------------------|
| `tool file` | `file.read path: "a.txt"`, `file.write path: "a.txt" text: "hi"` |
| `tool http` | `http.get url: "https://..."`, `http.post url: "https://..." body: "hi"` |
| `tool env "NAME"` | `env.get name: "NAME"` (only the names you list) |
| `tool state` | `state.set key: "k" value: "v"`, `state.get key: "k"` (the agent's memory) |
| `tool clock` | `clock.now`, `clock.wait seconds: 5` |

`tool file "data/"` lets the agent use only the `data` folder.

## 6. Use a tool someone else made (MCP)

Many programs offer tools through a standard called MCP. Use one with a single line:

```text
agent Weather
  goal "Answer questions about the weather"
  tool weather from mcp "npx -y weather-mcp"
  accepts ask city
  on ask
    forecast = weather.forecast city: city
    reply "In {city} it will be {forecast.summary}"
```

`tool weather from mcp "..."` starts the program and finds its tools. You call them like any other
tool: `weather.forecast city: city`. If the tool takes too long, add `within 60 seconds` at the end
of the line.

Tool names from other programs may contain dashes or dots, and you write them as they are:
`github.create-issue title: "Hi"`, or `weather.Weather.ask city: "Lisbon"` for a tool called
`Weather.ask`.

(The `weather-mcp` package above is only an example. Use any MCP server you have.)

## 7. Let the agent think

Some jobs need a language model. Add `think`:

```text
agent Helper
  goal "Answer questions"
  accepts ask question
  on ask
    reply think "{question}"
```

The model and your key are settings, never part of the agent. Open `metagente.toml`, remove the
`#` in front of the `[llm]` lines, and set the name of the environment variable that holds your
key. The agent file does not change when you switch providers. When the model needs a tool, it can
use only the tools the agent declared.

## 8. Agents that call agents

Put `weather.ag` and `planner.ag` in the same folder. In the planner:

```text
agent Planner
  goal "Plan a trip using the weather"
  link Weather
  accepts plan city
  on plan
    answer = Weather.ask city: city
    reply "Pack for this: {answer}"
```

`link Weather` finds `Weather` when the line runs, so you can change `weather.ag` without touching
the planner. Metagente checks that `Weather` really accepts `ask`, and stops with a clear message
if two agents call each other in a circle.

## 9. Let others reach your agent

```text
metagente serve weather.ag --a2a 8080
```

Other agents can now find your agent's card at `http://127.0.0.1:8080/.well-known/agent-card.json`
and send it tasks. If the file holds several agents, each one has its own card at
`http://127.0.0.1:8080/agents/Name/.well-known/agent-card.json`. `--mcp stdio` lets MCP programs use
the agents as tools (`Weather.ask`), and `--mcp 9000` does the same over the web on that port. If you
edit the file while it is served, the changes apply to the next request.

To call an agent served elsewhere, declare it with `remote`:

```text
agent Trip
  goal "Ask a remote weather agent"
  remote Bob at "http://127.0.0.1:8080"
  accepts plan city
  on plan
    forecast = Bob.ask city: city
    reply "Bob says: {forecast}"
```

Serving is limited to your own computer unless you add `--public` (there is no login in this version,
so be careful). 
## 10. Secure connections

An address that starts with `https://` works in `remote`, in `tool ... from mcp` and in `http.get`, with no change to your
agent:

```text
remote Bob at "https://agents.example.com/weather"
```

Certificates are checked for you. For a company proxy that asks for a login, a self-signed certificate on your
laptop, or a private model gateway, you add a few lines to `metagente.toml` (never to the agent). They are explained,
with copy and paste examples, in [Secure connections](../README.md#secure-connections). Python programs can call your
agents and your agents can call Python ones: see [examples/python_interop](../examples/python_interop/).

## Where next

- Decisions, loops and putting results together: [guide.md](guide.md)
- All the words of the language: [syntax.md](syntax.md)
- Ideas to try: change the goal, add `if` and `for`, remember things with `tool state`.
