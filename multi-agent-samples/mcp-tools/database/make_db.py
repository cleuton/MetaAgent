import sqlite3
from pathlib import Path

# shop.db is made next to this file. An old one is replaced.
path = Path(__file__).parent / "shop.db"
path.unlink(missing_ok=True)

db = sqlite3.connect(path)
db.execute(
    "CREATE TABLE orders (id INTEGER PRIMARY KEY, customer TEXT NOT NULL, item TEXT NOT NULL, quantity INTEGER NOT NULL)"
)
db.executemany(
    "INSERT INTO orders (customer, item, quantity) VALUES (?, ?, ?)",
    [
        ("Ana", "pencil", 3),
        ("Ana", "notebook", 2),
        ("Bruno", "pencil", 1),
        ("Ana", "eraser", 5),
        ("Carla", "notebook", 4),
    ],
)
db.commit()
db.close()
print("made", path.name)
