# Tasks: Linux Zip Distribution

**Input**: `/specs/003-zip-installers/` (spec.md, plan.md, research.md, data-model.md, quickstart.md)

**Rules for every task**:
- Create no script, anywhere, and leave none behind.
- Run no git command that changes state: no `add`, `commit`, `tag`, `branch`, `checkout`, `stash`, `restore` or `push`. Reading with `git status` and `git diff` is fine.
- Edit only these repository files: `README.md`, `Cargo.toml`, `CHANGELOG.md`, `.gitignore` (and `Cargo.lock`, which only `cargo build` changes). Everything else is built in a temporary folder outside the repository.
- No tests are requested; the checks are the ones written in the tasks.

**Execution order** (not the same as the priority order of the stories): Phase 1, Phase 2, Phase 3 (story 2, the README section), Phase 4 (story 1, the Linux zip), Phase 5. Story 2 runs first because the zip copies `README.md`, which must already hold the section.

## Phase 1: Setup

- [x] T001 Check that `cargo`, `zip`, `unzip` and `objdump` exist (`which cargo zip unzip objdump`). If one is missing, stop and tell the user which one and how to install it; do not use another tool

## Phase 2: Foundational (version v0.1.1 and `.gitignore`)

- [x] T002 In `.gitignore`, add the single line `dist/` at the end
- [x] T003 In `README.md`, change the line `**Version: v0.1.0**` to `**Version: v0.1.1**`, and rewrite the sentence in the "## Versioning" section ("Semantic versioning. The version is kept in this file, in `CHANGELOG.md`, and (once it exists) in `Cargo.toml`.", which spans two lines) as "Semantic versioning. The version is kept in this file, in `CHANGELOG.md` and in `Cargo.toml`." Leave every other `v0.1.0` in the file as it is (history)
- [x] T004 In `Cargo.toml`, change `version = "0.1.0"` to `version = "0.1.1"` in the `[package]` section. Do not edit `Cargo.lock` by hand
- [x] T005 In `CHANGELOG.md`, change the heading `## Unreleased` to `## v0.1.1 - 2026-10-02`, and add as the last line of its `### Added` list: `- Distribution as a zip for Linux AMD64, and instructions in the README to build the Windows and macOS zips.`

---

## Phase 3: User Story 2 - Windows and macOS instructions in the README (Priority: P2, done FIRST because FR-001 needs it)

**Goal**: `README.md` has the section "Building the Windows and macOS zips", plain instructions, no script.

**Independent Test**: the section contains no `mkdir`, `cp`, `rm`, `Remove-Item`, `cat` or heredoc in its steps, and a rehearsal of the macOS steps by hand in a scratch folder gives the 12 files of the layout.

The zip for Linux copies `README.md` into `docs/README.md`, so this phase must be finished before Phase 4 copies it.

- [x] T006 [US2] In `README.md`, add the new section "Building the Windows and macOS zips" just before `## Versioning`. Write the Windows part: the one prerequisite (Rust from rustup.rs) plus one plain sentence that the Rust installer may ask for the Visual Studio C++ build tools and to accept it; numbered plain sentence steps (open PowerShell in the repository root by typing `powershell` in the File Explorer address bar; run `cargo build --release`; create `metagente` inside `dist/`, creating `dist/` if missing and deleting any old `metagente` folder and old zip there; build the folder tree; copy the files; create `README.md` checking it does not end in `.md.txt`, in Notepad choosing "All files"; zip with Windows 11 "Compress to ZIP file" or Windows 10 / "Show more options" "Send to", "Compressed (zipped) folder", then rename to `metagente-windows-amd64.zip`); the folder tree in a plain text block with `bin\metagente.exe`; and the two column list of source and destination, exactly the rows in FR-004. The only command line in the steps is `cargo build --release`
- [x] T007 [US2] In the same section, write the macOS part: same structure, with "New Terminal at Folder" (and where to turn it on: System Settings, Keyboard, Keyboard Shortcuts, Services, Files and Folders), `target/release/metagente` to `bin/metagente`, and the zip step as "open a terminal in `dist/` the same way and run `zip -r -D metagente-macos-arm64.zip metagente`", with the reason Finder's Compress is not used. The only command lines in the steps are `cargo build --release` and that `zip` line
- [x] T008 [US2] In the same section, add for each platform a plain text block labeled "content of README.md" with the full small README of that platform, version already written as `v0.1.1`: one sentence about Metagente; how to run (`.\bin\metagente.exe --version` and `.\bin\metagente.exe run samples\clock.ag now` on Windows, `./bin/...` on macOS) and the line about other samples needing setup (`docs/tutorial.md`); the PATH note (Windows: one line about Path under "Edit environment variables", no command; macOS: the `export PATH` line and the `sudo cp bin/metagente /usr/local/bin/` line); the systems line (64 bit Windows; Apple Silicon Macs); the security line (SmartScreen "More info", "Run anyway"; Gatekeeper and `xattr -d com.apple.quarantine bin/metagente`); the docs location and `https://github.com/cleuton/MetaAgent`. One screen each
- [x] T009 [US2] Check the section: search it for `mkdir`, `cp `, `rm `, `Remove-Item`, `cat >` and `<<` outside the two README blocks (there must be none); confirm nothing in the steps mentions the spec or "D-1"; then rehearse the macOS steps by hand in a scratch folder outside the repository (copy only the files in the list, write the README from the block, run the `zip` line) and confirm the zip has exactly the 12 files of the layout. Delete the scratch folder

