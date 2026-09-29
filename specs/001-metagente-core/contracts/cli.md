# Contract: Command line

Binary name: `metagente`.

| Command | Behavior | Exit |
|---------|----------|------|
| `metagente run FILE.ag MESSAGE [key=value ...] [--agent NAME]` | Runs the agent (first in file by default), sends it `MESSAGE` with the given parameters, prints the reply. Example: `metagente run weather.ag ask city=Lisbon` | 0 ok, 1 agent/user error, 2 usage |
| `metagente serve FILE.ag [--a2a PORT] [--mcp stdio\|PORT] [--public]` | Keeps agents running; publishes Agent Cards, accepts A2A tasks, exposes agents as MCP tools. Listens on 127.0.0.1 only unless `--public` is given, which prints a warning that there is no authentication | runs until Ctrl-C |
| `metagente check FILE.ag` | Parses and validates only: syntax, declarations, permissions, links, interfaces. Runs nothing | 0 ok, 1 problems found |
| `metagente new NAME` | Creates a starter agent file and a `metagente.toml` | 0 |
| `metagente --version`, `--help` | Standard | 0 |

Rules:
- Output for users is plain text on stdout; problems go to stderr in the Diagnostic format.
- No Rust panic text or backtrace is ever printed. Internal failures print one plain sentence and the
  path of a log file.
- `run` requires no separate compile step (FR-001).
