# Auth chain: three agents that all need a token, and the token goes down the chain

Three small Metagente agents, one `metagente.toml`, one tiny validator program, three terminals, and no language
model key. You will call the first agent with a bearer token and watch the same token travel to the second agent
and then to a tool. Then you will break it on purpose to see the plain messages.

```text
 you (curl, Authorization: Bearer ...)
   |
   v
 Planner  --- A2A, same token --->  Researcher  --- MCP, same token --->  Facts
 (port 8080)                        (port 8081)                           (port 9000)
```

* `planner.ag`: asks the Researcher about a topic.
* `researcher.ag`: looks the fact up with a tool called `kb`.
* `facts.ag`: the tool. It is a Metagente agent, served over MCP.
* `metagente.toml`: says which messages need a token, which program decides if a token is good, and which names
  receive the caller's token (`forward`). It also says which Metagente version the project needs.
* `validator.py`: the program that decides. It is 7 lines. Metagente never opens a token; it hands the token to this
  program on standard input and obeys its exit code (0 means accepted).
* `call.json`: the request you will send.

Nothing in the three `.ag` files mentions tokens. The rules are all in `metagente.toml`.

> **Not tested on Microsoft Windows.** The validator runs in a clean environment (only `PATH` and `SystemRoot`).
> Whether `python` starts that way on Windows has not been tested yet. Linux and macOS are the tested systems.

## Before you start

You need `metagente` (version 0.1.4 or newer), `python3` (the validator uses only the standard library) and `curl`.
On Windows and in some other places the Python program is called `python`: then change `python3` to `python` on the
`validator =` line of `metagente.toml`.

Open a terminal in this folder (`multi-agent-samples/auth-chain`).

## Step 1: say which tokens are good

The validator reads the good tokens from a file next to it (it sees no environment variables, on purpose). The file
is never committed (`valid-tokens.txt` is in `.gitignore`).

```text
echo demo-token-1 > valid-tokens.txt
```

Windows (PowerShell): `"demo-token-1" | Out-File -Encoding ascii valid-tokens.txt`

## Step 2: start the three servers (one terminal each, all in this folder)

```text
metagente serve facts.ag --mcp 9000
metagente serve researcher.ag --a2a 8081
metagente serve planner.ag --a2a 8080
```

Each one prints `Warning: protected messages are served over plain http on this computer only. The token is not
encrypted.` That is expected on one computer. For real use serve with a certificate: see
[multi-agent-samples/https-chain](../https-chain/README.md).

## Step 3: call the Planner with a good token (terminal 4)

```text
curl -s localhost:8080/a2a -H "Content-Type: application/json" -H "A2A-Version: 1.0" -H "Authorization: Bearer demo-token-1" -d @call.json
```

Expected: a JSON answer that contains

```text
"text":"Planner got: Rust is a systems language"
```

The Researcher and the Facts tool each asked the validator about the same token, and each one printed nothing,
because accepted tokens are not logged.

## Step 4: break it on purpose

**No token.**

```text
curl -si localhost:8080/a2a -H "Content-Type: application/json" -H "A2A-Version: 1.0" -d @call.json
```

Answer: `HTTP/1.1 401 Unauthorized` with `www-authenticate: Bearer` and the message

```text
Planner.plan needs a token, and this request had none. Send the header `Authorization: Bearer <token>`.
```

The Planner's terminal prints `Refused Planner.plan: no token.`

**A token that is not good.**

```text
curl -si localhost:8080/a2a -H "Content-Type: application/json" -H "A2A-Version: 1.0" -H "Authorization: Bearer nope" -d @call.json
```

Answer: HTTP 401 and `The token was not accepted for Planner.plan. Get a new token from the system that issues
them.` The terminal prints `Refused Planner.plan: token not accepted (the validator exited with code 1).` The token
itself is never printed.

**A stopped validator.** Stop the Planner (Ctrl-C), change the `validator =` line of `metagente.toml` to
`python3x validator.py` (a program that does not exist), start the Planner again, and repeat Step 3 with
`demo-token-1`. Answer: HTTP 503 (not 401: the validator is broken, the token may be fine) and

```text
I could not ask the validator (python3x validator.py): the program python3x was not found. Nothing was run. Check that the validator works, or raise timeout_seconds in [auth].
```

Put the line back afterwards. (Because the servers read `[auth]` only when they start, editing the file while a
server runs changes nothing, and the server prints `Note: metagente.toml changed [auth] (or [version]). I keep serving
as I started; restart `metagente serve` for the change to apply.`)

**A typo in `protect`.** Change `"Planner.plan"` to `"Planner.plann"` and run `metagente check planner.ag`:

```text
Problem on line 6 of metagente.toml: [auth] protect lists Planner.plann, but no such message is declared.
Fix: did you mean `Planner.plan`?
```

Nothing is served while a name in `[auth]` is wrong, because a typo could leave a message open. A misspelled setting
(`protec = ...`) is stopped the same way.

**A lower version than the project needs.** Change `minimum = "0.1.4"` to `minimum = "9.0.0"` and run any command:

```text
This project needs Metagente 9.0.0 or newer, and this is 0.1.4. Nothing was run.
Fix: download the latest release at https://github.com/cleuton/MetaAgent/releases/latest
```

## Step 5: what to look at

* `researcher.ag` and `facts.ag` judged the same token with the same `validator.py`: the Planner passed it on because
  `forward = ["Researcher", "kb"]` lists them. Remove `"Researcher"` from `forward`, restart the Planner, and repeat
  Step 3: the Planner answers `Researcher refused the call: it needs a token. If it needs the one this agent received,
  add Researcher to forward in [auth].`
* `cache_seconds = 30` remembers an **accepted** token for 30 seconds, so ten calls cost one validator run. A refused
  token is never remembered. A revoked token stays good until its entry ends.
* Read [docs/authentication-guide.md](../../docs/authentication-guide.md) before you put this in front of real users.

## Common problems

| What you see | Why | What to do |
|--------------|-----|------------|
| `I could not ask the validator (python3 validator.py): the program python3 was not found.` | No `python3` on your `PATH` | Write `python` instead of `python3` in `metagente.toml`, or install Python |
| Every call is `401` although the token is in `valid-tokens.txt` | The file has the token on a line with spaces, or you started the servers from another folder | Start the servers from this folder; one token per line, no spaces |
| `Researcher refused the call: it needs a token...` | The name is not in `forward` | Add it to `forward` in `[auth]` and restart the server that makes the call |
| `kb refused the call: it did not accept the token this agent received.` | The tool server uses another validator, or the token is not in its list | Use the same `metagente.toml` and `valid-tokens.txt` for all three, as here |
| `could not listen ... address already in use` | Another program uses 8080, 8081 or 9000 | Stop it, or change the ports in the commands and in the two `.ag` addresses |
| `... is protected, and this address is plain http and open to other computers` | You used `--public` without a certificate | Serve with `tls_cert` and `tls_key`, or drop `--public` |
