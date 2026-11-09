# Authentication guide: protecting messages with a bearer token

*(new in 0.1.4)* For the people who will run this for real. Plain language, no cryptography needed.

> **Not tested on Microsoft Windows.** The validator runs in a clean environment (only `PATH` and `SystemRoot`).
> Whether `python`, `node` and other programs start with only those on Windows has not been tested yet. Linux and
> macOS are the tested systems for this feature. If you try it on Windows, a small `.cmd` wrapper that sets what your
> program needs and then starts it is the fallback.

## 1. What Metagente does, and what it does not

Metagente does **one thing** here: it reads the token from the request, hands it to **your** program, and obeys the
yes or no that comes back. It can also pass the same token on to the agents and tools you list.

Metagente does **not**:

* log anyone in, issue, renew or revoke a token;
* open a token, read its contents (the "claims"), check its signature, or fetch keys;
* know who a person is, what a role is, or who may do what.

All of that is the validator's job, and the validator belongs to your organization (your identity provider, your
gateway, your security team). That is on purpose: Metagente fits whatever you already have, and it stays small.

## 2. How it works, in one picture

```text
caller --- Authorization: Bearer <token> ---> metagente serve
                                                  |  1. find the message (Planner.plan)
                                                  |  2. is it in [auth] protect? no -> run it as always
                                                  |  3. yes -> run your validator, token on standard input
                                                  |  4. exit code 0 -> run the message; anything else -> refuse
```

`metagente.toml`:

```toml
[version]
minimum = "0.1.4"        # always set this when you use [auth] (see section 11)

[auth]
validator = "python3 validator.py"      # your program
protect = ["Planner.plan", "Researcher.*"]   # Agent.message, or Agent.* for every message of an agent
forward = ["Researcher", "kb"]          # who receives the caller's token (section 8)
cache_seconds = 30                      # 0 to 300, default 0
timeout_seconds = 5                     # 1 to 60, default 5
```

No `.ag` file changes. An `.ag` file cannot add, change or weaken any of this.

## 3. Every failure refuses

| Situation | Answer |
|-----------|--------|
| No `Authorization` header | HTTP 401, `WWW-Authenticate: Bearer` |
| A header that is not exactly one `Bearer <token>` | HTTP 401 (the validator is not even started) |
| The validator says no (any exit code but 0) | HTTP 401 |
| The validator is missing, cannot start, takes longer than `timeout_seconds`, or is killed | HTTP 503 (the validator is broken; the token may be fine) |
| A typo in `[auth]` (an unknown key, a wrong type, a name that matches nothing) | `run`, `serve` and `check` stop; nothing is served |
| A protected message on an open network | `serve` refuses to start (section 7) |

Nothing of the agent runs before an accepted token: not the handler, not a tool.

## 4. The validator contract

* Started **directly** (no shell), so nothing in a token can be read as a command.
* The **token goes to standard input, followed by one line break**, and then the input is closed.
* **Exit code 0 means accepted.** Anything else (another exit code, a crash, a signal) means not accepted.
* Standard output and standard error are thrown away and never shown.
* The working directory is the folder of `metagente.toml`. A first word with a `/` is relative to that folder; a bare
  name is looked up in `PATH`.
* The environment is **clean**: only `PATH` (and `SystemRoot` on Windows). The validator never sees your model key,
  proxy login or any other variable, and the token is never in its arguments or environment. If your validator needs a
  setting, read it from a file next to it.
* At most 8 validators run at the same time; the others wait. `timeout_seconds` covers waiting, starting and running
  together.

Python (standard library only):

```python
import sys

token = sys.stdin.readline().strip()
with open("valid-tokens.txt") as f:
    accepted = {line.strip() for line in f if line.strip()}
sys.exit(0 if token in accepted else 1)
```

The same in a shell script (`validator = "./validate.sh"`, make it executable):

```sh
#!/bin/sh
read -r token
grep -qxF "$token" valid-tokens.txt
```

Any language works: a program in Go or Rust that checks a signature against your keys, `node validator.js`,
`java -jar validator.jar`, or a tiny script that calls your identity provider. Read one line from standard input,
decide, exit.

### Test your validator by hand, with a good token and a bad one

```text
printf 'demo-token-1\n' | python3 validator.py ; echo "exit code: $?"      # expect 0
printf 'not-a-token\n'  | python3 validator.py ; echo "exit code: $?"      # expect 1
```

Do both. A validator that exits 0 for everything protects nothing, and that mistake is yours, not Metagente's.
Test it with a bad token **before** you rely on it.

## 5. The token

A token is read from exactly one `Authorization` header: `Bearer <token>` (the word in any letter case, one space). It
may contain only letters, digits and `- . _ ~ + /`, with `=` allowed at the end, and at most 16384 characters. That
covers a JWT. Anything else is refused as malformed without starting the validator.

## 6. The cache, and the revocation window

`cache_seconds` remembers a token the validator **accepted**, for that many seconds from the moment of acceptance (using
it does not extend it). A refusal, a malformed header and a 503 are never remembered. It lives only in memory, holds
at most 1000 entries and is lost on restart; Metagente keeps a fingerprint of the token, never the token.

**A revoked token stays good until its entry ends.** With `cache_seconds = 30`, a token you revoke can still be used
for up to 30 seconds by a process that accepted it just before. Choose the number you can live with, or keep it at 0.
In a chain of agents that all pass the token on, each agent judges the token again (with its own cache), so a revoked
token stops at the first agent that does not have it cached.

## 7. https, or this computer only

