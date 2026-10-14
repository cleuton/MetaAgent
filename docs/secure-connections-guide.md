# Secure connections, from zero: certificates, https and a chain of agents

This guide assumes you know nothing about certificates. By the end you will have made a certificate yourself,
started three small agents that talk to each other over **https**, seen the answer, proved that the connection
really is encrypted, and broken it on purpose to read the messages. It takes about 20 minutes.

Every command below was run on Linux, and the output shown was copied from that run. Where something was not run
on your system it says so (see [Test status](#test-status)).

1. [What you will build](#1-what-you-will-build)
2. [The words you need](#2-the-words-you-need)
3. [Before you start](#3-before-you-start)
4. [Create the certificate](#4-create-the-certificate)
5. [Tell Metagente about the certificate](#5-tell-metagente-about-the-certificate)
6. [Start the tool (terminal 1)](#6-start-the-tool-terminal-1)
7. [Start the Researcher (terminal 2)](#7-start-the-researcher-terminal-2)
8. [Run the Planner (terminal 3)](#8-run-the-planner-terminal-3)
9. [Prove that it is really https](#9-prove-that-it-is-really-https)
10. [Break it on purpose](#10-break-it-on-purpose)
11. [Common problems](#11-common-problems)
12. [Renew and clean up](#12-renew-and-clean-up)
13. [Safety rules](#13-safety-rules)
14. [Where next](#14-where-next)

## 1. What you will build

Three agents in the folder [`multi-agent-samples/https-chain`](../multi-agent-samples/https-chain/README.md), and one
certificate:

```text
 Planner  --- https --->  Researcher  --- https --->  Facts
 (you run it)             (a server, port 8443)       (a tool server, port 9443)
```

- The **Planner** is started with `metagente run`. It is never served. It asks the Researcher a question.
- The **Researcher** is a server on port 8443. It answers the Planner, and it asks the tool.
- **Facts** is the tool, on port 9443. It is an ordinary Metagente agent served with `--mcp`, so each message it accepts
  is a tool that other agents can call (see [An agent as a tool](tutorial.md#an-agent-as-a-tool)).

All of this runs on your own computer. No account and no model key are needed.

## 2. The words you need

- **https**: the normal web protocol (http) with the conversation encrypted, so nobody between the two computers can
  read or change it.
- **TLS**: the encryption that https uses. You will see the word in other documents; Metagente's messages avoid it.
- **Certificate**: a small file that says "this server is called *this name*, and here is its public key". The server
  shows it to every visitor.
- **Private key**: a secret file that matches the certificate. Only the server has it. If someone else gets it, they can
  pretend to be your server. Never share it, never commit it.
- **Certificate authority**: an organisation that everybody trusts and that signs certificates, so that a visitor can
  believe them. Public web sites use certificates signed by one.
- **Self-signed certificate**: a certificate that nobody but its owner signed. Nobody trusts it automatically, so you
  must tell each visitor "I trust this one". That is fine for development on your own computer, and it is what this
  guide uses.
- **The name inside a certificate**: the host names and addresses the certificate is valid for. If the address says
  `127.0.0.1` but the certificate only names `localhost`, the visitor refuses it. Yours will name both.

## 3. Before you start

You need:

- **`metagente`**, version 0.1.3 or newer. Check with `metagente --version`.
- **`openssl`**, to make the certificate:
  - Linux: it is already installed. Check with `openssl version`.
  - macOS: the `openssl` that comes with the system is too old for the command below. Run `brew install openssl` and
    use `$(brew --prefix openssl)/bin/openssl` wherever this guide says `openssl`.
  - Windows: run `winget install ShiningLight.OpenSSL.Light`, or open "Git Bash" (it comes with Git for Windows and has
    `openssl`).
- **Three terminals**, all in the folder `multi-agent-samples/https-chain`.
- **Free ports 8443 and 9443.** To see what uses a port: `ss -ltnp | grep 8443` (Linux), `lsof -i :8443` (macOS),
  `netstat -ano | findstr 8443` (Windows). If something uses it, pick other ports, and change them in the two `serve`
  commands **and** in the two addresses (`planner.ag` and `researcher.ag`).

## 4. Create the certificate

In the first terminal, in `multi-agent-samples/https-chain`, make a folder for it and create the pair of files:

```text
mkdir certs
openssl req -x509 -newkey rsa:2048 -nodes -days 365 -subj "/CN=localhost" -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" -addext "basicConstraints=critical,CA:FALSE" -keyout certs/dev-key.pem -out certs/dev-cert.pem
```

On Windows PowerShell keep the whole `openssl` command on one line, as shown. Nothing is printed except some dots and
plus signs while it works.

What every part means:

| Part | Meaning |
|------|---------|
| `req -x509` | Make a certificate that signs itself (self-signed) |
| `-newkey rsa:2048 -nodes` | Make a new 2048 bit key, and do not protect the key file with a password (it is only for development) |
| `-days 365` | Valid for one year |
| `-subj "/CN=localhost"` | The main name of the certificate |
| `-addext "subjectAltName=DNS:localhost,IP:127.0.0.1"` | **The names it is valid for: `localhost` and `127.0.0.1`.** This is what visitors really check |
| `-addext "basicConstraints=critical,CA:FALSE"` | Say "this certificate is for a server, not for signing other certificates". **Keep this part:** without it Metagente refuses the certificate when you trust it with `ca_file` |
| `-keyout` / `-out` | Where the private key and the certificate are written |

Look at what was made:

```text
ls -l certs
```

```text
total 8
-rw-rw-r-- 1 you you 1147 Oct 13 15:18 dev-cert.pem
-rw------- 1 you you 1704 Oct 13 15:18 dev-key.pem
```

Notice that the key (`dev-key.pem`) can be read only by you (`-rw-------`). Keep it that way. Now ask openssl what is
inside the certificate:

```text
openssl x509 -in certs/dev-cert.pem -noout -subject -issuer -dates
openssl x509 -in certs/dev-cert.pem -noout -ext subjectAltName
openssl x509 -in certs/dev-cert.pem -noout -ext basicConstraints
```

```text
subject=CN=localhost
issuer=CN=localhost
notBefore=Oct 13 18:18:12 2026 GMT
notAfter=Oct 13 18:18:12 2027 GMT
X509v3 Subject Alternative Name:
    DNS:localhost, IP Address:127.0.0.1
X509v3 Basic Constraints: critical
    CA:FALSE
```

What to look for:

- `subject` and `issuer` are the same: that is what **self-signed** means.
- `notBefore` and `notAfter`: the dates between which it is valid (one year).
- **Both names** are there: `DNS:localhost` and `IP Address:127.0.0.1`.
- `CA:FALSE`.

On macOS and Windows the key file has different permissions. On macOS run `chmod 600 certs/dev-key.pem`. On Windows
the file is private to your user in your profile folder; do not share the folder or copy the file elsewhere.

## 5. Tell Metagente about the certificate

Open `multi-agent-samples/https-chain/metagente.toml`. It is already written, and every line has a comment:

```toml
[secure]
agents = ["Planner", "Researcher", "Facts"]
tools = ["kb"]

[serve]
tls_cert = "certs/dev-cert.pem"
tls_key = "certs/dev-key.pem"

[network]
ca_file = "certs/dev-cert.pem"        # preferred: trust this one certificate and keep checking it
# allow_self_signed = true            # the other way, for development only
# self_signed_hosts = ["127.0.0.1"]   # ... and only for these hosts
```

It says three different things:

1. **`[serve]`: what a server shows.** `tls_cert` and `tls_key` are the certificate and the key that
   `metagente serve` presents. With them, a served agent answers **https only**: it does not answer plain http at all.
2. **`[network]`: what a caller trusts.** Your certificate is self-signed, so the callers must be told to trust it:
   - **`ca_file`** (preferred): "trust this certificate, and keep checking everything else". Metagente still checks
     that the name in the address is inside the certificate.
   - **`allow_self_signed`** (development only): "do not check certificates at all" for the hosts you list. It works with
     any certificate, but it switches the protection off, so Metagente prints a warning every time:

   ```text
   Warning: certificate checks are OFF for agents and tools for 127.0.0.1 (allow_self_signed = true in metagente.toml). Use this for development only.
   ```

3. **`[secure]`: what MUST use https.** It lists the agents and the tool by name. Calling any of them over plain
   `http://` is refused before anything is sent, and a listed agent refuses to be served without a certificate. This is
   separate from trust: `allow_self_signed` never makes `http://` acceptable for a listed name.

Nothing in the three `.ag` files says "secure". The only thing there is the address, which starts with `https://`.
Check the files (this also proves the certificate paths are right):

```text
metagente check planner.ag
```

```text
No problems found in planner.ag (1 agent).
```

## 6. Start the tool (terminal 1)

```text
metagente serve facts.ag --mcp 9443
```

```text
Serving 1 agent (Facts).
MCP: https://127.0.0.1:9443/mcp
```

The address starts with `https://`: it is serving over https. Leave this terminal running. Always wait for the
`MCP:` line before you start the next step.

## 7. Start the Researcher (terminal 2)

```text
metagente serve researcher.ag --a2a 8443
```

```text
Serving 1 agent (Researcher).
A2A: https://127.0.0.1:8443   (agent card: https://127.0.0.1:8443/.well-known/agent-card.json)
```

Leave it running. Start the tool first and the Researcher second: the Researcher only needs the tool when it is asked.

## 8. Run the Planner (terminal 3)

```text
metagente run planner.ag plan topic=rust
```

```text
Planner got: Rust is a systems language
```

That answer travelled over two https connections: Planner to Researcher, and Researcher to Facts. The Planner needed no
certificate of its own, because it is only run and never served. It only needed `ca_file`, to trust the certificate of
the servers it calls.

## 9. Prove that it is really https

**Ask for the agent's card, trusting the certificate:**

```text
curl -s --cacert certs/dev-cert.pem https://localhost:8443/.well-known/agent-card.json
```

It prints the Agent Card, a JSON text. Every address inside it starts with `https://`, for example:

```text
"url":"https://localhost:8443/a2a"
```

**Ask in plain http.** This must not work:

```text
curl -sS http://127.0.0.1:8443/.well-known/agent-card.json
```

```text
curl: (1) Received HTTP/0.9 when not allowed
```

The server does not speak http on that port: all curl got back was encrypted noise. (Port 9443 behaves the same.)

**Ask in https without trusting the certificate.** This must not work either, because the certificate is self-signed:

```text
curl -sS https://localhost:8443/.well-known/agent-card.json
```

```text
curl: (60) SSL certificate problem: self-signed certificate
```

**Look at the encryption itself:**

```text
echo | openssl s_client -connect localhost:8443 -CAfile certs/dev-cert.pem -verify_return_error 2>/dev/null | grep -E "Verification|Verify return"
```

```text
Verification: OK
Verify return code: 0 (ok)
```

Replace `localhost:8443` with `localhost:9443` to check the tool in the same way.

## 10. Break it on purpose

Do each experiment, read the message, then put the file back. The messages are what Metagente really prints.

**1. Use http instead of https.** In `planner.ag` change `https://127.0.0.1:8443` to `http://127.0.0.1:8443`:

```text
Problem on line 4 of planner.ag: Researcher is declared secure in metagente.toml but this address uses http: http://127.0.0.1:8443
  4 |   remote Researcher at "http://127.0.0.1:8443"
    |   ^
Fix: write https:// instead of http://, or remove Researcher from [secure] agents
```

It stops before anything is sent: plain http can never succeed here.

**2. Stop trusting the certificate.** In `metagente.toml` put a `#` in front of the `ca_file` line, then run the Planner:

```text
Problem on line 7 of planner.ag: I could not trust the certificate of https://127.0.0.1:8443/.well-known/agent-card.json (remote agent Researcher): it was signed by an authority I do not know, for example a self-signed certificate
  7 |     answer = Researcher.research topic: topic
    |              ^
Fix: add that authority with ca_file in [network]; for development only you can set allow_self_signed = true
```

Put the `#` back out again, or try the other way: replace the `ca_file` line with `allow_self_signed = true` and
`self_signed_hosts = ["127.0.0.1"]`. The chain works again, and the warning from chapter 5 appears.

**3. A certificate for the wrong name.** Make a certificate that only names `localhost` (the same command, with
`-addext "subjectAltName=DNS:localhost"`), put it in `certs/`, restart the servers and run the Planner, whose address says
`127.0.0.1`:

```text
Problem on line 7 of planner.ag: the certificate of https://127.0.0.1:8443/.well-known/agent-card.json is for localhost, but the address says 127.0.0.1
  7 |     answer = Researcher.research topic: topic
    |              ^
Fix: use the name that is on the certificate, or for development only set allow_self_signed = true in [network]
```

The fix is to make the certificate again with both names, as in chapter 4.

**4. Start a secure agent without a certificate.** Delete the two lines `tls_cert` and `tls_key` from `[serve]`, then start
the Researcher:

```text
Researcher is declared secure, so I will only serve it over https, but no certificate is set.
Fix: set tls_cert and tls_key in [serve] of metagente.toml, or use --tls-cert and --tls-key
```

It does not start, and it never falls back to http.

**5. Stop the tool server.** Stop terminal 1 (Ctrl-C) and run the Planner:

```text
Problem on line 7 of planner.ag: agent Researcher could not answer `research`
  7 |     answer = Researcher.research topic: topic
    |              ^
    Problem on line 7 of researcher.ag: I could not reach https://127.0.0.1:9443/mcp (tool server https://127.0.0.1:9443/mcp): the connection failed
      7 |     fact = kb.Facts.lookup topic: topic
        |            ^
    Fix: check the address and that the server is running
```

The Researcher says it could not reach the tool, and the Planner passes the message on. Stop the Researcher too and
you get this from the Planner:

```text
Problem on line 7 of planner.ag: I could not reach https://127.0.0.1:8443/.well-known/agent-card.json (remote agent Researcher): the connection failed
  7 |     answer = Researcher.research topic: topic
    |              ^
Fix: check the address, and that the other agent is being served (metagente serve)
```

## 11. Common problems

| What you see | What it means | What to do |
|--------------|---------------|------------|
| `I could not read the certificate file ... No such file or directory` | The certificate does not exist yet, or you are in another folder | Do chapter 4, and run every command in `multi-agent-samples/https-chain` |
| `I could not trust the certificate ... signed by an authority I do not know` | The caller was not told to trust the certificate | Check the `ca_file` line of `metagente.toml` (chapter 5) |
| `ca_file is set but did not make this certificate trusted` | The certificate was made without `CA:FALSE` | Make it again with the command of chapter 4 |
| `the certificate of ... is for localhost, but the address says 127.0.0.1` | The name in the address is not inside the certificate | Make the certificate again with both names (chapter 4) |
| `I could not listen for A2A on 127.0.0.1:8443 ... Address already in use` | Another program uses the port | Chapter 3: find it, or use other ports in both places |
| `I could not reach https://... the connection failed` | The server you call is not running (yet) | Start the servers in the order of chapters 6 and 7, and wait for the `MCP:` and `A2A:` lines |
| `the certificate in ... expired on ...` | The certificate is older than its days | Chapter 12 |
| `Warning: The certificate in ... expires on ...` | Less than 30 days are left | Chapter 12, when you can |
| `curl: (60) SSL certificate problem: self-signed certificate` | curl was not told to trust the certificate | Add `--cacert certs/dev-cert.pem`, as in chapter 9 |

The wait matters: a slow first start of a server must not make the Planner give up, so run the Planner only after the
`MCP:` and `A2A:` lines have appeared.

## 12. Renew and clean up

A development certificate lasts one year. To renew it: stop the servers, delete the `certs` folder, run the commands of
chapter 4 again, and start the servers again. Nothing else changes.

To clean up, stop each server with Ctrl-C and delete the `certs` folder.

## 13. Safety rules

- **This is for development.** A self-signed certificate on your own computer is fine. A server that other people use
  needs a certificate from a public certificate authority (your hosting provider or a free service such as Let's Encrypt
  will explain how). Metagente uses such a certificate in the same two settings, `tls_cert` and `tls_key`, and callers
  need no `ca_file` for it.
- **Never commit a private key.** The sample's `.gitignore` already excludes `certs/`. If a key was committed by
  mistake, treat it as public: make a new certificate and key (chapter 12), and remove the old one from the history.
- **The key is a secret.** Keep `dev-key.pem` readable by you only, and never paste it into a chat or an issue.
- **`allow_self_signed` switches the protection off.** Use it only for hosts you control, with `self_signed_hosts`.
- **Https is not a login.** It protects the connection, not who is calling. There is still no login for served agents in
  this version, and serving stays limited to your own computer unless you add `--public`.

## 14. Where next

- The sample and its README: [multi-agent-samples/https-chain](../multi-agent-samples/https-chain/README.md)
- Every key of `[secure]`, `[serve]` and `[network]`, and all the messages: the
  [README](../README.md#new-in-013-secure-connections-https-on-both-ends)
- Using an agent as a tool: [tutorial.md](tutorial.md#an-agent-as-a-tool)
- The words of the language: [syntax.md](syntax.md#secure-connections), and the [programming guide](guide.md)

## Test status

| Part | Verified? | How |
|------|-----------|-----|
| Every command and every output of this guide on Linux | Yes | Run by hand on Linux, with ports 18443 and 19443 because 8443 and 9443 were in use on that computer. The commands are otherwise the same |
| The certificate command, the three servers, the answer and the messages of chapter 10 | Yes, on every build | The automated test `https_chain` makes the certificate with the command of the sample README, runs the chain and compares the messages |
| macOS: `brew install openssl`, `chmod 600` | Not verified on this system | Taken from the documentation of those tools |
| Windows: `winget`, Git Bash, PowerShell, port commands | Not verified on this system | Taken from the documentation of those tools |
