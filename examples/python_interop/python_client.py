"""Calls an agent from Python: python python_client.py http://127.0.0.1:8081 --message "greet name=Ana"

The client itself is in tests/interop/a2a_sdk_client.py. It uses the official Python A2A SDK.
"""
import pathlib
import runpy

here = pathlib.Path(__file__).resolve().parent
runpy.run_path(str(here.parent.parent / "tests" / "interop" / "a2a_sdk_client.py"), run_name="__main__")
