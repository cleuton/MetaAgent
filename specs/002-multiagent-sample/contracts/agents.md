# Contract: the two agents

## Concierge
- Message: `brief city`
- Run: `metagente run concierge.ag city=Lisbon` (or `metagente run concierge.ag brief city=Lisbon`)
- Declares: one remote agent, `Researcher`, at `http://127.0.0.1:8080`. No tool, no link.
- Uses the model with `think`. No provider or model is named in the file.
- Size: fewer than 10 lines.

## Researcher
- Message: `research city`
- Serve: `metagente serve researcher.ag` (port from `metagente.toml`, local only)
- Declares: MCP server `fetch` (command `uvx mcp-server-fetch`) and `tool clock`. Nothing else.
- Uses the model with `think`; the model may use only `fetch` and `clock`.
- Size: fewer than 20 lines.

## What the Researcher publishes over A2A
- Agent Card: `http://127.0.0.1:8080/.well-known/agent-card.json`, one skill, `research`.
- Task: `SendMessage` with text `research city=Lisbon` (or a data part `{"skill":"research","arguments":{"city":"Lisbon"}}`).
- Reply: task `TASK_STATE_COMPLETED` with one artifact whose text is the facts and the checked time.

## MCP tool used
- Server: the reference `fetch` server, started as `uvx mcp-server-fetch` (stdio).
- Tool: `fetch`, with a required `url` and optional `max_length`, `start_index`, `raw`. It obeys `robots.txt`, so the sample reads the Wikipedia article page (`/wiki/{city}`), not the API path.

## Not allowed in the sample
Any tool other than those above, any provider or model name in an `.ag` file, any key in any file,
`--public`, and any syntax not in the tutorial or `docs/guide.md`.
