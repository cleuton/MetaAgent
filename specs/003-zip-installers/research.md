# Research: Linux Zip Distribution

The spec fixes the decisions. This only records how the few open points are settled.

## R1 Tools
- **Decision**: `cargo`, `zip`, `unzip`, `objdump`. All installed here (`unzip` comes from linuxbrew). If one were missing the run stops and names it (D-2).

## R2 glibc minimum
- **Decision**: run `objdump -T target/release/metagente`, take the highest `GLIBC_x.y` tag, and write it in the Linux README.
- **Rationale**: the spec forbids an assumed value. A first look at an earlier build showed `GLIBC_2.39`, so the README will probably say Ubuntu 24.04 or newer; the zip will not run on Ubuntu 22.04.
- **Alternative**: a static musl build would run everywhere, but the spec says plain glibc.

## R3 Order of work
- **Decision**: version changes and the README section first, then build, then assemble and zip (FR-001).
- **Rationale**: the zip's `docs/README.md` is a copy of `README.md`, so it must already hold the new version and the section; the binary must be built after the `Cargo.toml` bump so it prints 0.1.1.

## R4 Zip command
- **Decision**: from the staging parent folder, `zip -r -D <absolute dist path>/metagente-linux-amd64.zip metagente`, after deleting any old zip.
- **Rationale**: `-D` leaves out folder entries; running from the parent keeps `metagente/` as the single top folder; the executable bit of `bin/metagente` is stored, and `unzip -Z` shows it.

## R5 Staging
- **Decision**: `mktemp -d` outside the repository, deleted after the checks (D-2).

## R6 README section
- **Decision**: one section placed before "## Versioning". Per platform: the prerequisite, numbered plain sentence steps, the folder tree in a plain text block, a two column source/destination list, and the platform's README text in a block labeled "content of README.md". No command blocks except `cargo build --release` and, on macOS, the `zip` line.
