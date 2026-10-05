# Resume Screening: three agents, A2A and MCP

[**PORTUGUESE**](README_portuguese.md)

You hand over a folder of resumes and the text of a job opening. The system returns the **three best resumes for the job**, each with the candidate's name and the file name. Three Metagente agents do the work together:

```text
                                  +--A2A--> Leitor ----MCP "files"----> lists the folder
                                  |            |  \--MCP "convert"---> reads PDF, DOCX, HTML, MD, TXT, JSON
you --run--> Recrutador ----------+            |
 (folder + job)   ^                |            v digest of each resume
                  |                |
                  |                +--A2A--> Avaliador --MCP "fetch"--> Wikipedia (job technologies)
                  |                              |
                  +---------- top 3 -------------+
you <-- final list --- Recrutador
```

- The **Recrutador** (recruiter, `recrutador.ag`) is the coordinator. It receives the folder and the job, calls the other two agents over A2A and replies with the list. It only orchestrates: it does not talk to the model and does not read any file.
- The **Leitor** (reader, `leitor.ag`) is a server that stays up. It uses MCP to **access files**: it lists the folder, converts each resume (any format) to text and asks Claude for a short digest of each one.
- The **Avaliador** (evaluator, `avaliador.ag`) also stays up. It uses MCP to **search the internet**: it reads on Wikipedia what the job's technologies are, compares each candidate with the job and picks the three best.
- The model (Claude Sonnet 5.5) and the source of the key live only in `metagente.toml`. No `.ag` file mentions a provider or a model.

Each agent declares only what it needs: the Recrutador has no tool at all besides the two remote agents, the Leitor declares only file tools and the Avaliador declares only the web search tool.

## Test status (read before running)

What was and was not verified:

| Part | Verified? | How |
|------|-----------|-----|
| File MCP server (`server-filesystem`) | Yes | Actually called: `list_directory`, `list_allowed_directories`, and refusal of a path outside the folder |
| Conversion MCP server (`markitdown-mcp`) | Yes | All 9 files in the `curriculos/` folder were converted to text, including the 2 PDFs and the 2 DOCX |
| `fetch` MCP server | Partial | Starts and lists the `fetch` tool; the Wikipedia lookup **could not be tested** in this environment due to network restrictions |
| Syntax of the `.ag` files | Only by reading | Checked against `syntax.md`, `guide.md` and the City Briefing sample. **`metagente check` was not run** (the binary was not available here) |
| Running the agents with Claude, A2A between them, and the final ranking | **No** | None of this was executed. The output shown below is the **expected** result, not a recording |

That is why the "Running in stages" section proposes a ladder of tests. Start with `metagente check` and climb one rung at a time.

One specific point to watch: the Recrutador passes multi-line texts (the job and the digests) as values of an A2A call. The City Briefing sample only passes a short value (`city`). If that call fails, rung 4 below shows where.

## Prerequisites

1. **Metagente.** In the project folder (the one with `Cargo.toml`): `cargo build --release`. The program is `target/release/metagente`. The commands below say `metagente`; put that folder on your PATH or type the full path.
2. **Node.js**, because the file MCP server is started with `npx` (`npx -y @modelcontextprotocol/server-filesystem .`). Check with `npx --version`.
3. **uv**, which starts the conversion and `fetch` MCP servers (`uvx markitdown-mcp` and `uvx mcp-server-fetch`). Check with `uvx --version`. The first run downloads the packages.
4. **An Anthropic API key.** Usage is billed to your account. Screening 9 resumes makes several calls to the model (the Leitor uses one per step).
5. **Three terminals**, all open in this folder (`triagem-curriculos/`). Metagente looks for `metagente.toml` in the folder where it is started, and the file server also sees the current folder.

The `file://` paths used by the converter work on Linux and macOS. On Windows, the Leitor would need adjustments.

## Folders and files

```text
triagem-curriculos/
  README.md
  metagente.toml          configuration (model, key, timeout)
  recrutador.ag           coordinator
  leitor.ag               agent 1: files via MCP
  avaliador.ag            agent 2: internet via MCP
  vaga.txt                job text (fictional) to use in the request
  curriculos/             9 test files, all fictional
    fulano-quintanilha-brasil.pdf
    beltrano-albuquerque-sousa.docx
    cicrano-valadares.md
    fulana-mendes-tavares.txt
    beltrana-cavalcante-rosa.html
    cicrana-ferraz-lobo.pdf
    ze-fulanildo-barroso.json
    beltrano-fulanildo-neto.docx
    aviso-do-rh.txt        (not a resume, on purpose)
```

