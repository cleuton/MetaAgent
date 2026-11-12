# A tool that does what Metagente cannot: a database

Metagente has no database. A tool can have anything its language has. Here a small Python tool reads orders from a SQLite
database, and the agent asks it questions. The explanation is in the [guide](../../../docs/mcp-tools-guide.md).

* `make_db.py`: makes `shop.db` next to itself (a table `orders` with five rows). It uses only what comes with Python.
* `server.py`: the tool server, with two tools: `orders_of` (the items and quantities of a customer) and `total_items`
  (their sum). The queries are fixed and the customer is only a value of the query. The connection is read only. No SQL
  ever travels from the agent to the tool.
* `shop.ag`: the agent. It has two messages, `items` and `total`.
* `metagente.toml`: says which program is the tool (`shop_command`).
* `requirements.txt`: the pinned `mcp` package, the same as the Python project.

## Before you start

You need `metagente` version **0.1.5 or newer** and Python **3.10 or newer** (`python3 --version`; `py --version` on
Windows). Open a terminal in this folder (`multi-agent-samples/mcp-tools/database`) and run every command from here.

## Step 1: make the virtual environment, install the package, make the database

**Linux and macOS**

```text
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
python3 make_db.py
```

**Windows (PowerShell)**

```text
py -m venv .venv
.venv\Scripts\pip install -r requirements.txt
py make_db.py
```

## Step 2: ask

```text
metagente run shop.ag items customer=Ana
```

```text
[
  {
    "item": "pencil",
    "quantity": 3
  },
  {
    "item": "notebook",
    "quantity": 2
  },
  {
    "item": "eraser",
    "quantity": 5
  }
]
```

```text
metagente run shop.ag total customer=Ana
```

```text
10
```

A customer with no orders gives an empty answer:

```text
metagente run shop.ag total customer=Nobody
```

```text
0
```

```text
metagente run shop.ag items customer=Nobody
```

```text
[]
```

## Break it on purpose

Try a name written to look like an attack. It is only a name, so nobody has that name:

```text
metagente run shop.ag items "customer=Ana' OR '1'='1"
```

```text
[]
```

The text is never part of the SQL, so it cannot change what the query does.

## Clean up

Delete the `.venv` folder and `shop.db`.
