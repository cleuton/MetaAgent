# Contract: `samples/city-briefing/metagente.toml`

```toml
# Settings for the City Briefing demo. The agents never mention any of this.

[llm]
provider = "anthropic"
model = "claude-sonnet-5-5"          # change only this line to use another model
api_key_env = "ANTHROPIC_API_KEY"    # the NAME of the variable that holds your key

[runtime]
timeout_seconds = 90                 # a briefing needs a page fetch and two model answers
think_max_steps = 10

[serve]
a2a_port = 8080
bind = "127.0.0.1"                   # this computer only
```

Rules: no key in the file; no other sections; each setting has a comment saying what changing it does;
changing `model` changes the model of both agents (SC-005).