**Checkpoint**: the README section is complete.

---

## Phase 4: User Story 1 - The Linux zip (Priority: P1)

**Goal**: `dist/metagente-linux-amd64.zip` exists with the layout in `data-model.md`.

**Independent Test**: unzip it in an empty temporary folder, run `./bin/metagente --version` (prints `metagente 0.1.1`) and `./bin/metagente run samples/clock.ag now`.

- [x] T010 [US1] Run `cargo build --release` in the repository root. `Cargo.lock` is updated by this command; do not edit it
- [x] T011 [US1] Run `./target/release/metagente --version`. Continue only if it prints exactly `metagente 0.1.1`. If not, stop and report it; do not edit any file outside the list in the rules above to make it match
- [x] T012 [US1] Run `objdump -T target/release/metagente | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1`. The result is the highest `GLIBC_x.y` tag, and it is the glibc minimum to write in the Linux README
- [x] T013 [US1] Create a temporary folder outside the repository (`mktemp -d`) and, inside it, the folder `metagente` with `bin`, `docs` and `samples`. Copy in: `target/release/metagente` to `bin/metagente`; `metagente.toml`; `README.md` to `docs/README.md` (it already has the new section and version); `docs/guide.md`, `docs/syntax.md`, `docs/tutorial.md` to `docs/`; `examples/clock.ag`, `read_file.ag`, `weather.ag`, `planner.ag`, `remote_weather.ag` to `samples/`
- [x] T014 [US1] Write `metagente/README.md` in the temporary folder, one screen, saying only: one sentence about Metagente and `v0.1.1`; `./bin/metagente --version`, then `./bin/metagente run samples/clock.ag now`, and a line that the other samples need extra setup (`docs/tutorial.md`); the PATH note with `export PATH="$PWD/bin:$PATH"` and `sudo cp bin/metagente /usr/local/bin/`; the glibc minimum from T012 stated as measured; the docs location and `https://github.com/cleuton/MetaAgent`. No security line (Linux)
- [x] T015 [US1] Create `dist/` if missing, delete any old `dist/metagente-linux-amd64.zip`, then from the temporary folder run `zip -r -D <absolute path of the repository>/dist/metagente-linux-amd64.zip metagente`
- [x] T016 [US1] Verify the zip: `unzip -Z1 dist/metagente-linux-amd64.zip | sort` must list exactly the 12 files of the layout; `unzip -Z dist/metagente-linux-amd64.zip` must show `bin/metagente` with execute permission; unzip into a second temporary folder and from inside `metagente/` run `./bin/metagente --version` and `./bin/metagente run samples/clock.ag now`
- [x] T017 [US1] Delete both temporary folders

**Checkpoint**: the Linux zip is ready.

---

## Phase 5: Polish

- [x] T018 Run `git status --short`. It must show only `README.md`, `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `.gitignore`, `specs/003-zip-installers/` and `.specify/feature.json`; `dist/` must not appear. Report anything else
- [x] T019 Report to the user: the zip path and size, its file list, the glibc minimum written in its README, which checks passed, that the Windows and macOS zips were not built (only the macOS steps were rehearsed), and that the 2 minutes of SC-001 were not measured (only that every README step works when followed literally). Do not commit, tag or push

---

## Dependencies and order

- T001, then Phase 2 (T002 to T005, independent of each other), then Phase 3 (T006 to T009, in order, same file), then Phase 4 (T010 to T017, in order), then Phase 5.
- Phase 3 comes before Phase 4 on purpose: T013 copies `README.md`, which must already hold the new section and version.
- T004 must be done before T010, so the binary prints 0.1.1.

## Parallel opportunities

- T002 to T005 touch four different files and can be done together. Nothing else is worth parallelizing.

## Implementation strategy

MVP is the Linux zip (Phases 1, 2 and 4), but FR-001 needs the README section first, so do Phase 3 before building. Then stop: no scripts, no extras.
