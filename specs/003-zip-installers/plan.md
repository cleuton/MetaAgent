# Implementation Plan: Linux Zip Distribution

**Branch**: none (work stays on the current branch) | **Date**: 2026-10-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/003-zip-installers/spec.md`

## Summary

Bump the version to v0.1.1 in `README.md` (the Version line and the out of date sentence in its "Versioning" section), `Cargo.toml` and `CHANGELOG.md`; add the section "Building the Windows and
macOS zips" to `README.md` and `dist/` to `.gitignore`; then build once and zip a staging folder (kept outside the
repository) into `dist/metagente-linux-amd64.zip` with the system `zip` command. No script is created anywhere.

## Technical Context

**Language/Version**: none to write. Existing Rust project (`cargo build --release`); shell commands typed by the agent, not saved

**Primary Dependencies**: `cargo`, `zip`, `unzip`, `objdump`. All four are present here (checked read only)

**Storage**: files only: `dist/metagente-linux-amd64.zip` (gitignored) and a temporary staging folder outside the repository

**Testing**: the checks in the spec: `./target/release/metagente --version` must print exactly `metagente 0.1.1` before zipping; after zipping, the listing must match the layout and `bin/metagente` must keep its executable permission (`unzip -Z`); an unzip in a temporary folder runs `--version` and `samples/clock.ag`

**Target Platform**: Linux AMD64, glibc (this machine: Ubuntu, glibc 2.39). The README states the minimum read with `objdump -T`

**Project Type**: packaging of an existing CLI; nothing in `src/` changes

**Constraints**: no script; no git command that changes state; no branch; repository edits limited to `README.md`, `Cargo.toml`, `Cargo.lock` (by the build), `CHANGELOG.md`, `.gitignore`, plus the Speckit documents and `.specify/feature.json`

**Scale/Scope**: one zip of 12 files; one README section

## Constitution Check

- [x] **I to VII**: no language, tool, protocol, link or interpreter change.
- [x] **Constraints**: "Simple packaging" (a single folder in a single file) is what this feature delivers. Portability untouched.
- [x] **Workflow**: no grammar change; the zip ships existing examples.

Post-design re-check: passes. No Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/003-zip-installers/
├── plan.md
├── research.md
├── data-model.md      # the zip layout
├── quickstart.md      # how to check the result
└── tasks.md           # later, by /speckit-tasks
```

No `contracts/`: the only interface is the zip layout.

### Source Code (repository root)

```text
README.md        # version line + Versioning sentence + new section "Building the Windows and macOS zips"
Cargo.toml       # version = "0.1.1"
Cargo.lock       # updated by cargo build, not by hand
CHANGELOG.md     # "Unreleased" becomes "v0.1.1 - 2026-10-02" + one line
.gitignore       # + dist/
dist/            # generated, gitignored
```

**Structure Decision**: no new file or folder in the repository. The staging folder is a `mktemp -d` outside it, deleted after the zip is verified.

## Complexity Tracking

No violations.
