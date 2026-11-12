import sqlite3
from pathlib import Path
from typing import TypedDict

from mcp.server import MCPServer

# log_level keeps the SDK's routine messages (they go to the standard error) out of the way.
mcp = MCPServer("shop", version="1.0.0", log_level="WARNING")

# The database is next to this file. It is opened for reading only.
DATABASE = "file:" + str(Path(__file__).parent / "shop.db") + "?mode=ro"


class Item(TypedDict):
    item: str
    quantity: int


class Items(TypedDict):
    result: list[Item]


class Total(TypedDict):
    result: int


# The queries are fixed. The customer is only ever a value of the query (the "?"), never part of the SQL text.
@mcp.tool()
def orders_of(customer: str) -> Items:
    """The items and quantities that a customer ordered."""
    db = sqlite3.connect(DATABASE, uri=True)
    rows = db.execute("SELECT item, quantity FROM orders WHERE customer = ? ORDER BY id", (customer,)).fetchall()
    db.close()
    return {"result": [{"item": item, "quantity": quantity} for item, quantity in rows]}


@mcp.tool()
def total_items(customer: str) -> Total:
    """The total quantity that a customer ordered."""
    db = sqlite3.connect(DATABASE, uri=True)
    (total,) = db.execute("SELECT COALESCE(SUM(quantity), 0) FROM orders WHERE customer = ?", (customer,)).fetchone()
    db.close()
    return {"result": total}


if __name__ == "__main__":
    # Talk to Metagente through standard input and output. Print nothing else there.
    mcp.run()
