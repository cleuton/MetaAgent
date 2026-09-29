#!/bin/sh
# Simple packaging check (constitution: "Simple packaging").
# Copies the binary and a folder with two linked agents into a bare Debian container that has nothing
# installed (not even CA certificates) and runs them there.
# Usage: scripts/packaging-check.sh [path-to-metagente-binary]
set -eu
BIN="${1:-target/release/metagente}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/agents"
cp "$BIN" "$WORK/metagente"
cat > "$WORK/planner.ag" <<'AG'
agent Planner
  goal "Plan a trip using the helper"
  link Packer
  tool clock
  accepts plan city
  on plan
    list = Packer.pack city: city
    when = clock.now
    reply "{list} (planned {when.text})"
AG
cat > "$WORK/agents/Packer.ag" <<'AG'
agent Packer
  goal "Say what to pack"
  accepts pack city
  on pack
    reply "For {city}: sunscreen and a hat"
AG
out="$(docker run --rm -v "$WORK":/pkg -w /pkg debian:stable-slim ./metagente run planner.ag plan city=Lisbon)"
echo "$out"
case "$out" in
  "For Lisbon: sunscreen and a hat (planned "*) echo "packaging check passed" ;;
  *) echo "packaging check FAILED" >&2; exit 1 ;;
esac
