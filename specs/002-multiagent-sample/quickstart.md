# Quickstart: Validating the City Briefing sample

Prerequisites: the `metagente` binary (`cargo build --release`), `uv` (https://docs.astral.sh/uv/),
and `export ANTHROPIC_API_KEY=...` in both terminals. Work from `samples/city-briefing/`.

## 1. Offline flow (no key, no network)

```bash
cargo test --test integration sample_city_briefing
```
Expected: passes. It runs both agents against a fake model and a fake MCP server, and checks the file
sizes, that no key is in any file, and the researcher-down message.

## 2. The real demo (US1, US2, US3, SC-001)

```bash
# terminal 1
metagente serve researcher.ag
# terminal 2
metagente run concierge.ag city=Lisbon
```
Expected: about three lines of friendly briefing for Lisbon. Time it from the first README command:
under 10 minutes.

## 3. Researcher alone (US3)

```bash
metagente run researcher.ag city=Lisbon
```
Expected: three sentences of facts followed by `(checked <time>)`.

## 4. Errors are readable (US1-3, US2-2, US3-2, edge cases)

- Unset the key and run the Concierge: the message names `ANTHROPIC_API_KEY`.
- Stop terminal 1 and run the Concierge: the message names `Researcher` and the address, and points to
  `metagente serve` (SC-003).
- Run `metagente run researcher.ag city=` : "I need a city" with the line.
- Ask for a place with no page (`city=Xyzzyplugh`): the reply says no facts were found.
- Start a second `metagente serve researcher.ag`: the message says the port is busy and to choose another.

## 5. External A2A client (US2, SC-004)

```bash
curl http://127.0.0.1:8080/.well-known/agent-card.json
curl -X POST http://127.0.0.1:8080/a2a -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"SendMessage","params":{"message":{"messageId":"m1","role":"ROLE_USER","parts":[{"text":"research city=Lisbon"}]}}}'
```
Expected: the card lists the `research` skill; the second call returns a completed task with the facts.

## 6. One line change of model (SC-005)

Change `model` in `metagente.toml`, repeat step 2: both agents use the new model, and no `.ag` file was
edited. (Check that both agents really switched by asking a model name that does not exist: both fail
with the provider's "model not found" message.)

## 7. Local only (US2-3, SAM-009)

From another computer, `curl http://<this computer>:8080/.well-known/agent-card.json` is refused.
