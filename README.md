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

## Write your own tools in Python, TypeScript, Go and Java

An agent can call a **tool**: a small program, written in any language, that does one job. Metagente has no database,
no spreadsheets and no machine learning; your tool has whatever its language has. You plug it in with one line:

```text
tool calc from mcp @parameters.calc_command
```

**[Read the interoperability guide: tools in Python, TypeScript, Go and Java](docs/mcp-tools-guide.md)**

- The same example written four times, and **the same two agents** (byte for byte) in every language. Change one line of
  `metagente.toml` and the agent uses a tool in another language.
- A tool that reads a database.
- `smoke.py`, a program that tests a tool before any agent uses it.
- "Break it on purpose": what you see when something goes wrong, and a table of common problems.
- Every command and output was run for real. The files are in [multi-agent-samples/mcp-tools](multi-agent-samples/mcp-tools/README.md).

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

> **Note (0.1.4): not tested on Microsoft Windows.** The validator that `[auth]` runs is started with a clean environment
> (only `PATH` and `SystemRoot`). That has not been tested on Windows yet. Linux and macOS are the tested systems for it.

## What works today

- **Tools in Python, TypeScript, Go and Java:** write a tool in your language and plug it into an agent with one line. See the [interoperability guide](docs/mcp-tools-guide.md).
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
- **Several language models** (new in 0.1.5): `think "..." using fast` chooses which model answers; the models are `[llm.NAME]` sub-tables of `[llm]` in `metagente.toml`, Anthropic or OpenAI at their official addresses, with the model name and the key taken from environment variables.
- **Minimum version and bearer tokens** (new in 0.1.4): `[version]` makes an older Metagente refuse a project; `[auth]` protects messages with a token that a program of yours validates, and passes the token on to the agents and tools you list.

### New in 0.1.5: several language models for `think`

**One optional word on the `think` line, and no new section.** `think "..." using fast` sends that call to the model named
`fast`; a `think` without `using` still uses the model of `[llm]`, exactly as before. The models are sub-tables of the
`[llm]` section you already know: `[llm.fast]`, `[llm.strong]`. Only the agent line and `metagente.toml` change. A
complete, runnable example is [multi-agent-samples/two-models](multi-agent-samples/two-models/README.md).

```text
agent Assistant
  goal "Answer a question in two steps"
  accepts ask question
  on ask
    short = think "Rewrite as a short search query: {question}" using fast
    reply think "Answer this well: {short}" using strong
```

#### `[llm.NAME]`: the settings

| Setting | Required | What it is | If you leave it out |
|---------|----------|------------|---------------------|
| `provider` | yes | `anthropic` or `openai` | an error |
| `model` | `openai`: this or `model_env` | the model name | `anthropic`: `claude-sonnet-5-5` |
| `model_env` | see `model` | the **name of a variable** that holds the model name, read the first time that model is used | the file's `model` |
| `api_key_env` | no | the **name of the variable** that holds the key, read on every call | `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` |

* The name `NAME` (the nickname) starts with a letter and has at most 24 letters, digits, `_` or `-`. It cannot be `provider`,
  `model`, `api_key_env` or `base_url`, nor a word of the language (`reply`, `think`, `using`...).
* A named model always goes to the official address of its company. There is no `base_url`: for another address (a gateway,
  an OpenAI-compatible service) keep using the `[llm]` section, which did not change.
* `using` is only a word right after a `think` prompt, followed by the nickname and the end of the line. Everywhere else it is
  an ordinary name, so a file that has a variable called `using` keeps working.
* The nickname is written in the file. It cannot come from a parameter, a message or a variable.

#### Copy and paste: Anthropic only

```toml
[version]
minimum = "0.1.5"

[llm.claude]
provider = "anthropic"
model_env = "CLAUDE_MODEL"        # the NAME of a variable; you set it in your terminal
api_key_env = "ANTHROPIC_API_KEY"
```

```text
reply think "Say hello in one sentence" using claude
```

#### Copy and paste: OpenAI only

```toml
[version]
minimum = "0.1.5"

[llm.gpt]
provider = "openai"
model_env = "GPT_MODEL"
api_key_env = "OPENAI_API_KEY"
```

```text
reply think "Say hello in one sentence" using gpt
```

#### Copy and paste: both, in one agent

```toml
[version]
minimum = "0.1.5"

[llm.fast]
provider = "anthropic"
model_env = "FAST_MODEL"
api_key_env = "FAST_KEY"

[llm.strong]
provider = "openai"
model_env = "STRONG_MODEL"
api_key_env = "STRONG_KEY"
```

Keep a default model too, by writing it in `[llm]` as before; then `think` without `using` uses it:

```toml
[llm]
provider = "anthropic"
model = "claude-sonnet-5-5"

[llm.gpt]
provider = "openai"
model_env = "OPENAI_MODEL"
```

