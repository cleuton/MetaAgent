# Samples

Small projects that show Metagente doing real work. Each one is a folder you can open, read, and run
on its own; the README inside says exactly how.

| Sample | What it shows |
|--------|---------------|
| [city-briefing](city-briefing/) | Two agents working together. A **Concierge** asks a **Researcher** over A2A; the Researcher reads a web page through an MCP tool, and both use Claude to write. |
| [recruiter](metagente-demo-recrutador/) | 3 agents selecting candidates for a job |

To start, build Metagente once from the project folder (`cargo build --release`), then open the
sample's README.

Please read the `metagente.toml` file and create the necessary environment variable.
