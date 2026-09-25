#!/usr/bin/env python3
"""Minimal MCP stdio server used by OMNIX tests. Exposes one tool, `echo`."""
import json
import sys


def reply(msg_id, result):
    sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": msg_id, "result": result}) + "\n")
    sys.stdout.flush()


for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    msg = json.loads(line)
    method, msg_id = msg.get("method"), msg.get("id")
    if method == "initialize":
        reply(msg_id, {
            "protocolVersion": msg["params"]["protocolVersion"],
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "echo", "version": "0.1.0"},
        })
    elif method == "tools/list":
        reply(msg_id, {"tools": [{
            "name": "echo",
            "description": "Echo the text back",
            "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]},
        }]})
    elif method == "tools/call":
        text = msg["params"].get("arguments", {}).get("text", "")
        reply(msg_id, {"content": [{"type": "text", "text": f"echo: {text}"}], "isError": False})
    elif msg_id is not None:
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": msg_id, "error": {"code": -32601, "message": "not found"}}) + "\n")
        sys.stdout.flush()
