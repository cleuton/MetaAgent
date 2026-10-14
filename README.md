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
- **A2A and MCP server:** expose an agent as an MCP server or with an A2A Agent Card, and send or receive A2A tasks. Any agent you serve with `--mcp` is a tool that other agents (or any MCP client) can use: see [An agent as a tool](docs/tutorial.md#an-agent-as-a-tool).
- **CLI:** `new`, `check`, `run` and `serve`.
- **Multi-line text** (new in 0.1.2): a prompt with many lines between `"""` and `"""`, kept exactly as typed.
- **External parameters** (new in 0.1.2): addresses and prompts live in `metagente.toml` and agents read them as `@parameters.name`.
- **Secure connections** (new in 0.1.3): `https://` addresses everywhere, `[secure]` to require https by name, `metagente serve` answering over https (`--tls-cert`, `--tls-key`), self-signed certificates only when `metagente.toml` says so, and authenticated proxies.
- **Python interoperability** (new in 0.1.3): tested against the official A2A Python SDK, in both directions.

### New in 0.1.3: secure connections (https on both ends)

An address that starts with `https://` just works: agents, tools, web addresses and the language model are
reached over TLS, and the certificate is checked against the certificates of your computer (the binary also
carries a set of its own, so it works on a bare machine too). **No syntax changed.** The way a program asks for
a secure connection is the address itself: write `https://` instead of `http://`, written in the file or taken from
`@parameters.name`.

Two things are new on top of that: you can **require** https for a name, and `metagente serve` can **answer** over
https. A complete, runnable example is [multi-agent-samples/https-chain](multi-agent-samples/https-chain/README.md):
an agent calls an agent, which calls a tool, all over https, with a certificate you make on your computer.
New to certificates? Start with the [secure connections guide](docs/secure-connections-guide.md), which teaches them
from zero.

#### Require https for a name: `[secure]`

Optional. Secure communication is optional too: without this section nothing changes. It lives only in
`metagente.toml`, never in an `.ag` file, so an agent cannot weaken it.

```toml
[secure]
agents = ["Bob", "Researcher"]   # agent names, exactly as in the .ag files (case sensitive)
tools  = ["weather", "http"]     # tool names; "http" makes the built in http tool https only
llm    = false                   # true: the language model address must be https
```

* A name in `agents` is a `remote Bob at "..."` you call, **and** an agent you serve.
  Calling it over `http://` is refused with a plain message **before any connection is opened**, and serving it
  needs a certificate (see below).
* A name in `tools` is a tool you reach over an `http://` or `https://` address. A tool that runs as a local program
  (`stdio`) has no address to secure, so listing it is an error that says so. The same goes for built in tools
  other than `http`.
* `llm = true` refuses a model address that starts with `http://`, before the key is sent. The default hosted
  addresses are https and keep working.
* **An agent in `agents` is secure in every direction**: everything it serves is https only, and every network call
  its own handlers make (remote agents, tools reached over http, the `http` tool, the model) must be https, or it is
  refused. An agent it loads with `link` follows the same rule, because a linked agent runs in the context of the
  agent that loaded it. Local tools (`stdio`, `file`, `state`, `env`, `clock`) are not network traffic and keep working.
* For a third party tool, Metagente refuses http and refuses to be sent from https to http. That is everything it can
  guarantee: what the other server does with your data is outside its control. For agents and tools that Metagente
  serves itself, it guarantees the server side too.
* `metagente check` reports an `http://` address for anything listed, a listed name that matches nothing (a probable
  typo, with the closest name), unknown keys and wrong types. A change to `[secure]` applies to the next request
  while serving, with no restart.

#### Serve over https

```text
metagente serve agent.ag --a2a 8443 --mcp 9443 --tls-cert certs/cert.pem --tls-key certs/key.pem
```

or, once, in `metagente.toml`:

```toml
[serve]
tls_cert = "certs/dev-cert.pem"   # PEM certificate (relative paths start at the folder of metagente.toml)
tls_key  = "certs/dev-key.pem"    # PEM private key
```

* With a certificate and key, every port of the process answers https only: there is no plain listener and no
  redirect. Without them, `serve` answers plain http exactly as in 0.1.2. To have both, start two processes.
* The flags win over `metagente.toml` (a relative path given as a flag starts at the current folder), and `serve`
  says in one line when it used a flag instead of a setting. Giving only one of the two is an error that names the
  missing one.
* An agent in `[secure] agents` is **never** served over plain http: without a certificate and key `serve` refuses
  to start. If the file holds several agents and one is secure, the whole process answers https only, and says so.
  A secure agent cannot be served with `--mcp stdio`.
* Every address in the Agent Card is `https://`, whatever header a request carries. (A plain, not secure agent behind
  a reverse proxy that ends TLS and sends `X-Forwarded-Proto: https` still advertises https.)
* The certificate and key are read when `serve` starts. Editing them in `metagente.toml` later needs a restart, and
  `serve` prints one line saying so. An expired certificate is refused; one that expires within 30 days only warns.
* Serving stays on your own computer unless you add `--public`. Https protects the channel, not who is calling:
  there is still no login in this version.

#### A certificate for development, and how to trust it

```text
mkdir certs
openssl req -x509 -newkey rsa:2048 -nodes -days 365 -subj "/CN=localhost" -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" -addext "basicConstraints=critical,CA:FALSE" -keyout certs/dev-key.pem -out certs/dev-cert.pem
```

The certificate must name the host people use in the address (`localhost` and `127.0.0.1` here), and it must say
`CA:FALSE`. The computer that calls it then has to trust it, in one of two ways, in `[network]`:

1. **Preferred, `ca_file`**: `ca_file = "certs/dev-cert.pem"` trusts that certificate (or the authority that signed
   it) and keeps checking everything else, including the name.
2. **Development only, `allow_self_signed`**: skips the certificate checks. Every `run`, `serve` and `check` prints a
   warning that says so, and the agent file cannot silence it. `self_signed_hosts = ["127.0.0.1"]` limits it to
   those hosts.

Trusting a certificate and requiring https are independent: `allow_self_signed` never makes `http://` acceptable for
a name in `[secure]`, and `[secure]` never turns self-signed certificates on.

#### `[network]` and `[network.proxy]`: every key

```toml
[network]
allow_self_signed = false        # true: no certificate checks for agents, tools, the http tool and the proxy (development only)
self_signed_hosts = ["127.0.0.1"] # optional: limit allow_self_signed to these hosts
llm_allow_self_signed = false    # true: no certificate checks for the language model ONLY (a private gateway you trust)
ca_file = "certs/dev-ca.pem"     # trust one more authority, for every connection, and keep checking

[network.proxy]
url = "http://proxy.company.com:3128"   # no user name or password inside the address
username_env = "PROXY_USER"             # the NAME of the variable that holds the user name
password_env = "PROXY_PASSWORD"         # the NAME of the variable that holds the password
no_proxy = ["localhost", "127.0.0.1", ".internal.company.com"]
pass_to_tools = false                   # true: give these settings to tool programs started over stdio
```

All keys are optional and every default is off. `allow_self_signed` does **not** apply to the language model, because
your model key travels on that connection: `llm_allow_self_signed` is a separate switch, and it prints its own warning
(the key is sent to a server whose identity was not verified). A missing `ca_file` is an error that names the file and
the line.

* **Proxy.** Connections go through the proxy (`CONNECT` for https, plain proxying for http), with Basic login when
  `username_env` and `password_env` are set. The login is **only** the names of environment variables: a literal
  `username`, a literal `password`, or `user:pass@` inside `url` is rejected, because `metagente.toml` is often
  shared or committed. The login is sent only to the proxy, is never printed, and those variables cannot be read by
  agents (`tool env`), exactly like the model key.
* **Proxy from the environment.** With no `url`, the usual `HTTPS_PROXY`, `HTTP_PROXY` and `NO_PROXY` variables are
  used (`check` says so once). With a `url`, the toml wins and the environment proxy variables are ignored
  completely, including `NO_PROXY`.
* **`pass_to_tools = true`** gives `HTTP_PROXY`, `HTTPS_PROXY` and `NO_PROXY` (and their lower case forms) to MCP servers
  started over `stdio`, only to those child programs, login included. `check` warns that the login becomes visible to
  those programs, so declare only tools you trust. A `stdio` tool makes no network connection through Metagente, so
  the proxy and certificate settings do not apply to that pipe.

#### What you see when something is wrong

Every message names the thing, says what happened, and ends with the fix. None shows Rust or library text, a key or a
password.

| Message (shape) | Fix |
|-----------------|-----|
| `Bob is declared secure in metagente.toml but this address uses http: http://host:8443` | write `https://`, or remove `Bob` from `[secure] agents` |
| `Researcher is declared secure, so it may only call https addresses. This address uses http: ...` | change the address so that it starts with `https://` |
| `The model address in [llm] uses http, and [secure] llm is on. Nothing was sent.` | use an `https://` address in `[llm]` |
| `Researcher is declared secure, so I will only serve it over https, but no certificate is set.` | set `tls_cert` and `tls_key` in `[serve]`, or use `--tls-cert` and `--tls-key` |
| `I have a certificate but no key (tls_key).` (or the reverse) | add the missing setting |
| `Facts is declared secure and stdio is not https.` | serve it with `--mcp PORT` and a certificate |
| `files runs as a local program, so it has no address to secure.` | remove it from `[secure] tools`, or give the tool an `https://` address |
| `the key in ... does not match the certificate in ...` | use the key that was made with that certificate |
| `the certificate in ... expired on ...` | make a new certificate and key |
| `Warning: The certificate in ... expires on ...` | renew it when you can |
| `https://host/... sent me to http://host/..., and I will not follow it: an https address sent me to an http address` | serve the agent over https, or use the final address |
| `the agent card of Bob points tasks to ..., which is less secure than ...; I will not follow it` | serve the agent over https, so its card points to an https address |
| `the agent card of Bob points tasks to ..., a different host than ...; Bob is declared secure, so I will not follow it` | serve the agent so that its card points to its own address |
| `I could not complete a secure connection to https://...` | check the port and that the server answers https; there is never a retry over http |
| `I could not trust the certificate of ...: it was signed by an authority I do not know, for example a self-signed certificate` | `ca_file` (preferred) or, for development only, `allow_self_signed` |
| `the certificate of ... has expired` / `... is not valid yet` | renew it / check the clock |
| `the certificate of ... is for X, but the address says Y` | use the name that is on the certificate |
| `the proxy ... refused the login` | check `username_env` and `password_env`, and that the variables are set |
| `I could not reach the proxy ...; ... was never contacted` | check `url` in `[network.proxy]` |
| `` `password` in [network.proxy] must not hold a real login `` | write the name of a variable in `password_env` |
| `[secure] lists the agent Bobb, but no agent or remote called Bobb is declared here. Did you mean Bob?` | fix the name |

#### Test it with Python

`cargo test` also checks that a Python agent and a Metagente agent call each other, over plain http, over https with a
self-signed certificate, and through an authenticated proxy, using the official `a2a-sdk`. Those tests need Python
3.11 or newer; without it they say they were skipped (in CI a missing Python fails the build):

```text
python3 -m venv .venv
. .venv/bin/activate            # Windows: .venv\Scripts\activate
pip install -r tests/interop/requirements.txt
export METAGENTE_INTEROP_PYTHON="$PWD/.venv/bin/python"   # Windows: set METAGENTE_INTEROP_PYTHON=.venv\Scripts\python.exe
cargo test
```

To try the two sides by hand, see [examples/python_interop](examples/python_interop/README.md).

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

## A complete example

[City Briefing](samples/city-briefing/README.md) puts it all together: a Concierge agent asks a Researcher
agent over A2A, and the Researcher reads a web page through an MCP tool. Both use Claude. Bring your own API key.

## Learn more

| Topic | Where |
|-------|-------|
| Tutorial, step by step | [docs/tutorial.md](docs/tutorial.md) |
| Programming guide (if, loops, results) | [docs/guide.md](docs/guide.md) |
| Language syntax, including multi-line text and parameters (new in 0.1.2) | [docs/syntax.md](docs/syntax.md) |
| Samples | [samples/](samples/) |
| Secure connections end to end (new in 0.1.3) | [multi-agent-samples/https-chain](multi-agent-samples/https-chain/README.md) |
| Secure connections guide for beginners: what a certificate is, make a self-signed one, run the whole chain over https (new in 0.1.3) | [docs/secure-connections-guide.md](docs/secure-connections-guide.md) |
| Changelog | [CHANGELOG.md](CHANGELOG.md) |

## Roadmap

| Stage | State |
|-------|-------|
| Core language, built in and MCP tools, CLI, tutorial | Built |
| Safe agents (per-agent permissions) | Built |
| Dynamic link between agents | Built |
| Multi-line text and external parameters (v0.1.2) | Built |
| Secure connections: https on both ends, `[secure]`, proxy, Python interoperability (v0.1.3) | Built |
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
| v0.1.3 feature spec (secure communication by declaration) | `specs/005-secure-by-declaration/spec.md` |
| v0.1.3 plan, tasks, research and contracts | `specs/005-secure-by-declaration/` |

## Contributing

Issues, ideas and pull requests are welcome. If you build an agent with Metagente, open an issue and show it;
good ones become samples. 

If Metagente is useful to you, a star on the repository helps other people find it.

## Version

**v0.1.3.** Semantic versioning. The version is kept in this file, in `CHANGELOG.md` and in `Cargo.toml`.