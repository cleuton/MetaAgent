"""An independent A2A client: the official Python SDK (pip install -r tests/interop/requirements.txt).

Usage: python a2a_sdk_client.py BASE_URL [--message "ask city=Lisbon"] [--insecure | --ca FILE] [--proxy URL]
Prints one JSON object describing what the SDK saw. Used by tests/integration/a2a_interop.rs (SC-006)
and tests/integration/python_interop.rs (0.1.3).

0.1.3: --insecure skips certificate checks (a self-signed test server), --ca trusts one authority,
--proxy sends everything through a proxy (the login may be inside the address, as is usual for Python tools).
"""
import argparse
import asyncio
import json

import httpx
from a2a.client import A2ACardResolver, ClientConfig, create_client
from a2a.types.a2a_pb2 import GetTaskRequest, Message, Part, Role, SendMessageRequest, TaskState


def text_message(text):
    return SendMessageRequest(
        message=Message(message_id="interop-1", role=Role.ROLE_USER, parts=[Part(text=text)])
    )


async def last_task(client, request):
    task = None
    async for response in client.send_message(request):
        if response.HasField("task"):
            task = response.task
    return task


async def main(args):
    verify = False if args.insecure else (args.ca if args.ca else True)
    out = {}
    async with httpx.AsyncClient(verify=verify, proxy=args.proxy) as http:
        card = await A2ACardResolver(http, args.base).get_agent_card()
        out["card_name"] = card.name
        out["skills"] = [s.id for s in card.skills]
        out["interfaces"] = [(i.protocol_binding, i.protocol_version) for i in card.supported_interfaces]
        out["endpoint"] = card.supported_interfaces[0].url if card.supported_interfaces else ""

        client = await create_client(card, ClientConfig(httpx_client=http))
        task = await last_task(client, text_message(args.message))
        out["state"] = TaskState.Name(task.status.state)
        out["text"] = task.artifacts[0].parts[0].text if task.artifacts else ""

        fetched = await client.get_task(GetTaskRequest(id=task.id))
        out["get_task_state"] = TaskState.Name(fetched.status.state)
        out["get_task_text"] = fetched.artifacts[0].parts[0].text if fetched.artifacts else ""

        try:
            await last_task(client, text_message("dance"))
            out["unsupported_error"] = None
        except Exception as e:  # the SDK raises its own error type
            out["unsupported_error"] = str(e)
    print(json.dumps(out))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("base")
    parser.add_argument("--message", default="ask city=Lisbon")
    parser.add_argument("--insecure", action="store_true")
    parser.add_argument("--ca")
    parser.add_argument("--proxy")
    asyncio.run(main(parser.parse_args()))
