# Metagente

**Build AI agents in a few lines, not a few hundred.**

Metagente is a small language for building AI agents. You describe what the agent is for, which tools it may
use and what it answers; the interpreter does the rest. It speaks **MCP** (Model Context Protocol) and **A2A**
(Agent to Agent) natively, so your agents can use any MCP tool and talk to other agents out of the box.

**Demonstration video**
[![Metagente demo](https://img.youtube.com/vi/zrNYqy84Ak8/maxresdefault.jpg)](https://www.youtube.com/watch?v=zrNYqy84Ak8)

This demonstration is in [**multi-agent-samples**](./multi-agent-samples/metagente-demo-recrutador/)

The interpreter is a single binary written in Rust. No Python environment, no framework to learn.

<!-- DEMO: replace with a GIF of writing weather.ag and running it (10 to 15 seconds). -->
![Demo](docs/demo.gif)

```text
agent Weather
  goal "Answer questions about the weather"
  tool weather from mcp "npx -y weather-mcp"
  accepts ask city
  on ask
    forecast = weather.forecast city: city
    reply "In {city} it will be {forecast.summary}"
```

That is a complete agent: a goal, an MCP tool, an input and a reply.

## Try it in 30 seconds

1. Download the zip for your system from the [latest release](https://github.com/cleuton/MetaAgent/releases/latest):
   `metagente-linux-amd64.zip`, `metagente-macos-arm64.zip` or `metagente-windows-amd64.zip`.
2. Unzip it and open a terminal in the folder.
3. Run the first sample:

```text
./bin/metagente run samples/clock.ag now
```

On Windows use `.\bin\metagente.exe` instead. Then create your own agent:

```text
./bin/metagente new hello
./bin/metagente run hello.ag greet name=World
```

Prefer to build from source? You need [Rust](https://rustup.rs):

```text
git clone https://github.com/cleuton/MetaAgent
cd MetaAgent
cargo build --release
target/release/metagente run examples/clock.ag now
```

## Why Metagente?

Frameworks such as LangChain or CrewAI are powerful, but they assume you are a programmer working inside a
Python project. Metagente makes a different trade:

| | Python agent frameworks | Metagente |
|---|---|---|
| What you write | Python code using a library | A short declarative agent file |
| Install | Python, virtual env, packages | One binary |
| Tools | Framework specific wrappers | Any MCP server |
| Agents talking to agents | Usually in-process | A2A, across processes and machines |
| Safety | Up to your code | Per-agent permissions for folders, env vars and links |
| Errors | Stack traces | Beginner friendly messages |

If you need fine control in Python, use a framework. If you want a working agent quickly, or you want to give
non-programmers a way to build agents, Metagente is for you.

## What works today

- **Language and interpreter:** goals, tools, inputs, replies, `if`, loops and results.
- **Built in tools and MCP tools:** plug in any MCP server.
- **Safe agents:** each agent declares which folders, environment variables and other agents it may use.
- **Dynamic link:** agents call agents by name or path, with interface checks and cycle detection.
- **A2A and MCP server:** expose an agent as an MCP server or with an A2A Agent Card, and send or receive A2A tasks.
- **CLI:** `new`, `check`, `run` and `serve`.
- **Multi-line text** (new in 0.1.2): a prompt with many lines between `"""` and `"""`, kept exactly as typed.
- **External parameters** (new in 0.1.2): addresses and prompts live in `metagente.toml` and agents read them as `@parameters.name`.
- **Secure connections** (new in 0.1.3): `https://` addresses for agents, tools and the model, authenticated proxies, and self-signed certificates for development. See [Secure connections](#secure-connections).
- **Python interoperability** (new in 0.1.3): a Python agent and a Metagente agent call each other over A2A. See [examples/python_interop](examples/python_interop/).

### New in 0.1.2

A prompt with many lines, written as normal lines. It reaches the model exactly as typed
([examples/multiline.ag](examples/multiline.ag)):

```text
agent Recruiter
  goal "Screen candidate resumes"
  accepts screen resume
  on screen
    reply think """You are a technical recruiter.
Read the resume below and list its three strongest points.

Resume:
{resume}"""
```

An address and a prompt that change from one computer to another, kept out of the agent. They are entries of
`[parameters]` in `metagente.toml`, in the agents' folder ([examples/parameters.ag](examples/parameters.ag)):

```toml
[parameters]
a2a_leitor = "http://127.0.0.1:8080"
prompt1 = "you are a recruiter..."
```

```text
agent Screener
  goal "Screen a resume with the help of a remote reader"
  remote leitor at @parameters.a2a_leitor
  accepts screen candidate
  on screen
    resume = leitor.read candidate: candidate
    reply think @parameters.prompt1
```

An agent loaded with `link` reads the `metagente.toml` of the agent that loaded it. Details are in
[docs/syntax.md](docs/syntax.md#parameters).

## Secure connections

*New in 0.1.3.* An address that starts with `https://` just works, for `remote`, for `tool ... from mcp`, for the
`http` tool and for the language model. Nothing changes in your `.ag` files. Everything below is set in
`metagente.toml` (never in the agent, so an agent cannot weaken it).

```text
agent Trip
  goal "Ask a remote weather agent"
  remote Bob at "https://agents.example.com/weather"
  accepts plan city
  on plan
    forecast = Bob.ask city: city
    reply "Bob says: {forecast}"
```

### What the default checking does

Metagente checks the certificate of every server it talks to, using the certificates of your computer. On a computer
that has none (a fresh container, for example) it uses the certificates that are built into the program, so `https`
still works and nothing fails at startup. An expired certificate, a certificate for another name, or one signed by an
authority Metagente does not know is refused with a sentence that names the address, says what is wrong and says how
to fix it. An `https://` address is never allowed to send you to a plain `http://` one: that is refused too, including
an Agent Card that points tasks to `http://`.

### Every setting

All keys are optional. A `metagente.toml` without `[network]` behaves as before (plus working `https` and the proxy
variables of your environment).

```toml
[network]
allow_self_signed = false            # true skips certificate checks for agents, tools and web addresses (development only)
llm_allow_self_signed = false        # true skips certificate checks for the language model only (a private gateway you trust)
# self_signed_hosts = ["localhost", "127.0.0.1", "dev.lab.local"]   # optional: limit allow_self_signed to these hosts
# ca_file = "certs/dev-ca.pem"       # trust one more authority and keep checking (better than skipping checks)

[network.proxy]
# url = "http://proxy.company.com:3128"   # no user name or password inside the address
# username_env = "PROXY_USER"             # the NAME of the variable that holds the user name
# password_env = "PROXY_PASSWORD"         # the NAME of the variable that holds the password
# no_proxy = ["localhost", "127.0.0.1", ".internal.company.com"]
# pass_to_tools = false                   # true gives these settings to tool programs started over stdio
```

| Key | Default | What it does |
|-----|---------|--------------|
| `allow_self_signed` | `false` | Accept self-signed and otherwise untrusted certificates (even a wrong name) for agents, tool servers, web addresses and an `https://` proxy. **Not** for the language model. |
| `llm_allow_self_signed` | `false` | The same, for the language model connection only. |
| `self_signed_hosts` | none | With `allow_self_signed`, skip checks only for these hosts. Every other host is still checked. Does not affect the language model. |
| `ca_file` | none | A PEM file with one or more authorities to trust, for every connection (the model too), with checks still on. A relative path starts at the folder of `metagente.toml`. |
| `[network.proxy] url` | none | The proxy, `http://` or `https://`. Tunnels are opened with `CONNECT` for `https` addresses; `http` addresses are sent through it. |
| `username_env`, `password_env` | none | The **names** of the environment variables that hold the proxy login (Basic). Both or neither. |
| `no_proxy` | none | Hosts that skip the proxy: names, `.domain.suffixes`, IP addresses, `localhost`. |
| `pass_to_tools` | `false` | Give the proxy settings to tool programs started over stdio. |

### A proxy that needs a login

Put the login in two environment variables and name them in `metagente.toml`:

```bash
export PROXY_USER=ann
export PROXY_PASSWORD='p@ss:word'      # any characters work: it is never part of an address
```

```toml
[network.proxy]
url = "http://proxy.company.com:3128"
username_env = "PROXY_USER"
password_env = "PROXY_PASSWORD"
```

That is all: four lines. The login goes only to the proxy, never to the server you are calling, and it is never
printed. **A real user name or password written in `metagente.toml`, or inside the proxy `url`, is refused** with the
line number, because that file is often shared or committed. The same rule is why the model key is also only a
variable name (`api_key_env`).

### The proxy variables you already have

If `metagente.toml` has no proxy `url`, Metagente uses `HTTPS_PROXY` for `https://` addresses, `HTTP_PROXY` for
`http://` addresses and `NO_PROXY` (lower case names work too). A login written in the standard form
(`http://user:pass@host:3128`) is used and never shown. A company that already sets these needs no change at all.
When `metagente.toml` has a `url`, it wins completely: the proxy variables, `NO_PROXY` included, are ignored.
If only `no_proxy` is in `metagente.toml`, it replaces `NO_PROXY` and the proxy still comes from the environment.
`metagente check` says which one is in use.

### Self-signed certificates while developing

```toml
[network]
allow_self_signed = true
```

One line, and every `run`, `serve` and `check` prints, for as long as it is there:

```text
Warning: certificate checks are OFF for agents and tools (allow_self_signed = true in metagente.toml). Use this for development only.
```

The agent file cannot silence it. Prefer `ca_file` whenever you can: it trusts one authority and keeps checking
everything else. `self_signed_hosts = ["dev.lab.local"]` limits the relaxed checking to the hosts you list, and a
redirect to a host that is not on the list is refused.

### A private model gateway: `llm_allow_self_signed`

The language model receives your API key, so it has its own switch, separate on purpose. `allow_self_signed` never
relaxes it, and `llm_allow_self_signed` never relaxes anything else. If you run your own gateway with a self-signed
certificate and trust it:

```toml
[network]
llm_allow_self_signed = true
```

The warning says plainly that the key will be sent to a server whose identity was not verified. The proxy settings
apply to the model connection too.

### Tool programs started over stdio: `pass_to_tools`

A tool server started with a command, for example `tool files from mcp "npx -y some-server"`, is a program on your
computer. Metagente talks to it through its input and output, so `[network]` does not apply to that conversation.
If the program itself downloads things and must use your proxy, set `pass_to_tools = true`: the program then gets
`HTTP_PROXY`, `HTTPS_PROXY` and `NO_PROXY` (and their lower case forms), including the login. Nothing else gets them
and Metagente's own environment is not changed. `metagente check` warns that the login becomes visible to the tool
programs, so declare only tools you trust. Agents themselves still cannot read the login variables with
`tool env`, exactly like the model key.

### Serving over https

`metagente serve` speaks plain http and stays on your own computer unless you add `--public`. To offer an agent over
`https`, put a reverse proxy that ends TLS in front of it. If that proxy sends `X-Forwarded-Proto: https`, the Agent
Card advertises `https://` addresses, so Metagente clients accept it.

### New messages and what to do

| You see | Because | Do this |
|---------|---------|---------|
| `I could not trust the certificate of https://... : it was signed by an authority I do not know, for example a self-signed certificate` | The server's authority is not trusted here. | Add its authority with `ca_file`; for development only, `allow_self_signed = true`. |
| `the certificate of ... is for X, but the address says Y` | The certificate belongs to another name. | Use the name on the certificate. |
| `the certificate of ... expired on 2025-01-31` / `is not valid until ...` | The certificate is out of date, or this computer's clock is wrong. | Renew it, or fix the clock. |
| `... sent me to ..., and I will not follow it: an https address sent me to an http address` | A redirect would leave the protected connection. | Serve over `https`. |
| `the agent card of Bob points tasks to http://..., which is less secure than https://...` | The card downgrades to plain `http`. | Serve the agent over `https`. |
| `I could not trust the certificate of the language model server ...` | The model gateway is not trusted. | `ca_file`, or `llm_allow_self_signed = true` for a gateway you trust. |
| `the proxy http://... refused the login` | Wrong or missing proxy login. | Check `username_env`, `password_env` and the variables they name. |
| `the proxy ... asks for NTLM login, and this version supports only Basic` | The proxy wants Kerberos, NTLM or Digest. | Use a proxy that accepts Basic. |
| `the proxy ... refused to open a connection to host:443 (it answered 403)` | The proxy does not allow that destination. | Ask its administrator, or add the host to `no_proxy`. |
| `I could not reach the proxy ...; https://... was never contacted` | The proxy address is wrong or the proxy is down. | Check `url` or `HTTPS_PROXY`. |
| `the proxy ... did not answer in time` | The proxy accepted the connection and said nothing. | Check the proxy; raise `timeout_seconds`. |
| `I could not trust a certificate while connecting to ... through the proxy ...` | The proxy itself (an `https://` proxy) or the server has an untrusted certificate. | `ca_file` for the proxy's authority, or `allow_self_signed` for development. |
| `... must not hold a real login` (with the line) | A literal user name or password is in `metagente.toml`. | Write the name of a variable in `username_env` / `password_env`. |
| `[network.proxy] has username_env but no password_env` | Half a login. | Set both. |
| `the variable ... named by password_env is not set or is empty` | The variable is missing in this environment. | Set it before starting Metagente. |
| `I could not read the certificate file ...` | `ca_file` points to a missing or unreadable file. | Check the path (it starts at the folder of `metagente.toml`). |

### Python agents and Metagente agents, both ways

Metagente speaks plain A2A, so it works with agents written with the official Python SDK. The folder
[examples/python_interop/](examples/python_interop/) shows both directions in about 10 minutes: a Metagente agent
calls a Python agent, and a Python program calls a Metagente agent.

#### Setting up and running the Python interop tests

You need Python 3.11 or newer. From the top folder of the project:

```bash
python3 -m venv .venv
source .venv/bin/activate                 # Linux and macOS
# .venv\Scripts\activate                  # Windows (cmd or PowerShell)
pip install -r tests/interop/requirements.txt
export METAGENTE_INTEROP_PYTHON="$PWD/.venv/bin/python"          # Linux and macOS
# set METAGENTE_INTEROP_PYTHON=%CD%\.venv\Scripts\python.exe     # Windows cmd
cargo test python_interop
```

Without `METAGENTE_INTEROP_PYTHON` these tests print `SKIPPED` and the exact commands above, and pass. When the `CI`
variable is set (as it is in continuous integration), a missing Python or package makes them fail instead. The
tests start a Python agent, make a self-signed certificate on the spot (none is stored in the repository) and use a
small authenticating proxy that is part of the test code. `a2a-sdk` is pinned to 1.2.0 (A2A 1.0) in
`tests/interop/requirements.txt`, which lists every package the Python side needs.

To run the pieces by hand:

```bash
python tests/interop/python_agent/agent.py --port 9100                       # the Python agent (add --tls-cert and --tls-key for https)
python tests/interop/a2a_sdk_client.py http://127.0.0.1:8081 --message "greet name=Ana"   # the Python client
python tests/interop/helpers/make_cert.py /tmp/certs 127.0.0.1 localhost     # a self-signed certificate
```

## A complete example

[City Briefing](samples/city-briefing/README.md) puts it all together: a Concierge agent asks a Researcher
agent over A2A, and the Researcher reads a web page through an MCP tool. Both use Claude. Bring your own API key.

## Learn more

| Topic | Where |
|-------|-------|
| Tutorial, step by step | [docs/tutorial.md](docs/tutorial.md) |
| Programming guide (if, loops, results) | [docs/guide.md](docs/guide.md) |
| Language syntax, including multi-line text and parameters (new in 0.1.2); no syntax changed in 0.1.3 | [docs/syntax.md](docs/syntax.md) |
| HTTPS, proxies, self-signed certificates and Python interop (new in 0.1.3) | [Secure connections](#secure-connections) |
| Samples | [samples/](samples/) |
| Changelog | [CHANGELOG.md](CHANGELOG.md) |

## Roadmap

| Stage | State |
|-------|-------|
| Core language, built in and MCP tools, CLI, tutorial | Built |
| Safe agents (per-agent permissions) | Built |
| Dynamic link between agents | Built |
| Multi-line text and external parameters (v0.1.2) | Built |
| HTTPS, authenticated proxies, self-signed development certificates, Python interop (v0.1.3) | Built |
| v1.0: MCP server, A2A Agent Card, A2A tasks, `serve` | Built, release pending |
| **Project Barracuda:** an agent server invoked via A2A or a frontend, with a queue for batch triggering | Next |
| Long running tasks, authentication for served agents, agent registry, scheduling, A2A streaming | Ideas |

## How it is built

Metagente is developed with spec-driven development using [GitHub Spec Kit](https://github.com/github/spec-kit).
Every feature starts as a spec, so you can read why things are the way they are:

| Document | Path |
|----------|------|
| Constitution (principles) | `.specify/memory/constitution.md` |
| Feature spec | `specs/001-metagente-core/spec.md` |
| Implementation plan | `specs/001-metagente-core/plan.md` |
| Task list | `specs/001-metagente-core/tasks.md` |
| Language, CLI, configuration and protocol contracts | `specs/001-metagente-core/contracts/` |
| Validation guide | `specs/001-metagente-core/quickstart.md` |
| v0.1.2 feature spec (multi-line text and parameters) | `specs/004-multiline-text-and-parameters/spec.md` |
| v0.1.2 plan, tasks and contracts | `specs/004-multiline-text-and-parameters/` |
| v0.1.3 feature spec (secure connections and Python interoperability) | `specs/005-secure-connections-interop/spec.md` |
| v0.1.3 research (connection path matrix), plan, tasks and contracts | `specs/005-secure-connections-interop/` |

## Contributing

Issues, ideas and pull requests are welcome. If you build an agent with Metagente, open an issue and show it;
good ones become samples. 

If Metagente is useful to you, a star on the repository helps other people find it.

## Version

**v0.1.3.** Semantic versioning. The version is kept in this file, in `CHANGELOG.md` and in `Cargo.toml`.