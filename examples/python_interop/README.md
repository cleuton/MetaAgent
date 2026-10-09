# Metagente and Python, calling each other

Two small examples, one for each direction. Both use the standard A2A protocol, so nothing here is special
to Metagente: the Python side is the official Python A2A SDK (`a2a-sdk`).

You need Python 3.11 or newer and the `metagente` program. About 10 minutes.

## 1. Set up Python once

Open a terminal in the **top folder of the project** (the one that has `Cargo.toml`) and run:

```bash
python3 -m venv .venv
source .venv/bin/activate            # Windows: .venv\Scripts\activate
pip install -r examples/python_interop/requirements.txt
```

## 2. Metagente calls a Python agent

Terminal A (leave it running):

```bash
cd examples/python_interop
python python_agent.py --port 9100
```

Terminal B:

```bash
cd examples/python_interop
metagente run caller.ag shout_it text=hello
```

You should see `Pyra says: HELLO!`. The Python agent offers two skills, `echo` and `shout`. `caller.ag` has just one
line that says where it is: `remote Pyra at "http://127.0.0.1:9100"`.

## 3. A Python program calls a Metagente agent

Stop the Python agent (Ctrl-C), then, in terminal A:

```bash
cd examples/python_interop
metagente serve greeter.ag --a2a 8081
```

Terminal B:

```bash
cd examples/python_interop
python python_client.py http://127.0.0.1:8081 --message "greet name=Ana"
```

The answer is printed as a small JSON text; look for `"text": "Hello, Ana! This greeting came from Metagente."`.

## If the other agent uses https or a proxy

Put the settings in `metagente.toml` (never in the `.ag` file). A self-signed certificate while developing:

```toml
[network]
allow_self_signed = true     # development only: Metagente prints a warning every time
```

A company proxy that needs a login, where `PROXY_USER` and `PROXY_PASSWORD` are environment variables you set:

```toml
[network.proxy]
url = "http://proxy.company.com:3128"
username_env = "PROXY_USER"
password_env = "PROXY_PASSWORD"
```

The main README explains every setting in the section "Secure connections".
