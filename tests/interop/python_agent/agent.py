"""0.1.3: a small A2A agent built with the official Python SDK (a2a-sdk), used by the interop tests.

It offers two skills, `echo` and `shout`. Metagente agents call it with:

    remote Pyra at "http://127.0.0.1:9100"
    ... ask Pyra.echo text: "hello"

Usage:
    python agent.py --port 9100 [--tls-cert cert.pem --tls-key key.pem]

Prints `READY <base url>` on standard output when it is listening.
"""
import argparse

import uvicorn
from a2a.server.agent_execution import AgentExecutor, RequestContext
from a2a.server.events import EventQueue
from a2a.server.request_handlers import DefaultRequestHandler
from a2a.server.routes import create_agent_card_routes, create_jsonrpc_routes
from a2a.helpers.proto_helpers import new_task_from_user_message
from a2a.server.tasks import InMemoryTaskStore, TaskUpdater
from a2a.types.a2a_pb2 import (
    AgentCapabilities,
    AgentCard,
    AgentInterface,
    AgentSkill,
    Part,
)
from google.protobuf.json_format import MessageToDict
from starlette.applications import Starlette

SKILLS = {
    "echo": "Gives back the text it was given, with the prefix `echo: `",
    "shout": "Gives back the text in upper case with an exclamation mark",
}


def build_card(base_url: str) -> AgentCard:
    return AgentCard(
        name="Pyra",
        description="A Python A2A agent used to test Metagente",
        version="1.0.0",
        supported_interfaces=[
            AgentInterface(url=base_url + "/", protocol_binding="JSONRPC", protocol_version="1.0")
        ],
        capabilities=AgentCapabilities(streaming=False),
        default_input_modes=["text/plain", "application/json"],
        default_output_modes=["text/plain"],
        skills=[
            AgentSkill(id=name, name=name, description=text, tags=["interop"])
            for name, text in SKILLS.items()
        ],
    )


def wanted(context: RequestContext) -> tuple[str, str]:
    """The skill and the text of a request: from a Metagente data part, or from plain text."""
    message = context.message
    skill = ""
    text = ""
    if message is not None:
        for part in message.parts:
            if part.HasField("data"):
                data = MessageToDict(part.data)
                if isinstance(data, dict):
                    skill = str(data.get("skill", skill))
                    arguments = data.get("arguments") or {}
                    text = str(arguments.get("text", text))
        meta = MessageToDict(message.metadata) if message.HasField("metadata") else {}
        skill = skill or str(meta.get("skill", ""))
    if not text:
        text = context.get_user_input()
    return skill or "echo", text


class PyraExecutor(AgentExecutor):
    async def execute(self, context: RequestContext, event_queue: EventQueue) -> None:
        updater = TaskUpdater(event_queue, context.task_id, context.context_id)
        skill, text = wanted(context)
        if context.current_task is None and context.message is not None:
            await event_queue.enqueue_event(new_task_from_user_message(context.message))
        if skill == "echo":
            answer = "echo: " + text
        elif skill == "shout":
            answer = text.upper() + "!"
        else:
            await updater.failed(updater.new_agent_message([Part(text=f"I do not offer `{skill}`")]))
            return
        await updater.add_artifact([Part(text=answer)])
        await updater.complete()

    async def cancel(self, context: RequestContext, event_queue: EventQueue) -> None:
        await TaskUpdater(event_queue, context.task_id, context.context_id).cancel()


def build_app(base_url: str) -> Starlette:
    card = build_card(base_url)
    handler = DefaultRequestHandler(
        agent_executor=PyraExecutor(), task_store=InMemoryTaskStore(), agent_card=card
    )
    routes = create_agent_card_routes(card) + create_jsonrpc_routes(handler, "/")
    return Starlette(routes=routes)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=9100)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--tls-cert")
    parser.add_argument("--tls-key")
    args = parser.parse_args()
    scheme = "https" if args.tls_cert else "http"
    base = f"{scheme}://{args.host}:{args.port}"
    config = uvicorn.Config(
        build_app(base),
        host=args.host,
        port=args.port,
        ssl_certfile=args.tls_cert,
        ssl_keyfile=args.tls_key,
        log_level="warning",
    )
    server = uvicorn.Server(config)

    original = server.startup

    async def startup(sockets=None):
        await original(sockets)
        print("READY " + base, flush=True)

    server.startup = startup
    server.run()


if __name__ == "__main__":
    main()
