# 0.1.4: the validator: the token arrives on standard input, the exit code is the answer (0 = accepted)
import sys

token = sys.stdin.readline().strip()
with open("valid-tokens.txt") as f:
    accepted = {line.strip() for line in f if line.strip()}
sys.exit(0 if token in accepted else 1)
