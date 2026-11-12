#!/usr/bin/env python3
"""Test a calc tool server without an agent.

Usage:  python3 ../smoke.py -- <the command that starts the server>

It starts the server, does the handshake that Metagente does, lists the tools,
makes the fixed calls and prints one line for each. A correct server gives the
same lines in every language. Uses only the Python standard library.
"""
import json
import queue
import subprocess
import sys
import threading

PROTOCOL_VERSION = "2026-07-28"  # the version Metagente 0.1.5 sends
WAIT_SECONDS = 60

EMPTY = "values must not be empty"
SIZE = "vectors must have the same size"

# What every correct server announces: name -> (description, {property: (type text, required)})
TOOLS = {
    "dot": ("Dot product of two lists of numbers of the same size.",
            {"a": "array of number", "b": "array of number"}),
    "mean": ("Average of a list of numbers.", {"values": "array of number"}),
    "scale": ("Every number of the list multiplied by the factor.",
              {"factor": "number", "values": "array of number"}),
}

# The fixed calls: (tool, arguments, expected result or the sentence of the error)
CALLS = [
    ("mean", {"values": [2, 4, 4, 4, 5, 5, 7, 9]}, {"result": 5}),
    ("mean", {"values": [1, 2, 3, 4]}, {"result": 2.5}),
    ("dot", {"a": [1, 2, 3], "b": [4, 5, 6]}, {"result": 32}),
    ("scale", {"values": [1, 2, 3], "factor": 2}, {"result": [2, 4, 6]}),
    ("mean", {"values": []}, EMPTY),
    ("dot", {"a": [1, 2], "b": [1]}, SIZE),
    ("dot", {"a": [], "b": []}, EMPTY),
    ("scale", {"values": [], "factor": 2}, EMPTY),
]


def plain(value):
    """A number with no fraction becomes a whole number (5.0 -> 5)."""
    if isinstance(value, bool):
        return value
    if isinstance(value, float) and value == int(value):
        return int(value)
    if isinstance(value, list):
        return [plain(v) for v in value]
    if isinstance(value, dict):
        return {k: plain(v) for k, v in value.items()}
    return value


def show(value):
    return json.dumps(plain(value), sort_keys=True, separators=(", ", ": "))


def compact(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def type_text(prop):
    kind = prop.get("type")
    if kind == "array":
        items = prop.get("items") or {}
        if items.get("type") == "number":
            return "array of number"
        return "array of " + compact(items)
    if kind == "number":
        return "number"
    return compact(kind)


class Server:
    def __init__(self, command):
        self.process = subprocess.Popen(
            command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, universal_newlines=True, bufsize=1)
        RUNNING.append(self.process)
        self.lines = queue.Queue()
        self.next_id = 0
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        for line in self.process.stdout:
            self.lines.put(line)
        self.lines.put(None)

    def send(self, message):
        self.process.stdin.write(json.dumps(message) + "\n")
        self.process.stdin.flush()

    def ask(self, method, params):
        self.next_id += 1
        wanted = self.next_id
        self.send({"jsonrpc": "2.0", "id": wanted, "method": method, "params": params})
        while True:
            try:
                line = self.lines.get(timeout=WAIT_SECONDS)
            except queue.Empty:
                stop("I gave up: the server did not answer `%s` in %d seconds." % (method, WAIT_SECONDS))
            if line is None:
                stop("The server ended before it answered `%s`." % method)
            try:
                message = json.loads(line)
            except ValueError:
                stop("I will not test this server: it wrote text to its standard output that is not "
                     "a protocol message:\n  %s\nA tool must write nothing but protocol there. "
                     "Print messages for yourself to the standard error instead." % line.strip())
            if isinstance(message, dict) and message.get("id") == wanted:
                return message


RUNNING = []


def stop(message):
    print(message)
    for process in RUNNING:
        process.kill()
    sys.exit(2)


def main():
    if "--" not in sys.argv:
        stop("Usage: python3 smoke.py -- <the command that starts the server>")
    command = sys.argv[sys.argv.index("--") + 1:]
    if not command:
        stop("Usage: python3 smoke.py -- <the command that starts the server>")
    try:
        server = Server(command)
    except OSError as error:
        stop("I could not start the server `%s`: %s" % (" ".join(command), error))

    passed = 0
    total = 0

    def report(ok, line):
        nonlocal passed, total
        total += 1
        passed += 1 if ok else 0
        print(line)

    server.ask("initialize", {"protocolVersion": PROTOCOL_VERSION, "capabilities": {},
                              "clientInfo": {"name": "smoke.py", "version": "1.0.0"}})
    server.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
    listed = server.ask("tools/list", {}).get("result", {}).get("tools", [])
    by_name = {tool.get("name"): tool for tool in listed}

    for name in sorted(set(TOOLS) | set(by_name)):
        tool = by_name.get(name)
        if tool is None:
            report(False, "FAIL tool %s: the server does not have it" % name)
            continue
        schema = tool.get("inputSchema") or {}
        required = schema.get("required") or []
        properties = schema.get("properties") or {}
        parts = ["%s: %s (%s)" % (key, type_text(properties[key]), "required" if key in required else "optional")
                 for key in sorted(properties)]
        line = "tool %s: %s | %s" % (name, tool.get("description", ""), ", ".join(parts))
        if name not in TOOLS:
            report(False, "FAIL " + line + " (a tool that the contract does not have)")
            continue
        description, wanted = TOOLS[name]
        expected = "tool %s: %s | %s" % (
            name, description, ", ".join("%s: %s (required)" % (k, wanted[k]) for k in sorted(wanted)))
        report(line == expected, line if line == expected else "FAIL %s\n     expected %s" % (line, expected))

    for name, arguments, expected in CALLS:
        call = "%s %s" % (name, show(arguments))
        answer = server.ask("tools/call", {"name": name, "arguments": arguments})
        result = answer.get("result")
        if result is None:
            report(False, "FAIL %s -> the server answered with a protocol error, not a result: %s"
                   % (call, compact(answer.get("error"))))
            continue
        texts = [c.get("text", "") for c in result.get("content", []) if c.get("type") == "text"]
        if isinstance(expected, str):
            if result.get("isError") is True and any(expected in t for t in texts):
                report(True, '%s -> error containing "%s"' % (call, expected))
            else:
                report(False, 'FAIL %s -> expected an error containing "%s", got %s'
                       % (call, expected, compact(result)))
            continue
        structured = result.get("structuredContent")
        if result.get("isError") or structured is None or len(texts) != 1:
            report(False, "FAIL %s -> expected structuredContent and one text block, got %s" % (call, compact(result)))
            continue
        try:
            same = plain(json.loads(texts[0])) == plain(structured)
        except ValueError:
            same = False
        if not same:
            report(False, "FAIL %s -> structuredContent %s and the text block %s are not the same JSON"
                   % (call, show(structured), json.dumps(texts[0])))
        elif plain(structured) != plain(expected):
            report(False, "FAIL %s -> got %s, expected %s" % (call, show(structured), show(expected)))
        else:
            report(True, "%s -> %s" % (call, show(structured)))

    print("%d of %d checks passed" % (passed, total))
    try:
        server.process.stdin.close()
        server.process.terminate()
    except OSError:
        pass
    sys.exit(0 if passed == total else 1)


main()
