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

# 0.1.3: https in the bare container too. The certificate is made here, on the host, with openssl; the container has no
# CA certificates at all, so it can only work if the binary carries what it needs. One `metagente serve` answers over
# https, and a second `metagente run` in the same container calls it, trusting the certificate with ca_file.
mkdir -p "$WORK/certs"
openssl req -x509 -newkey rsa:2048 -nodes -days 365 -subj "/CN=localhost" \
  -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" -addext "basicConstraints=critical,CA:FALSE" \
  -keyout "$WORK/certs/key.pem" -out "$WORK/certs/cert.pem" >/dev/null 2>&1
cat > "$WORK/greeter.ag" <<'AG'
agent Greeter
  goal "Say hello"
  accepts greet name
  on greet
    reply "hello {name} over https"
AG
cat > "$WORK/caller.ag" <<'AG'
agent Caller
  goal "Call the greeter over https"
  remote Greeter at "https://127.0.0.1:8443"
  accepts go
  on go
    reply Greeter.greet name: "container"
AG
cat > "$WORK/metagente.toml" <<'TOML'
[secure]
agents = ["Greeter", "Caller"]

[serve]
tls_cert = "certs/cert.pem"
tls_key = "certs/key.pem"

[network]
ca_file = "certs/cert.pem"
TOML
out="$(docker run --rm -v "$WORK":/pkg -w /pkg debian:stable-slim sh -c './metagente serve greeter.ag --a2a 8443 >/dev/null 2>&1 & sleep 2; ./metagente run caller.ag go')"
echo "$out"
case "$out" in
  "hello container over https") echo "https packaging check passed" ;;
  *) echo "https packaging check FAILED" >&2; exit 1 ;;
esac
