#!/bin/sh
# 0.1.5: one-off tool. Freezes what Metagente 0.1.4 did with the code blocks of its own docs, so that 0.1.5 can be
# proved to behave the same (spec FR-022). Run it ONCE, with a binary built from the untouched 0.1.4 tree, before any
# change to src/. It writes tests/fixtures/v0.1.4-blocks/ (the blocks) and tests/fixtures/v0.1.4-golden/ (what
# `metagente check` printed for each, with the version number replaced by <version> and temporary folders by <tmp>).
set -eu
cd "$(dirname "$0")/.."
BIN="${1:-target/debug/metagente}"
BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"
BLOCKS=tests/fixtures/v0.1.4-blocks
GOLDEN=tests/fixtures/v0.1.4-golden
rm -rf "$BLOCKS" "$GOLDEN"
mkdir -p "$BLOCKS" "$GOLDEN"

python3 - "$BLOCKS" <<'PY'
import re, subprocess, sys, os
out = sys.argv[1]
files = ["README.md", "docs/tutorial.md", "docs/guide.md", "docs/syntax.md",
         "docs/secure-connections-guide.md", "docs/authentication-guide.md"]
for f in files:
    text = subprocess.run(["git", "show", "v0.1.4:" + f], capture_output=True, text=True, check=True).stdout
    source = f.replace("/", "-").replace(".md", "")
    n = 0
    for m in re.finditer(r"^```([a-zA-Z0-9_-]*)\n(.*?)^```\s*$", text, re.S | re.M):
        lang, body = m.group(1), m.group(2)
        first = next((l for l in body.splitlines() if l.strip() and not l.strip().startswith("#")), "")
        if lang == "toml":
            ext = "toml"
        elif lang in ("", "text") and re.match(r"agent\s+\w", first):
            ext = "ag"
        else:
            continue
        n += 1
        with open(os.path.join(out, "%s-%02d.%s" % (source, n, ext)), "w") as fh:
            fh.write(body)
PY

HELLO='agent Hello
  goal "say hello"
  accepts hi
  on hi
    reply "hi"
'
norm() { sed -e "s/0\.1\.4/<version>/g" -e "s#$1#<tmp>#g"; }
for b in "$BLOCKS"/*; do
  name=$(basename "$b")
  tmp=$(mktemp -d)
  case "$name" in
    *.ag) cp "$b" "$tmp/block.ag"; target=block.ag ;;
    *.toml) cp "$b" "$tmp/metagente.toml"; printf '%s' "$HELLO" > "$tmp/hello.ag"; target=hello.ag ;;
  esac
  set +e
  (cd "$tmp" && "$BIN" check "$target" >"$tmp/.out" 2>"$tmp/.err")
  code=$?
  set -e
  { echo "exit: $code"; echo "--- stdout"; norm "$tmp" <"$tmp/.out"; echo "--- stderr"; norm "$tmp" <"$tmp/.err"; } >"$GOLDEN/$name.txt"
  rm -rf "$tmp"
done

# The files of examples/ and of every sample of multi-agent-samples/, as TRACKED by git (so nothing a person generated
# on this computer, like certs/ or valid-tokens.txt, changes the result). Each .ag file is checked from the folder of its
# project, by its file name, the way the integration test does it.
check_folder() {
  src="$1"; label="$2"
  tmp=$(mktemp -d)
  git ls-files "$src" | while read -r f; do
    rel="${f#"$src"/}"
    mkdir -p "$tmp/$(dirname "$rel")"
    cp "$f" "$tmp/$rel"
  done
  for ag in "$tmp"/*.ag; do
    [ -e "$ag" ] || continue
    base=$(basename "$ag")
    set +e
    (cd "$tmp" && "$BIN" check "$base" >"$tmp/.out" 2>"$tmp/.err")
    code=$?
    set -e
    { echo "exit: $code"; echo "--- stdout"; norm "$tmp" <"$tmp/.out"; echo "--- stderr"; norm "$tmp" <"$tmp/.err"; } >"$GOLDEN/$label-$base.txt"
  done
  rm -rf "$tmp"
}
check_folder examples examples
for d in multi-agent-samples/*/; do
  name=$(basename "$d")
  check_folder "multi-agent-samples/$name" "samples-$name"
done

# The examples that RUN offline with a result that does not depend on the clock, the network or a tool server: what they
# print (`metagente run <file> <message> <values>`), from a folder that has a notes.txt for read_file.ag. The ones left
# out (clock.ag, weather.ag, planner.ag, remote_weather.ag, parameters.ag) need the time, an MCP server or another agent.
run_example() {
  file="$1"; shift
  tmp=$(mktemp -d)
  git ls-files examples | while read -r f; do
    rel="${f#examples/}"
    mkdir -p "$tmp/$(dirname "$rel")"
    cp "$f" "$tmp/$rel"
  done
  printf 'hello notes\n' > "$tmp/notes.txt"
  set +e
  (cd "$tmp" && "$BIN" run "$file" "$@" >"$tmp/.out" 2>"$tmp/.err")
  code=$?
  set -e
  { echo "exit: $code"; echo "--- stdout"; norm "$tmp" <"$tmp/.out"; echo "--- stderr"; norm "$tmp" <"$tmp/.err"; } >"$GOLDEN/run-examples-$file.txt"
  rm -rf "$tmp"
}
run_example broken_permission.ag
run_example broken_syntax.ag
run_example broken_undeclared_tool.ag
run_example cycle_a.ag
run_example cycle_b.ag
run_example multiline.ag screen resume=text
run_example read_file.ag summarize
echo "blocks: $(ls "$BLOCKS" | wc -l), goldens: $(ls "$GOLDEN" | wc -l)"