The names, companies, schools, emails and phone numbers are made up (`example.com` and phone numbers full of zeros). Any resemblance is coincidental.

## Configuration

Everything you might want to change is in `metagente.toml`:

```toml
[llm]
provider = "anthropic"
model = "claude-sonnet-5-5"          # change only this line to use another model
api_key_env = "ANTHROPIC_API_KEY"    # the NAME of the variable that holds the key

[runtime]
timeout_seconds = 180                # reading 9 resumes takes several steps with the tools
think_max_steps = 30                 # the Leitor uses 1 step to list and 1 per file

[serve]
bind = "127.0.0.1"                   # this computer only
```

The file never stores the key. Export it in both server terminals, because the Leitor and the Avaliador talk to Claude:

```bash
export ANTHROPIC_API_KEY=your-key-here
```

`think_max_steps` is set to 30 (the default is 10) because the Leitor needs one step per resume. For larger folders, raise this number and `timeout_seconds`.

Two things live inside the `.ag` files, because that is where Metagente declares them:

- Which MCP servers to start: `tool files from mcp "..."` and `tool convert from mcp "..."` in `leitor.ag`, and `tool fetch from mcp "..."` in `avaliador.ag`.
- Where the remote agents are: the two `remote ... at "http://127.0.0.1:808x"` lines in `recrutador.ag`. They must match the ports used with `serve`.

## The job and the test resumes

`vaga.txt` asks for a senior backend engineer in **Rust**: 4 or more years of backend, **2 or more years of Rust in production**, REST or gRPC, PostgreSQL, messaging (Kafka or RabbitMQ), Docker and Kubernetes. Nice to have: Tokio and Axum, OpenTelemetry and Prometheus, AWS, English.

The resumes were designed so that the right answer is defensible:

| File | Format | Profile | Expected |
|------|--------|---------|----------|
| `fulano-quintanilha-brasil.pdf` | PDF, single column | 9 years of backend, 4 of Rust in production (Tokio, Axum), Kafka, PostgreSQL, Kubernetes, OpenTelemetry, fluent English | **1st place** |
| `cicrano-valadares.md` | Markdown | 5 years of backend, 3 of Rust (Actix), RabbitMQ, PostgreSQL, Docker; Kubernetes only in staging | **Top 3** |
| `beltrano-albuquerque-sousa.docx` | Word | 7 years of backend, mostly Go, 1 year of Rust in production, gRPC, Kafka, Kubernetes | **Top 3** |
| `fulana-mendes-tavares.txt` | Plain text | 12 years of Java and Spring, Kafka, Kubernetes; Rust only as study | Out |
| `beltrano-fulanildo-neto.docx` | Word, in a table | SRE with 8 years, strong Kubernetes, no Rust and no business APIs | Out |
| `ze-fulanildo-barroso.json` | JSON (JSON Resume standard) | Junior with a Rust bootcamp and a huge list of keywords, but zero years in production | Out (the "loose keywords" test) |
| `beltrana-cavalcante-rosa.html` | HTML | Front-end with 6 years, basic backend | Out |
| `cicrana-ferraz-lobo.pdf` | PDF, two columns | Data scientist, 5 years of Python | Out |
| `aviso-do-rh.txt` | Text | Not a resume | Ignored |

First place is solid. Positions 2 and 3 (Cicrano and Beltrano Albuquerque) are an honest technical tie: one has more Rust, the other has more Kubernetes and Kafka. The set of three should be this one, but the order between them may vary from one run to the next.

## Running

**Terminal 1**: start the Leitor and leave it running.

```bash
export ANTHROPIC_API_KEY=your-key-here
metagente serve leitor.ag --a2a 8081
```

**Terminal 2**: start the Avaliador and leave it running.

```bash
export ANTHROPIC_API_KEY=your-key-here
metagente serve avaliador.ag --a2a 8082
```

**Terminal 3**: send the request to the Recrutador.

```bash
metagente run recrutador.ag find folder=resumes job="$(cat job-description.txt)"
```

`$(cat job-description.txt)` puts the whole job text into a single argument. It may take a few minutes, almost all of them with the Leitor opening the 9 files. When it finishes, stop both servers with Ctrl-C.

