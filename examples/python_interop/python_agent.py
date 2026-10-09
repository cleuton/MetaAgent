"""Starts the Python agent used by this example: python python_agent.py --port 9100

The agent itself is in tests/interop/python_agent/agent.py. It was written with the official Python A2A SDK
and offers two skills, `echo` and `shout`.
"""
import pathlib
import runpy

here = pathlib.Path(__file__).resolve().parent
runpy.run_path(str(here.parent.parent / "tests" / "interop" / "python_agent" / "agent.py"), run_name="__main__")
