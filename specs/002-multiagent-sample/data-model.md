# Data Model: Multi-Agent Sample (City Briefing)

No stored data. These are the things the sample is made of and what passes between them.

## Concierge (`concierge.ag`)
| Part | Value |
|------|-------|
| goal | Give a visitor a short briefing about a city |
| declares | `remote Researcher at "http://127.0.0.1:8080"` (nothing else) |
| accepts | `brief city` |
| does | asks `Researcher.research city: city`, then `think`s a three line briefing from the facts |
| returns | the briefing text |

## Researcher (`researcher.ag`)
| Part | Value |
|------|-------|
| goal | Find facts about a city and summarize them |
| declares | `tool fetch from mcp "uvx mcp-server-fetch"` and `tool clock` (nothing else) |
| accepts | `research city` |
| does | stops with "I need a city" if empty; reads the clock; `think`s three sentences of facts, using `fetch` |
| returns | the facts followed by `(checked <time>)` |

## metagente.toml (in `samples/city-briefing/`)
| Section | Key | Value |
|---------|-----|-------|
| `[llm]` | `provider` | `anthropic` |
| `[llm]` | `model` | `claude-sonnet-5-5` |
| `[llm]` | `api_key_env` | `ANTHROPIC_API_KEY` (the variable's name, never a key) |
| `[runtime]` | `timeout_seconds` | `90` |
| `[runtime]` | `think_max_steps` | `10` |
| `[serve]` | `a2a_port` | `8080` |
| `[serve]` | `bind` | `127.0.0.1` |

## Flow of one question
```text
person --run--> Concierge --A2A SendMessage (skill research, city)--> Researcher
                                                                        |-- MCP call: fetch (uvx mcp-server-fetch)
                                                                        |-- clock.now
                                                                        |-- model: facts
                 Concierge <----- task COMPLETED, artifact "reply" ---- Researcher
person <--------- briefing (model: three lines from the facts) -- Concierge
```

## States and failures
| Situation | Where it shows | Message comes from |
|-----------|----------------|--------------------|
| Key variable not set | either agent | interpreter: the variable is not set |
| Researcher not running | Concierge | interpreter: could not reach the address; points to `metagente serve` |
| `uvx` missing | Researcher | interpreter: could not start the tool server, check it is installed |
| City empty | Researcher | the agent's own `fail "I need a city"` |
| Page missing | Researcher | the model says no facts were found |
| Model loops on tools | either agent | interpreter: step limit message |
| Port 8080 busy | serve | interpreter: could not listen; choose another port |