### Expected output (illustrative, not recorded)

```text
Os 3 melhores currículos para a vaga:
1. Fulano Quintanilha Brasil | fulano-quintanilha-brasil.pdf
2. Cicrano Valadares | cicrano-valadares.md
3. Beltrano Albuquerque Sousa | beltrano-albuquerque-sousa.docx
```

### Running in stages (recommended the first time)

Since nothing here was run end to end, climb this ladder. Each rung isolates one part:

1. **Syntax.** `metagente check leitor.ag`, `metagente check avaliador.ag` and `metagente check recrutador.ag`. Each error points to the line and says how to fix it.
2. **Leitor alone** (no A2A): `metagente run leitor.ag summarize folder=curriculos > resumos.txt`. Open `resumos.txt`: there should be 9 `ARQUIVO / NOME / RESUMO` blocks, with `aviso-do-rh.txt` marked as "não é currículo" (not a resume).
3. **Avaliador alone** (no A2A): `metagente run avaliador.ag rank job="$(cat vaga.txt)" candidates="$(cat resumos.txt)"`. It should print a 3-line list.
4. **Everything together.** Start both servers and run the Recrutador, as above. If rung 4 fails and rungs 2 and 3 passed, the problem is in the A2A call with long multi-line texts. In that case, test the Leitor over A2A with the `curl` in the next section, which has only one short value.

## Checking an agent from outside (A2A)

Any A2A client can talk to the Leitor, not only the Recrutador. With Terminal 1 running:

```bash
# Who is it and what can it do? (the agent card)
curl http://127.0.0.1:8081/.well-known/agent-card.json

# Ask for the resume digests
curl -X POST http://127.0.0.1:8081/a2a -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"SendMessage","params":{"message":{"messageId":"m1","role":"ROLE_USER","parts":[{"text":"summarize folder=curriculos"}]}}}'
```

The card should list one skill, `summarize`. The `curl` format follows the City Briefing sample.

## How it works

`recrutador.ag`:

```text
agent Recrutador
  goal "Receive a job description and answer with the three best matching resumes in a folder"
  remote Leitor at "http://127.0.0.1:8081"
  remote Avaliador at "http://127.0.0.1:8082"
  accepts find folder job  # list the three best resumes in a folder for a job description
  on find
    if not folder
      fail "Preciso do nome da pasta com os currículos"
    if not job
      fail "Preciso do texto da vaga"
    candidates = Leitor.summarize folder: folder within 240 seconds
    ranking = Avaliador.rank job: job candidates: candidates within 180 seconds
    reply "Os 3 melhores currículos para a vaga:\n{ranking}"
```

Read it aloud: the Recrutador understands the `find` message, which carries a folder and a job. It asks the Leitor for the resume digests, hands those digests along with the job to the Avaliador and replies with what the Avaliador returned. The two lines `Leitor.summarize ...` and `Avaliador.rank ...` are A2A calls. `within` allows more time than the default, because reading files is slow.

`leitor.ag` (abridged):

```text
agent Leitor
  goal "Read every resume in a folder, in any format, and write a short digest of each one"
  tool files from mcp "npx -y @modelcontextprotocol/server-filesystem ."
  tool convert from mcp "uvx markitdown-mcp"
  accepts summarize folder  # read all resumes in a folder and summarize each one
  on summarize
    ...
    digest = think "Você é um leitor de currículos. Use a ferramenta files para listar a pasta {folder} ... Para CADA arquivo, use a ferramenta convert ... escreva um bloco ARQUIVO / NOME / RESUMO ..."
    reply digest
```

There are two MCP servers in the same agent. `files` knows how to **list** the folder and find the absolute path. It only reads text, so on its own it cannot open PDF or Word. `convert` (MarkItDown) opens any common format and returns text, but it needs the absolute path in `file://...` form. `think` brings the two together: the model lists, builds the path of each file, converts and summarizes. That is why the step limit goes up to 30.

`avaliador.ag` (abridged):

```text
agent Avaliador
  goal "Compare resume digests with a job description and pick the three best matches"
  tool fetch from mcp "uvx mcp-server-fetch"
  accepts rank job candidates  # pick the three best resumes for a job description
  on rank
    ...
    ranking = think "Você é um avaliador técnico de currículos. Primeiro, identifique na vaga até 3 tecnologias principais e leia a página de cada uma na Wikipedia ... Depois compare cada candidato com a vaga ... Responda SOMENTE com as três melhores ... 1. Nome do candidato | nome-do-arquivo.ext ..."
    reply ranking
```

