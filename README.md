# Metagente

![](logo.jpg)

**Version: v0.1.1**

Metagente is an interpreted fourth generation language (4GL) for building AI agents in minutes, with
little or no programming experience. The interpreter is written in Rust and speaks MCP (Model Context
Protocol) and A2A (Agent to Agent) natively.

## A first agent

```text
agent Weather
  goal "Answer questions about the weather"
  tool weather from mcp "npx -y weather-mcp"
  accepts ask city
  on ask
    forecast = weather.forecast city: city
    reply "In {city} it will be {forecast.summary}"
```

(The MCP server above is illustrative. See `specs/001-metagente-core/contracts/language.md`.)

## Status

The core is built and tested: the interpreter, the built in tools, MCP, dynamic link and A2A. See
[CHANGELOG.md](CHANGELOG.md) and [docs/tutorial.md](docs/tutorial.md).

```text
cargo build --release
target/release/metagente new hello
target/release/metagente run hello.ag greet name=World
```

## Roadmap

| Stage | Scope | State |
|-------|-------|-------|
| v0.1.0 | Constitution, spec, plan, contracts and task list for the core | Done |
| MVP | Tools and tasks (built in and MCP), `run`, `check`, `new`, beginner friendly errors, tutorial (user stories 1 and 2) | Built |
| Safe agents | Per-agent permissions: file folders, named environment variables, declared links (user story 3) | Built |
| Dynamic link | Agents calling agents by name or path, interface checks, cycle detection (user story 4) | Built |
| v1.0 | MCP server, A2A Agent Card, receive and send A2A tasks, local-only `serve` (user story 5) | Built; waiting for a first-time-user session (SC-001) and a tagged release |
| Later | Long running tasks with later result checks, authentication for served agents, central agent registry, scheduling, A2A streaming, `ListTasks` and `CancelTask` | Ideas, each needs its own spec |

**Project Barracuda:**

The next major milestone is to create an agent server that can be invoked via A2A or even through a frontend, and that also integrates with a queue for batch triggering.

**A great sample:**

A sample that puts it together, [City Briefing](samples/city-briefing/README.md), shows a Concierge agent
asking a Researcher agent over A2A while the Researcher reads a page through an MCP tool, both using Claude
(everything is tested except the run with the live Claude API, which needs your key).

The full task list is in `specs/001-metagente-core/tasks.md`.

## Project documents

| Document | Path |
|----------|------|
| Programming guide (if, loops, results) | `docs/guide.md` |
| Samples (start with `city-briefing`: two agents, A2A, MCP and Claude) | `samples/` |
| Constitution (principles) | `.specify/memory/constitution.md` |
| Feature spec | `specs/001-metagente-core/spec.md` |
| Implementation plan | `specs/001-metagente-core/plan.md` |
| Task list | `specs/001-metagente-core/tasks.md` |
| Language, CLI, configuration and protocol contracts | `specs/001-metagente-core/contracts/` |
| Validation guide | `specs/001-metagente-core/quickstart.md` |

## Building the Windows and macOS zips

The zip for Linux is built on Linux. The zips for Windows and macOS are built by hand, on a computer of that
platform, by following the steps below. Do the steps of one platform only. Nothing here is a script.

### Windows (produces `dist\metagente-windows-amd64.zip`)

**What you need.** Rust, from <https://rustup.rs>. The Rust installer may ask you to install the Visual Studio C++
build tools; accept it.

**Steps.**

1. In File Explorer, open the repository folder, click the address bar, type `powershell` and press Enter. A
   PowerShell window opens in that folder.
2. Build the program by running this line:

   ```text
   cargo build --release
   ```

3. In File Explorer, open the folder `dist` inside the repository folder (create it if it is not there; git
   ignores it). If the folder `metagente` or the file `metagente-windows-amd64.zip` are already there from an earlier
   build, delete them. Then create a new, empty folder named `metagente` inside `dist`.
4. Inside `dist\metagente`, create folders so that you get this tree (the files come in the next steps):

   ```text
   metagente
   ├── README.md
   ├── metagente.toml
   ├── bin
   │   └── metagente.exe
   ├── docs
   │   ├── README.md
   │   ├── guide.md
   │   ├── syntax.md
   │   └── tutorial.md
   └── samples
       ├── clock.ag
       ├── read_file.ag
       ├── weather.ag
       ├── planner.ag
       └── remote_weather.ag
   ```

5. Copy each file in the left column (a path inside the repository folder) to the place in the right column (a path
   inside `dist\metagente`):

   | Copy this file | To here |
   |----------------|---------|
   | `target\release\metagente.exe` | `bin\metagente.exe` |
   | `metagente.toml` | `metagente.toml` |
   | `README.md` | `docs\README.md` |
   | `docs\guide.md` | `docs\guide.md` |
   | `docs\syntax.md` | `docs\syntax.md` |
   | `docs\tutorial.md` | `docs\tutorial.md` |
   | `examples\clock.ag` | `samples\clock.ag` |
   | `examples\read_file.ag` | `samples\read_file.ag` |
   | `examples\weather.ag` | `samples\weather.ag` |
   | `examples\planner.ag` | `samples\planner.ag` |
   | `examples\remote_weather.ag` | `samples\remote_weather.ag` |

