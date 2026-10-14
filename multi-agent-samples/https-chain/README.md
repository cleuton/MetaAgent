# HTTPS chain: an agent calls an agent, which calls a tool, all over https

Three small Metagente agents, one certificate, three terminals, and no language model key. You will see
the whole chain work over https, and then you will break it on purpose to see the plain messages.

```text
 Planner  --- https (A2A) --->  Researcher  --- https (MCP) --->  Facts
 (metagente run)                (metagente serve)                 (metagente serve)
```

* `planner.ag`: asks the Researcher about a topic.
* `researcher.ag`: looks the fact up with a tool called `kb`.
* `facts.ag`: the tool. It is a Metagente agent, served over MCP.
* `metagente.toml`: says that all three agents and the `kb` tool MUST use https, where the certificate and key
  are, and which certificate to trust.

Nothing in the three `.ag` files says "secure". The only thing that asks for https there is the address that
starts with `https://`. The rule that it MUST be https is in `metagente.toml`.

New to certificates? The [secure connections guide](../../docs/secure-connections-guide.md) teaches them from zero and
walks through this same chain, chapter by chapter.

## Before you start

You need `metagente` (version 0.1.3 or newer) and `openssl`.

* **Linux**: `openssl` is already there.
* **macOS**: the `openssl` that comes with the system is too old for the command below. Run
  `brew install openssl` and use `$(brew --prefix openssl)/bin/openssl` instead of `openssl`.
* **Windows**: run `winget install ShiningLight.OpenSSL.Light`, or use the `openssl` that comes with Git for
  Windows (open "Git Bash"). In PowerShell write `mkdir certs` as shown, and keep each command on one line.

Open a terminal in this folder (`multi-agent-samples/https-chain`).

## Step 1: make a certificate for your computer

The certificate is made here, on your computer, and it is never committed (`certs/` is in `.gitignore`).
It is valid for `localhost` and `127.0.0.1`.

```text
mkdir certs
openssl req -x509 -newkey rsa:2048 -nodes -days 365 -subj "/CN=localhost" -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" -addext "basicConstraints=critical,CA:FALSE" -keyout certs/dev-key.pem -out certs/dev-cert.pem
```

It makes `certs/dev-cert.pem` (the certificate) and `certs/dev-key.pem` (the private key: keep it to yourself).

## Step 2: start the tool (terminal 1)

```text
metagente serve facts.ag --mcp 9443
```

It prints `MCP: https://127.0.0.1:9443/mcp`. Leave it running.

## Step 3: start the Researcher (terminal 2)

```text
metagente serve researcher.ag --a2a 8443
```

It prints `A2A: https://127.0.0.1:8443 ...`. Leave it running.

## Step 4: ask the Planner (terminal 3)

```text
metagente run planner.ag plan topic=rust
```

You should see:

```text
Planner got: Rust is a systems language
```

The Planner reached the Researcher over https, and the Researcher reached the `kb` tool over https. Both servers
answer https only: a plain `http://` request to port 8443 or 9443 gets no answer at all.

## Two ways to trust the certificate

The certificate is self-signed, so the computer that calls it must be told to trust it. `metagente.toml` has both ways.

1. **Preferred: `ca_file`.** `ca_file = "certs/dev-cert.pem"` trusts that one certificate and keeps checking
   everything else, including the name. This is what the sample uses.
2. **Development only: `allow_self_signed`.** Replace the `ca_file` line with
   `allow_self_signed = true` and `self_signed_hosts = ["127.0.0.1"]`. Certificate checks are off for
   `127.0.0.1`, and every `run`, `serve` and `check` prints a warning that says so:

```text
Warning: certificate checks are OFF for agents and tools for 127.0.0.1 (allow_self_signed = true in metagente.toml). Use this for development only.
```

Saying that something is secure and trusting a certificate are two separate settings. `allow_self_signed`
never makes http acceptable for something listed in `[secure]`.

## Break it on purpose

Try each of these and compare with what you see. Put the file back after each one.

**1. Use http instead of https.** In `planner.ag`, change `https://127.0.0.1:8443` to `http://127.0.0.1:8443`:

```text
Problem on line 4 of planner.ag: Researcher is declared secure in metagente.toml but this address uses http: http://127.0.0.1:8443
Fix: write https:// instead of http://, or remove Researcher from [secure] agents
```

Nothing is sent to the Researcher: the run stops before any connection is opened. Do the same in
`researcher.ag` (the address of `kb`), restart the Researcher, and run the Planner again:

```text
kb is declared secure in metagente.toml but this address uses http: http://127.0.0.1:9443/mcp
Fix: write https:// instead of http://, or remove kb from [secure] tools
```

**2. Take the certificate away.** Delete the two lines `tls_cert` and `tls_key` from `[serve]` in
`metagente.toml`, and start the Researcher:

```text
Researcher is declared secure, so I will only serve it over https, but no certificate is set.
Fix: set tls_cert and tls_key in [serve] of metagente.toml, or use --tls-cert and --tls-key
```

It does not start, and it never falls back to plain http. If you delete only `tls_cert`:

```text
I have a key but no certificate (tls_cert).
Fix: add tls_cert to [serve] in metagente.toml, or use --tls-cert with the PEM file of the certificate
```

**3. Stop trusting the certificate.** Put a `#` in front of the `ca_file` line (and leave `allow_self_signed`
off), then run the Planner:

```text
I could not trust the certificate of https://127.0.0.1:8443/.well-known/agent-card.json (remote agent Researcher): it was signed by an authority I do not know, for example a self-signed certificate
Fix: add that authority with ca_file in [network]; for development only you can set allow_self_signed = true
```

**4. Use the other way to trust it.** Put `allow_self_signed = true` and `self_signed_hosts = ["127.0.0.1"]`
in `[network]` instead of `ca_file`. The chain works again, with the warning shown above.

## Common problems

| What you see | What it means | What to do |
|--------------|---------------|------------|
| `I could not read the certificate file ... No such file or directory` | Step 1 was not done, or you are in another folder | Make the certificate (Step 1) and run the commands in this folder |
| `I could not listen for A2A on 127.0.0.1:8443 ... Address already in use` | Another program uses that port | Stop it, or use other ports in `--a2a`, `--mcp` and in the two addresses |
| `the certificate of ... is for ..., but the address says ...` | The name in the address is not on the certificate | Use `127.0.0.1` or `localhost`, as in Step 1 |
| `ca_file is set but did not make this certificate trusted` | The certificate was made without `CA:FALSE` | Make it again with the command of Step 1 |
| `the certificate in ... expired` | The certificate is older than its days | Make a new one (Step 1) |
| `I could not complete a secure connection to https://...` | The port answers plain http, or nothing | Check that the server on that port was started with a certificate |
| `Warning: The certificate in ... expires on ...` | The certificate has less than 30 days left | Make a new one when you can |

## Files

| File | What it is |
|------|------------|
| `planner.ag`, `researcher.ag`, `facts.ag` | The three agents (each is under 10 lines of code) |
| `metagente.toml` | `[secure]` the names that must use https, `[serve]` the certificate and key, `[network]` what to trust |
| `certs/` | Made by Step 1 on your computer. Never committed |
