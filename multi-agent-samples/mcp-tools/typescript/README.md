# A tool written in TypeScript

The `calc` tool server in TypeScript, plugged into the two agents of this sample. Only the commands are here. The
explanation of every line is in the [guide](../../../docs/mcp-tools-guide.md).

* `src/server.ts`: the tool server (three tools: `mean`, `dot`, `scale`).
* `package.json` and `package-lock.json`: the pinned packages. `tsconfig.json`: how `tsc` builds.
* `vectors.ag` and `caller.ag`: the two agents. They are the same files in every language folder.
* `metagente.toml`: says which program is the tool (`calc_command`). Change that one line to use another language.

## Before you start

You need `metagente` version **0.1.5 or newer**, Node.js **20 or newer** with npm (`node --version`). Python **3.9 or newer** is also needed, only for `smoke.py` (`python3 --version`; `py --version` on Windows).
Open a terminal in this folder (`multi-agent-samples/mcp-tools/typescript`) and run every command from here: the tool is
started from the folder where you start `metagente`.

## Step 1: install and build

**Linux and macOS**

```text
npm install
npm run build
```

**Windows (PowerShell)**

```text
npm install
npm run build
```

## Step 2: test the tool without an agent

**Linux and macOS**

```text
python3 ../smoke.py -- node dist/server.js
```

**Windows (PowerShell)**

```text
py ..\smoke.py -- node dist\server.js
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
    Problem on line 12 of vectors.ag: the tool `dot` failed: vectors must have the same size
      12 |     r = calc.dot a: a b: b
         |         ^
    Fix: check the values you sent, or the tool's own documentation
```

More cases, with their messages, are in the guide.

## Clean up

Delete the `node_modules` and `dist` folders.
