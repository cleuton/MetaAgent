from typing import TypedDict

from mcp.server import MCPServer
from mcp.server.mcpserver.exceptions import ToolError

# The name and the version are what this server tells Metagente about itself.
# log_level keeps the SDK's routine messages (they go to the standard error) out of the way.
mcp = MCPServer("calc", version="1.0.0", log_level="WARNING")


# Every tool answers with an object that has one field, "result".
class Number(TypedDict):
    result: float


class Numbers(TypedDict):
    result: list[float]


@mcp.tool()
def mean(values: list[float]) -> Number:
    """Average of a list of numbers."""
    if not values:
        raise ToolError("values must not be empty")
    return {"result": sum(values) / len(values)}


@mcp.tool()
def dot(a: list[float], b: list[float]) -> Number:
    """Dot product of two lists of numbers of the same size."""
    if len(a) != len(b):
        raise ToolError("vectors must have the same size")
    if not a:
        raise ToolError("values must not be empty")
    return {"result": sum(x * y for x, y in zip(a, b))}


@mcp.tool()
def scale(values: list[float], factor: float) -> Numbers:
    """Every number of the list multiplied by the factor."""
    if not values:
        raise ToolError("values must not be empty")
    return {"result": [v * factor for v in values]}


if __name__ == "__main__":
    # Talk to Metagente through standard input and output.
    # Never print anything else to standard output: it would break the conversation.
    mcp.run()
