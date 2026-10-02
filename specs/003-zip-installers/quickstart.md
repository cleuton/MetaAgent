# Quickstart: checking the zip

Nothing here commits, tags or pushes.

1. `./target/release/metagente --version` prints exactly `metagente 0.1.1`.
2. `unzip -Z1 dist/metagente-linux-amd64.zip | sort` lists exactly the 12 files in `data-model.md`.
3. `unzip -Z dist/metagente-linux-amd64.zip` shows `bin/metagente` with execute permission (`-rwx…`).
4. Unzip into an empty temporary folder. From inside `metagente/`: `./bin/metagente --version` prints `metagente 0.1.1`, and
   `./bin/metagente run samples/clock.ag now` answers with the time.
5. Put the `bin` folder on the PATH in a throwaway shell; `metagente --version` works from another folder.
6. `git status --short` shows only `README.md`, `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `.gitignore`,
   `specs/003-zip-installers/` and `.specify/feature.json`. `dist/` does not appear.
7. Windows and macOS: not testable here. The macOS steps can be rehearsed in a scratch copy outside the repository.