#### Why the keys live only in environment variables

A key written in a file ends up in a repository, a backup or a screenshot. In `metagente.toml` you write only the
**name** of the variable, and the key stays in your shell. A setting called `key`, `api_key`, `token`, `secret` or
`password` in a sub-table is refused. Metagente never prints a key, never lets an agent read the variable
(`tool env` refuses it even when the agent declares it) and sends it only to the official address of its own company. A
message about a sub-table prints nothing from the file except a short plain name, so a key pasted where a name belongs
is refused without being shown.

#### Use `[version]`

Metagente 0.1.4 and older do not know `[llm.NAME]` (they ignore it with a note) and stop on `using`. Put
`[version] minimum = "0.1.5"` in the project; Metagente 0.1.5 warns when a sub-table is set without it.

#### What you see when something is wrong

Every message has the line and the fix. The main ones:

| Situation | Message |
|-----------|---------|
| A model name that does not exist | `There is no model called fats. The models are: fast, strong.` and `did you mean fast?` |
| A misspelled setting | `[llm.fast] has the setting api_key_evn, which I do not know.` and `did you mean api_key_env?` |
| `model` and `model_env` together | `[llm.fast] has both model and model_env.` |
| A key written in the file | `[llm.fast] has a key written in the file. Keys are never kept here.` |
| An address in a named model | `[llm.fast] has base_url. A named model always uses the official address.` |
| A provider other than the two | `The provider anthropik in [llm.fast] is not one a named model accepts. Use anthropic or openai.` |
| No provider, or `openai` with no model name | `The provider is missing in [llm.fast].` / `The provider openai needs a model name in [llm.fast].` |
| A variable that is not set | `The model fast needs the variable FAST_KEY, and it is not set.` |
| A call fails | `The model fast (anthropic) answered with status 429.` or `...could not be reached: timeout.` (never the text of the service, a model name or an address) |
| `[llm.fast]` without `[version]` | `Warning: [llm.fast] is set but [version] is not. Older programs do not know it.` |

`metagente check` also warns when the key or model variable of a model that is used is not set where you run it. While
`metagente serve` runs, an edit of the models is not applied: one line asks for a restart.

### New in 0.1.4: a minimum version, and tokens for served messages

**No syntax changed.** Both sections live only in `metagente.toml`, never in an `.ag` file. A complete, runnable
example is [multi-agent-samples/auth-chain](multi-agent-samples/auth-chain/README.md) (three agents, one token, no model
key). Read the [authentication guide](docs/authentication-guide.md) before real users.

#### `[version]`: say which Metagente a project needs

```toml
[version]
minimum = "0.1.4"      # three whole numbers, no leading zeros; the project runs on that version and any newer one
```

* An older program stops `run`, `check` and `serve` with *"This project needs Metagente 0.1.4 or newer, and this is
  0.1.3. Nothing was run."* before it looks at anything else in the file. No section, no check.
* **Always set it when you use `[auth]`.** Programs older than 0.1.4 do not know `[auth]`: they ignore it and serve every
  message **without protection**. `[version]` makes them refuse the project. Metagente 0.1.4 warns when `[auth]` is set
  without `[version]`, and `metagente new` writes `[version]` for you.

#### `[auth]`: protect messages with a bearer token

```toml
[auth]
validator = "python3 validator.py"           # your program: the token on standard input, exit code 0 = accepted
protect = ["Planner.plan", "Worker.*"]       # Agent.message, or Agent.* for every message of an agent
forward = ["Researcher", "kb"]               # remote agents and http(s) tools that receive the caller's token
cache_seconds = 30                           # 0 to 300, default 0: remember an ACCEPTED token this long
timeout_seconds = 5                          # 1 to 60, default 5: the validator's time limit
```

| Key | Default | Limits | Meaning |
|-----|---------|--------|---------|
| `validator` | none | text | The command. Started with no shell, in a clean environment (only `PATH`). Required when `protect` is not empty. |
| `protect` | `[]` | list of `Agent.message` or `Agent.*` | Messages that need an accepted token over A2A and MCP on http. |
| `forward` | `[]` | names of `remote` agents and http(s) MCP tools | Receive `Authorization: Bearer <the caller's token>`. Nothing else does. |
| `cache_seconds` | `0` | 0 to 300 | A token the validator accepted is remembered this long. A refusal never is. |
| `timeout_seconds` | `5` | 1 to 60 | Waiting, starting and running the validator, together. |

* Metagente never opens a token, checks a signature or knows who a person is. Your validator decides.
* Without a token, with a malformed one, or with one the validator refuses: HTTP 401 with `WWW-Authenticate: Bearer`.
  If the validator is missing, too slow or broken: HTTP 503. Nothing of the agent runs. Every typo in `[auth]` stops
  `run`, `serve` and `check`.
