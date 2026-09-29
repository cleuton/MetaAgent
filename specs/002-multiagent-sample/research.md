# Phase 0 Research: Multi-Agent Sample (City Briefing)

The spec has no open clarifications. These are the decisions, each checked against the built
interpreter where possible (checks run 2026-09-29).

## D1. Where the MCP server and the remote address are declared

- **Decision**: in the agent files (`tool fetch from mcp "uvx mcp-server-fetch"` in `researcher.ag`,
  `remote Researcher at "http://127.0.0.1:8080"` in `concierge.ag`). `metagente.toml` holds the model,
  the key variable name, timeouts and the port.
- **Rationale**: this is how the language works (spec 001, tutorial steps 6 and 9). The spec input asked
  for the toml, and the spec was corrected; extending the language is out of scope (SAM-010).
- **Alternatives**: a `[mcp]`/`[remote]` toml section (rejected: a language change, not allowed here).

## D2. The Researcher lets the model choose what to fetch

- **Decision**: `facts = think "Use the fetch tool to read <page for {city}> and write three short
  sentences ..."`. The model calls `fetch`; the clock is read by the agent itself.
- **Rationale**: it makes the edge case "the page does not exist" work by construction: a failed
  fetch is handed back to the model, which then says no facts were found (spec edge case 2). It also
  shows `think` using a declared tool, which is the point of a model agent.
- **Alternatives**: call `fetch.fetch url: ...` directly (deterministic, but a failed fetch would stop
  the agent with an error instead of a friendly "no facts found").
- **Check**: `uvx mcp-server-fetch` was called through Metagente with `fetch.fetch url: ...
  max_length: "300"` and returned the page text (the text value `"300"` was accepted as a number by
  the argument check). The first run takes about 12 s while `uv` downloads packages; later runs are fast.

## D3. The page to read

- **Decision**: the Wikipedia article, `https://en.wikipedia.org/wiki/{city}`. The reference fetch server
  returns the first 5000 characters of it as text (the summary paragraph and the fact box), which is
  plenty for three sentences.
- **Rationale**: it works with the real fetch server, which respects `robots.txt` and refuses pages
  that site owners closed to automatic readers.
- **Alternatives**: the Wikipedia summary API (`/api/rest_v1/page/summary/{city}`, small JSON). **Rejected
  after a run against the real server (2026-09-29)**: Wikipedia's `robots.txt` forbids automated fetching
  of that path, so the server answers "autonomous fetching of this page is not allowed" for every city and
  the demo would report "no facts found" every time. The offline tests could not show this because their
  fake server has no such rule. Using `--ignore-robots-txt` was rejected: a sample should not teach
  bypassing a site's rules.
- **Note**: a city name with spaces is put in the address by the model; the README uses one word
  cities in its example.

## D4. Concierge composition

- **Decision**: `facts = Researcher.research city: city`, then `reply think "Write a friendly briefing
  of three lines ... using only these facts: {facts}"`.
- **Rationale**: two lines that read aloud as what they do; "using only these facts" keeps the model
  from inventing.
- **Note**: the concierge's `think` can also see the Researcher as a tool (its Agent Card is fetched
  when needed). That is harmless and the prompt tells it to use the facts it already has.

## D5. How the demo is started

- **Decision**: terminal 1 serves the Researcher (`metagente serve researcher.ag`), terminal 2 runs the
  Concierge once per question (`metagente run concierge.ag city=Lisbon`). Both run from inside
  `samples/city-briefing/`, where `metagente.toml` is found. The A2A port comes from the toml
  (`a2a_port = 8080`), so no flag is needed.
- **Rationale**: the shortest commands that still show two processes and A2A. With one accepted
  message the message name may be left out of `run`.
- **Alternatives**: serving the Concierge too (extra terminal, nothing gained for this demo).

## D6. Timeouts

- **Decision**: `timeout_seconds = 90` in the sample toml.
- **Rationale**: the Concierge's call to the Researcher includes a model round trip, a page fetch and a
  second model round trip; the default 30 s can be too tight. The same setting bounds the fetch and the
  model calls in the Researcher.
- **Alternatives**: `within 90 seconds` on the call in `concierge.ag` (works, but adds an extra phrase to the agent).

## D7. The key

- **Decision**: `api_key_env = "ANTHROPIC_API_KEY"` and the README says to `export` it in **both**
  terminals (both processes call the model). No key in any file (SAM-005). When it is missing the
  interpreter says "the variable ANTHROPIC_API_KEY is not set".
- **Alternatives**: a `.env` file (would put a key in a file; not supported by the interpreter).

## D8. Offline test of the whole flow

- **Decision**: one integration test that starts the existing in-process fake MCP server (with a `fetch`
  tool) and a mock Anthropic endpoint (canned answers: a `fetch` tool call, then facts, then the
  briefing), serves `researcher.ag` over the in-process A2A server, and runs `concierge.ag`. It replaces
  three strings in copies of the real files: the MCP command, the Researcher address, and `base_url`
  in the toml. It also checks the line counts (SC-002), that no file contains a key (SAM-005), that
  only the declared tools appear, and the researcher-down message (SC-003).
- **Rationale**: keeps the sample from rotting, needs no network, no key and no `uv`.
- **Alternatives**: rely on manual runs only (rots silently).

## D9. What is verified by hand with the real services

- **Decision**: at implementation, run the demo once with the real key and the real fetch server and
  record the result in `samples/city-briefing/README.md` ("Tested on ...") and in the feature
  quickstart. This is the only step that spends model credits and needs the network.

## D10. The A2A check from outside

- **Decision**: the README shows `curl` for the Agent Card and for one `SendMessage`, using the
  same shapes the interoperability test already sends, so a reader can see the Researcher answer
  without the Concierge.
