"""An independent A2A client: the official Python SDK (pip install a2a-sdk).

Usage: python a2a_sdk_client.py BASE_URL
Prints one JSON object describing what the SDK saw. Used by tests/integration/a2a_interop.rs (SC-006).
"""
import asyncio
import json
import sys

import httpx
from a2a.client import A2ACardResolver, create_client
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


async def main(base):
    out = {}
    async with httpx.AsyncClient() as http:
        card = await A2ACardResolver(http, base).get_agent_card()
    out["card_name"] = card.name
    out["skills"] = [s.id for s in card.skills]
    out["interfaces"] = [(i.protocol_binding, i.protocol_version) for i in card.supported_interfaces]

    client = await create_client(base)
    task = await last_task(client, text_message("ask city=Lisbon"))
    out["state"] = TaskState.Name(task.status.state)
    out["text"] = task.artifacts[0].parts[0].text

    fetched = await client.get_task(GetTaskRequest(id=task.id))
    out["get_task_state"] = TaskState.Name(fetched.status.state)
    out["get_task_text"] = fetched.artifacts[0].parts[0].text

    try:
        await last_task(client, text_message("dance"))
        out["unsupported_error"] = None
    except Exception as e:  # the SDK raises its own error type
        out["unsupported_error"] = str(e)
    print(json.dumps(out))


asyncio.run(main(sys.argv[1]))
