# Contract: Configuration

Provider and runtime settings live outside agent code (FR-017).

## `metagente.toml` (project root, optional)

```toml
[llm]
provider = "anthropic"        # or "openai-compatible"
model = "..."                 # provider specific model id
api_key_env = "ANTHROPIC_API_KEY"   # name of the environment variable holding the key
base_url = ""                 # only for openai-compatible providers

[runtime]
timeout_seconds = 30          # default per call (FR-016)
think_max_steps = 10          # limit on model/tool steps per think (FR-020)

[serve]
a2a_port = 8080
bind = "127.0.0.1"           # local only by default (FR-019); use --public to open up
```

## Environment overrides

`METAGENTE_LLM_PROVIDER`, `METAGENTE_LLM_MODEL`, `METAGENTE_TIMEOUT_SECONDS`, `METAGENTE_THINK_MAX_STEPS`.
API keys are read only from the environment variable named in `api_key_env`, never stored in files.

## Behavior

- No LLM configured and a handler uses `think`: plain message "This agent needs a language model.
  Run `metagente new` or add an [llm] section to metagente.toml." Agents that never use `think` run
  with no LLM.
- Unknown keys in the file produce a warning naming the key, not a crash.