The internet search here is modest on purpose: the Avaliador confirms what the job's technologies are and which ones are similar, to give partial credit to candidates who use an equivalent (Go instead of Rust, RabbitMQ instead of Kafka). It is there to show a second kind of MCP tool in action. If the page does not load, the Avaliador continues without it, and the ranking still works.

The Avaliador's prompt explicitly asks that required skills weigh more than nice-to-haves, and that production experience weighs more than a bootcamp or a list of keywords. That is what should keep Zé Fulanildo Barroso off the podium.

## Things to try

- **Change the job.** Write a Java job and run again: Fulana Mendes Tavares should rise to the top.
- **Add a resume** in another format (for example `.pptx` or `.xlsx`, formats MarkItDown usually supports; I have not tested these two) and watch the Leitor include it with no change to the agents.
- **Change the model.** Change `model` in `metagente.toml`. The Leitor and the Avaliador follow the change.
- **Ask for the reason.** In `avaliador.ag`, change the requested format to `1. Name | file | reason in one sentence` and compare.
- **Break it on purpose.** Stop Terminal 2 and run the Recrutador: the error should say it could not reach the Avaliador at `http://127.0.0.1:8082`.
- **Missing folder.** `folder=nao-existe`: the Leitor should report that it could not list the folder.

## Common problems

| You see | What it means | What to do |
|---------|---------------|------------|
| `the language model could not answer: the variable ANTHROPIC_API_KEY is not set` | The server terminal does not have the key | `export ANTHROPIC_API_KEY=...` in Terminals 1 and 2 |
| `I could not reach http://127.0.0.1:8081/...` (or `8082`) | The corresponding server is not running | Start Terminal 1 or 2 |
| `I could not start the tool server` with `npx` | Node.js is not installed | Install Node.js and check `npx --version` |
| `I could not start the tool server` with `uvx` | `uv` is not installed | Install uv and check `uvx --version` |
| `the agent used 30 steps and did not finish` | Folder too large for the limit | Raise `think_max_steps` and `timeout_seconds` |
| `did not finish within ... seconds` | Slow model or conversion | Try again or raise `timeout_seconds` and the Recrutador's `within` values |
| `I could not listen for A2A on 127.0.0.1:8081` | Something else is using the port | Stop the other program, or change the port in `serve` **and** in `recrutador.ag` |
| The final list has fewer than 3 lines or comes with extra text | The model did not follow the format | Run again; if it persists, reinforce the format in the Avaliador's prompt |
| A resume does not appear in the digests | The model skipped the file | Run rung 2 and check; raise `think_max_steps` |

If anything else goes wrong, `metagente check` on each file points to the line with the error and says how to fix it.

## Limits and cautions

- **The converter is not confined to the folder.** `markitdown-mcp` reads any `file://` on the computer. What restricts the Leitor to the folder is the prompt text, not the server. The file server, on the other hand, does refuse paths outside the project folder (tested). For real use, run it in a folder or account that holds only what may be read.
- **Real resumes are personal data.** The text of each file is sent to the Claude API. With real candidates, confirm the legal basis (LGPD, or GDPR where it applies) and your company's policy before running. This demo uses only fictional data.
- **The ranking is a support, not a decision.** A model can make mistakes or reflect biases in the text. Use the list to prioritize human reading, not to discard candidates.
- **The result varies.** The model writes differently on each run. The order of positions 2 and 3 may swap.

## What this demo shows about Metagente

| Feature | Where it appears |
|---------|------------------|
| Agent calling agent over A2A | `Leitor.summarize` and `Avaliador.rank` in `recrutador.ag`, with `within` to adjust the time |
| MCP for files | `tool files from mcp` and `tool convert from mcp` in `leitor.ag` |
| MCP for the internet | `tool fetch from mcp` in `avaliador.ag` |
| Several MCP servers in one agent | The Leitor uses two |
| `think` with declared tools | The Leitor and the Avaliador, limited to what each one declared |
| Coordinator without a model | The Recrutador only orchestrates, with `if`, `fail` and `reply` |
| Configuration outside the agents | `metagente.toml` (model, key, time, steps) |
| `serve` with A2A | Two servers on different ports, on this computer only |