* The Agent Card, MCP `initialize` and `tools/list`, and every message not in `protect` stay public.
* A protected message is never served in the open: plain http on another address (`--public`) and `--mcp stdio` are
  refused; plain http on this computer starts with a warning.
* `link` cannot reach a protected message from a request that carried no accepted token, and `GetTask` returns a
  protected task only to the token that started it.
* Try it: [multi-agent-samples/auth-chain](multi-agent-samples/auth-chain/README.md).

**Not tested on Microsoft Windows.** The validator runs in a clean environment (only `PATH` and `SystemRoot`) and that
has not been tested on Windows yet.

Messages you will see:

| Situation | Message |
|-----------|---------|
| No token | `Planner.plan needs a token, and this request had none. Send the header Authorization: Bearer <token>.` |
| Malformed token | `The Authorization header of this request is not a bearer token I can read.` |
| Not accepted | `The token was not accepted for Planner.plan.` |
| Validator trouble | `I could not ask the validator (python3 validator.py): it did not answer in 5 seconds. Nothing was run.` |
| Open network | `Planner.plan is protected, and this address is plain http and open to other computers: 0.0.0.0:8080. A token would travel in the open.` |
| Over stdio | `Planner.plan is protected, and stdio has no place for a token.` |
| Linked call | `Admin.delete is protected, and the request that reached Planner did not carry an accepted token.` |
| Task | `This task needs the token that started it.` |
| Misspelled key | `[auth] has the setting protec, which I do not know. Nothing was served, because a misspelled setting could leave messages open.` |
| Name that matches nothing | `[auth] protect lists Planner.plann, but no such message is declared.` |
| Cannot receive | `files is a built in tool, so it cannot receive a token.` |
| Destination wants a token | `Researcher refused the call: it needs a token. If it needs the one this agent received, add Researcher to forward in [auth].` |
| Too old | `This project needs Metagente 0.1.4 or newer, and this is 0.1.3. Nothing was run.` |
| Bad version shape | `[version] minimum must be three numbers like 0.1.4, but it says ">=0.1.4".` |
| Missing `[version]` | `Warning: [auth] is set but [version] is not. Metagente 0.1.3 and older ignore [auth] and would serve every message without protection.` |

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
| Two models in one agent: a fast Anthropic model and an OpenAI model, break it on purpose (new in 0.1.5) | [multi-agent-samples/two-models](multi-agent-samples/two-models/README.md) |
| Bearer tokens end to end: three agents, one token, break it on purpose (new in 0.1.4) | [multi-agent-samples/auth-chain](multi-agent-samples/auth-chain/README.md) |
| Authentication guide: what Metagente does and does not do, the validator, the cache, the checklist (new in 0.1.4) | [docs/authentication-guide.md](docs/authentication-guide.md) |
| **Interoperability guide:** write your own tools in Python, TypeScript, Go and Java, and plug them into an agent with one line; a tool that reads a database | [docs/mcp-tools-guide.md](docs/mcp-tools-guide.md) |
| The tools sample: the same tool in four languages, the same two agents, a database tool | [multi-agent-samples/mcp-tools](multi-agent-samples/mcp-tools/README.md) |
| Changelog | [CHANGELOG.md](CHANGELOG.md) |

## Roadmap

| Stage | State |
|-------|-------|
| Core language, built in and MCP tools, CLI, tutorial | Built |
| Safe agents (per-agent permissions) | Built |
| Dynamic link between agents | Built |
| Multi-line text and external parameters (v0.1.2) | Built |
| Secure connections: https on both ends, `[secure]`, proxy, Python interoperability (v0.1.3) | Built |
| Minimum version and bearer tokens for served messages (v0.1.4) | Built |
| Several language models for `think`: `using NAME`, `[llm.NAME]` (v0.1.5) | Built, live checks pending |
| v1.0: MCP server, A2A Agent Card, A2A tasks, `serve` | Built, release pending |
| **Project Barracuda:** an agent server invoked via A2A or a frontend, with a queue for batch triggering | Next |
| Long running tasks, tokens for unattended callers, agent registry, scheduling, A2A streaming | Ideas |

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
| v0.1.4 feature spec (minimum version and authentication of served messages) | `specs/006-version-and-served-auth/spec.md` |
| v0.1.4 plan, tasks, research and contracts | `specs/006-version-and-served-auth/` |
| v0.1.5 feature spec (several language models for `think`) | `specs/007-multiple-llm-models/spec.md` |
| v0.1.5 plan, tasks, research, contracts and the live checks | `specs/007-multiple-llm-models/` |

## Contributing

Issues, ideas and pull requests are welcome. If you build an agent with Metagente, open an issue and show it;
good ones become samples. 

If Metagente is useful to you, a star on the repository helps other people find it.

## Version

**v0.1.5.** Semantic versioning. The version is kept in this file, in `CHANGELOG.md` and in `Cargo.toml`.