6. Create the file `README.md` at the top of `dist\metagente`: open Notepad, paste the text under "content of
   README.md" below, choose File, Save as, go to `dist\metagente`, type `README.md` as the name and choose "All files"
   in "Save as type". Check that the file is named `README.md` and not `README.md.txt`.
7. In File Explorer, open `dist` and right click the folder `metagente`. On Windows 11 choose "Compress to ZIP file".
   On Windows 10, or on Windows 11 under "Show more options", choose "Send to", then "Compressed (zipped) folder".
   Then rename the new `metagente.zip` to `metagente-windows-amd64.zip`.

**Content of README.md** (Windows):

```text
# Metagente v0.1.1 (Windows, 64 bit)

Metagente is a language for building AI agents in minutes, with little or no programming experience.

## Run it from this folder

Open PowerShell in this folder and run:

    .\bin\metagente.exe --version
    .\bin\metagente.exe run samples\clock.ag now

The other files in samples\ are for reading and may need extra setup (a tool, Node.js or a running agent):
see docs\tutorial.md.

## Use it from anywhere

Add the bin folder to Path: open Settings, search for "Edit environment variables", edit Path, add the full
path of this folder's bin, then open a new terminal.

## Works on

64 bit Windows (AMD64).

The first time you run it, Windows SmartScreen may warn that the file is not signed: click "More info", then "Run anyway".

## More

Documentation: the docs folder. Repository: https://github.com/cleuton/MetaAgent
```

### macOS, Apple Silicon (produces `dist/metagente-macos-arm64.zip`)

**What you need.** Rust, from <https://rustup.rs>.

**Steps.**

1. Open Terminal in the repository folder: in the Finder, right click the folder and choose "New Terminal at Folder".
   If you do not see it, turn it on in System Settings, Keyboard, Keyboard Shortcuts, Services, Files and Folders.
2. Build the program by running this line:

   ```text
   cargo build --release
   ```

3. In the Finder, open the folder `dist` inside the repository folder (create it if it is not there; git ignores
   it). If the folder `metagente` or the file `metagente-macos-arm64.zip` are already there from an earlier build,
   delete them. Then create a new, empty folder named `metagente` inside `dist`.
4. Inside `dist/metagente`, create folders so that you get this tree (the files come in the next steps):

   ```text
   metagente
   ├── README.md
   ├── metagente.toml
   ├── bin
   │   └── metagente
   ├── docs
   │   ├── README.md
   │   ├── guide.md
   │   ├── syntax.md
   │   └── tutorial.md
   └── samples
       ├── clock.ag
       ├── read_file.ag
       ├── weather.ag
       ├── planner.ag
       └── remote_weather.ag
   ```

5. Copy each file in the left column (a path inside the repository folder) to the place in the right column (a path
   inside `dist/metagente`):

   | Copy this file | To here |
   |----------------|---------|
   | `target/release/metagente` | `bin/metagente` |
   | `metagente.toml` | `metagente.toml` |
   | `README.md` | `docs/README.md` |
   | `docs/guide.md` | `docs/guide.md` |
   | `docs/syntax.md` | `docs/syntax.md` |
   | `docs/tutorial.md` | `docs/tutorial.md` |
   | `examples/clock.ag` | `samples/clock.ag` |
   | `examples/read_file.ag` | `samples/read_file.ag` |
   | `examples/weather.ag` | `samples/weather.ag` |
   | `examples/planner.ag` | `samples/planner.ag` |
   | `examples/remote_weather.ag` | `samples/remote_weather.ag` |

6. Create the file `README.md` at the top of `dist/metagente`: open TextEdit, choose Format, Make Plain Text, paste the
   text under "content of README.md" below, and save it as `README.md` in `dist/metagente`. If macOS asks whether to
   use `.txt` or `.md`, choose `.md`. Check that the file is named `README.md` and not `README.md.txt`.
7. Open Terminal in the `dist` folder the same way (right click it in the Finder, "New Terminal at Folder") and run
   this line. The Finder's "Compress" is not used, because it adds `__MACOSX` and `.DS_Store` entries to the zip:

   ```text
   zip -r -D metagente-macos-arm64.zip metagente
   ```

**Content of README.md** (macOS):

```text
# Metagente v0.1.1 (macOS, Apple Silicon)

Metagente is a language for building AI agents in minutes, with little or no programming experience.

## Run it from this folder

Open Terminal in this folder and run:

    ./bin/metagente --version
    ./bin/metagente run samples/clock.ag now

The other files in samples/ are for reading and may need extra setup (a tool, Node.js or a running agent):
see docs/tutorial.md.

## Use it from anywhere

Either add the bin folder to your PATH (this terminal only):

    export PATH="$PWD/bin:$PATH"

or copy the program to a folder that is already on the PATH:

    sudo mkdir -p /usr/local/bin
    sudo cp bin/metagente /usr/local/bin/

## Works on

Macs with Apple Silicon (M1 or newer).

The first time you run it, macOS may block it because it is not signed. Clear that with:

    xattr -d com.apple.quarantine bin/metagente

## More

Documentation: the docs folder. Repository: https://github.com/cleuton/MetaAgent
```

## Versioning

Semantic versioning. The version is kept in this file, in `CHANGELOG.md` and in `Cargo.toml`.
