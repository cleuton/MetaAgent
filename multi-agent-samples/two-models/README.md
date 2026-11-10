# Two models in one agent

One small agent, two language models. A cheap, fast model rewrites the question as a short search query, and a
stronger model writes the answer. The agent has six lines. It never says which company, which model or which key:
that is in `metagente.toml`, and the agent only says `using fast` and `using strong`.

```text
 question --> fast (Anthropic) --> short query --> strong (OpenAI) --> answer
```

* `assistant.ag`: the agent. Its two `think` lines end with `using fast` and `using strong`.
* `metagente.toml`: names the two models (`[llm.fast]` and `[llm.strong]`, which live under the `[llm]` section you may
  already know) and the **variables** that hold each model name and each key. The keys and the model names are never
  written in a file.

Changing a model, or the company behind it, changes `metagente.toml` and no agent.

## Before you start

You need `metagente` version **0.1.5 or newer** (the `[version]` line of `metagente.toml` makes an older program stop with
a plain message instead of running it wrongly), an Anthropic key and an OpenAI key, and the name of one model of each
company that can be used with tools (look them up in the documentation of each company).

Open a terminal in this folder (`multi-agent-samples/two-models`).

## Step 1: put the names and the keys in your environment

Nothing below is saved in a file. Replace the words in angle brackets.

**Linux and macOS**

```text
export FAST_MODEL=<a small Anthropic model>
export FAST_KEY=<your Anthropic key>
export STRONG_MODEL=<an OpenAI model>
export STRONG_KEY=<your OpenAI key>
```

**Windows (PowerShell)**

```text
$env:FAST_MODEL = "<a small Anthropic model>"
$env:FAST_KEY = "<your Anthropic key>"
$env:STRONG_MODEL = "<an OpenAI model>"
$env:STRONG_KEY = "<your OpenAI key>"
```

**Windows (Command Prompt)**

```text
set FAST_MODEL=<a small Anthropic model>
set FAST_KEY=<your Anthropic key>
set STRONG_MODEL=<an OpenAI model>
set STRONG_KEY=<your OpenAI key>
```

## Step 2: check it

```text
metagente check assistant.ag
```

It prints `No problems found in assistant.ag (1 agent).` If a variable is not set in this terminal it says so first, as a
warning (it does not stop):

```text
Warning: the model fast needs the variable FAST_KEY, and it is not set here.
```

## Step 3: run it

```text
metagente run assistant.ag ask question="why is the sky blue"
```

You get a short answer about sunlight being scattered by the air. The fast model only rewrote your question as a short
search query; the strong model wrote what you read.

## Break it on purpose

Each change below stops with a plain message. Undo each change before the next one. The messages are shown without the
folder of the file; yours starts with the full path.

1. **A misspelled model name.** In `assistant.ag` change `using fast` to `using fats`, then `metagente check assistant.ag`:

   ```text
   Problem on line 6 of assistant.ag: There is no model called fats. The models are: fast, strong.
   Fix: did you mean fast?
   ```

2. **A misspelled setting.** In `metagente.toml` change `api_key_env` under `[llm.fast]` to `api_key_evn`, then
   `metagente check assistant.ag`:

   ```text
   Problem on line 9 of metagente.toml: [llm.fast] has the setting api_key_evn, which I do not know.
   Fix: did you mean api_key_env?
   ```

3. **A variable that is not set.** Open a new terminal (so the variables of step 1 are gone) and run step 3:

   ```text
   Problem on line 6 of assistant.ag: The model fast needs the variable FAST_KEY, and it is not set.
   Fix: set the variable
   ```

4. **Another address.** Under `[llm.fast]` add the line `base_url = "https://example.com/v1"`, then
   `metagente check assistant.ag`:

   ```text
   Problem on line 9 of metagente.toml: [llm.fast] has base_url. A named model always uses the official address.
   Fix: for another address, use the [llm] section
   ```

   A named model is always the real Anthropic or OpenAI service. For a gateway of your own, keep using the `[llm]` section
   as before: nothing about it changed.

## Common problems

| What you see | What it means | What to do |
|--------------|---------------|------------|
| `This project needs Metagente 0.1.5 or newer, and this is 0.1.4. Nothing was run.` | An older program reads the `[version]` line and refuses the project | Install 0.1.5 or newer |
| `The model fast needs the variable FAST_KEY, and it is not set.` | The variable named by `api_key_env` is empty or missing in this terminal | Set it (step 1) in the terminal you run from |
| `The model fast needs the variable FAST_MODEL, and it is empty or not set.` | The variable named by `model_env` is empty or missing | Set it (step 1) |
| `The model strong (openai) answered with status 401.` | The key is not accepted by that company | Check the key and that it belongs to that company |
| `The model strong (openai) answered with status 404.` (or 400) | The model name is not known to that company, or that model does not accept tools | Check `STRONG_MODEL` |
| `The model strong (openai) answered with status 429.` | Too many requests, or no credit left | Wait and try again, or check the plan of your account |
| `The model fast (anthropic) could not be reached: timeout.` | No answer in time | Check your connection and the `[network]` section (a proxy?) |
| `There is no model called ...` | The word after `using` is not a name under `[llm]` | Use one of the names the message lists |
| `The provider ... is not one a named model accepts. Use anthropic or openai.` | Only those two companies, at their own addresses | For another service use the `[llm]` section |

A message about a model never shows a key, a model name or an address.
