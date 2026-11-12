# A tool written in Python

The `calc` tool server in Python, plugged into the two agents of this sample. Only the commands are here. The
explanation of every line is in the [guide](../../../docs/mcp-tools-guide.md).

* `server.py`: the tool server (three tools: `mean`, `dot`, `scale`).
* `vectors.ag` and `caller.ag`: the two agents. They are the same files in every language folder.
* `metagente.toml`: says which program is the tool (`calc_command`). Change that one line to use another language.
* `requirements.txt`: the pinned `mcp` package.

## Before you start

You need `metagente` version **0.1.5 or newer**, and Python **3.10 or newer** (`python3 --version`; `py --version` on
Windows). Open a terminal in this folder (`multi-agent-samples/mcp-tools/python`) and run every command from here: the
tool is started from the folder where you start `metagente`.

## Step 1: make the virtual environment and install the package

**Linux and macOS**

```text
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
```

**Windows (PowerShell)**

```text
py -m venv .venv
.venv\Scripts\pip install -r requirements.txt
```

## Step 2: test the tool without an agent

**Linux and macOS**

```text
python3 ../smoke.py -- .venv/bin/python server.py
```

**Windows (PowerShell)**

```text
py ..\smoke.py -- .venv\Scripts\python.exe server.py
```

It prints twelve lines and ends with `11 of 11 checks passed`.

## Step 3: run the agent

```text
metagente run caller.ag go
```

```text
mean 5, dot 32, doubled [2, 4, 6]
```

## Break it on purpose

`metagente run caller.ag oops` sends two vectors of different sizes. The tool refuses, and its own sentence reaches you:

```text
Problem on line 13 of caller.ag: Vectors could not answer `similarity`
  13 |     reply Vectors.similarity a: [1, 2] b: [1]
     |           ^
  Vectors could not answer:
    Problem on line 12 of vectors.ag: the tool `dot` failed: Error executing tool dot: vectors must have the same size
      12 |     r = calc.dot a: a b: b
         |         ^
    Fix: check the values you sent, or the tool's own documentation
```

More cases, with their messages, are in the guide.

## Clean up

Delete the `.venv` folder.