A protected message is never served in the open on a network:

* Plain `http` on another address (for example `--public`) is **refused at start**:
  `Planner.plan is protected, and this address is plain http and open to other computers: 0.0.0.0:8080. A token would travel in the open.`
* Plain `http` on this computer (`127.0.0.1`, `localhost`, `::1`) starts with a warning. It is fine for development and
  for a reverse proxy that ends https in front of Metagente.
* `--mcp stdio` is refused for a protected message: a pipe has no place for a header.
* With `tls_cert` and `tls_key` in `[serve]` (or `--tls-cert`, `--tls-key`) it starts. For a real deployment, use that.

## 8. Passing the token on

An agent that receives a token can pass it on, **only to the names in `forward`**: a `remote` agent or an MCP tool
reached by an `http://` or `https://` address. For a listed name, calls that run something (a task to a remote agent,
a tool call) carry `Authorization: Bearer <the same token>`. Nothing else does:

* a name that is not listed receives nothing, and the call is exactly what it was before 0.1.4;
* a request without a token sends no header at all;
* stdio tools, the built in `http` tool and the language model **never** get the token (listing them is an error);
* a redirect never carries the token;
* a listed destination at a plain-http address that is not on this computer is refused before any connection.

Things to know:

* **The next agent receives the same power as the first.** Keep `forward` short and list only agents and tools you
  trust with the caller's identity.
* The `.ag` language has no way to read the token: not as a name, not through `env`, not in a `think` prompt, not in a
  `reply`. The `on` block does not know who the caller is.
* **The person is identified by whoever receives the passed on token**, for example an MCP tool that filters documents
  by user. Metagente never decides who the person is.
* A model asking for a listed MCP tool through `think` sends the token with that call too. Each distinct token gets its
  own session with a tool server (kept for a while, then dropped).
* If a destination answers 401, the message says so: if the name is not in `forward`, it tells you to add it.

## 9. The side doors are closed

* **`link`**: a protected message is protected however it is reached. If a request that came over the network (no
  token, or a public message) tries to reach a protected message of a linked agent, the call is refused: *"Admin.delete
  is protected, and the request that reached Planner did not carry an accepted token."* Protect the first message too,
  or call the protected one over the network. `metagente run` at the terminal and `on start` have no request and no
  outsider, so nothing is judged there. A linked agent uses the `[auth]` of the agent that loaded it.
* **`GetTask`**: a task that came from a protected message is read back only with the token that started it. No token,
  a refused token, or another accepted token gets 401 (*"This task needs the token that started it."*). If a person's
  token was renewed in the meantime, the old task cannot be read with the new one. Because the token is judged again on
  every call, with `cache_seconds = 0` each poll costs one validator start.

## 10. What stays public, and what it reveals

The Agent Card, MCP `initialize` and `tools/list`, and every message that is not in `protect` need no token. So a person
without a token can still see **which messages exist, their parameters and which ones need a bearer token** (the card
says so, skill by skill). If that list is itself sensitive, put the service behind your gateway as well.

## 11. Always set `[version]` when you use `[auth]`

Programs older than 0.1.4 do not know `[auth]`: they ignore it, print a short note, and **serve every message without
protection**. `[version] minimum = "0.1.4"` makes those programs refuse the project instead. Metagente 0.1.4 prints a
warning when `[auth]` is set without `[version]`. `[version]` and `[auth]` are read when `run` or `serve` starts; editing
them while serving changes nothing and prints one line asking for a restart (so a deleted section can never open a
message unnoticed).

## 12. The lines you will see in the log

```text
Refused Planner.plan: no token.
Refused Planner.plan: malformed Authorization header.
Refused Planner.plan: token not accepted (the validator exited with code 1).
Refused Planner.plan: validator unavailable (it did not answer in 5 seconds).
Refused Admin.delete: the request that reached Planner did not carry an accepted token.
Warning: protected messages are served over plain http on this computer only. The token is not encrypted. ...
Note: metagente.toml changed [auth] (or [version]). I keep serving as I started; restart `metagente serve` ...
```

The token is never printed, and neither is anything the validator said.

## 13. Before you put a protected agent in front of real users

- [ ] The validator was tested by hand with a good token **and** a bad token, and the bad one exits non-zero.
- [ ] `[version] minimum = "0.1.4"` is set.
- [ ] `metagente check` says no problems, and shows no warning about the validator program.
- [ ] Served with `tls_cert` and `tls_key` (or behind a reverse proxy that ends https on this computer).
- [ ] `protect` lists every message that must not be public; try each one without a token and see the 401.
- [ ] `forward` lists only the agents and tools you trust with the caller's token.
- [ ] `cache_seconds` is a revocation delay you can accept.
- [ ] The validator reads its settings from a file next to it, not from environment variables.
- [ ] You know who is identified downstream, and that the downstream tools do their own filtering.
- [ ] You tried the "break it on purpose" steps of [multi-agent-samples/auth-chain](../multi-agent-samples/auth-chain/README.md).

## 14. If you run it on Windows (help wanted)

Nobody has recorded a Windows run yet. If you try it, please run the sample in `multi-agent-samples/auth-chain` on a
Windows computer, with the validator as it is (`python validator.py`; change `python3` to `python` in `metagente.toml`),
and open an issue saying whether the good token was accepted, the bad one refused, and the Python version you used. If
Python does not start in the clean environment (only `PATH` and `SystemRoot`), write a small `validate.cmd` that sets
what your program needs and then starts it, and use `validator = "validate.cmd"`.
