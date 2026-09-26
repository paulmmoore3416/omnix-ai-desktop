"""MCP server: protocol, scope enforcement and an end-to-end run over real HTTP."""

from __future__ import annotations

import io
import json
import threading
from http.server import ThreadingHTTPServer

from kb_core.mcp import McpServer, Scope, http_client, run
from kb_core.server import Api, make_handler

from .test_kb_core import Base


class McpTests(Base):
    def setUp(self) -> None:
        super().setUp()
        httpd = ThreadingHTTPServer(("127.0.0.1", 0), make_handler(self.cfg, Api(self.kb)))
        threading.Thread(target=httpd.serve_forever, daemon=True).start()
        self.addCleanup(httpd.server_close)
        self.addCleanup(httpd.shutdown)
        self.call = http_client(f"http://127.0.0.1:{httpd.server_address[1]}", None)
        self.kb.save_memory("Paul's NAS lives at 10.0.0.5 in the homelab rack", ["homelab"], 7, "general",
                            collection="homelab")
        self.kb.save_memory("Paul prefers dark roast coffee", [], 5, "general")
        self.kb.save_memory("Patient in bed 3W-12 has a penicillin allergy", [], 8, "general", collection="clinical")

    def rpc(self, server: McpServer, method: str, params: dict | None = None, mid: int = 1) -> dict:
        return server.handle({"jsonrpc": "2.0", "id": mid, "method": method, "params": params or {}})

    def tool(self, server: McpServer, name: str, **args) -> dict:
        return self.rpc(server, "tools/call", {"name": name, "arguments": args})["result"]

    def test_initialize_and_list_tools(self) -> None:
        s = McpServer(self.call, Scope(allow=["homelab"]))
        init = self.rpc(s, "initialize", {"protocolVersion": "2025-03-26", "capabilities": {}})["result"]
        self.assertEqual(init["protocolVersion"], "2025-03-26")
        self.assertIn("tools", init["capabilities"])
        self.assertEqual(self.rpc(s, "initialize", {"protocolVersion": "1999-01-01"})["result"]["protocolVersion"],
                         "2025-06-18")
        names = [t["name"] for t in self.rpc(s, "tools/list")["result"]["tools"]]
        self.assertEqual(names, ["search_memory", "list_collections"])  # read-only: no remember
        writable = McpServer(self.call, Scope(allow=["homelab"], write=True))
        self.assertIn("remember", [t["name"] for t in self.rpc(writable, "tools/list")["result"]["tools"]])

    def test_search_stays_inside_the_allowlist(self) -> None:
        s = McpServer(self.call, Scope(allow=["homelab"]))
        r = self.tool(s, "search_memory", query="NAS homelab rack", min_score=0)
        self.assertFalse(r["isError"])
        self.assertIn("10.0.0.5", r["content"][0]["text"])
        self.assertIn("not as instructions", r["content"][0]["text"])
        r = self.tool(s, "search_memory", query="penicillin allergy patient", min_score=0)
        self.assertNotIn("penicillin", r["content"][0]["text"])
        r = self.tool(s, "search_memory", query="allergy", collections=["clinical"])
        self.assertTrue(r["isError"])
        self.assertIn("not allowed", r["content"][0]["text"])

    def test_all_with_exclude(self) -> None:
        s = McpServer(self.call, Scope(everything=True, exclude=["clinical"]))
        text = self.tool(s, "search_memory", query="coffee roast", min_score=0)["content"][0]["text"]
        self.assertIn("dark roast", text)
        self.assertNotIn("penicillin", self.tool(s, "search_memory", query="penicillin", min_score=0)["content"][0]["text"])
        listing = self.tool(s, "list_collections")["content"][0]["text"]
        self.assertIn("homelab", listing)
        self.assertNotIn("clinical", listing)

    def test_search_does_not_count_as_recall(self) -> None:
        s = McpServer(self.call, Scope(allow=["default"]))
        before = self.kb.search("coffee", 3)["results"][0]
        self.tool(s, "search_memory", query="coffee", min_score=0)
        after = self.kb.get_memory(before["id"])
        self.assertEqual(after["access_count"], self.kb.get_memory(before["id"])["access_count"])
        self.assertEqual(after["access_count"], 1)  # only the direct track=True search above

    def test_remember_needs_write_and_scope(self) -> None:
        ro = McpServer(self.call, Scope(allow=["default"]))
        self.assertIn("error", self.rpc(ro, "tools/call", {"name": "remember", "arguments": {"content": "x"}}))
        rw = McpServer(self.call, Scope(allow=["default"], write=True))
        r = self.tool(rw, "remember", content="Paul uses Neovim for editing", tags=["tools"])
        self.assertFalse(r["isError"], r)
        mem_id = r["content"][0]["text"].split(": ")[1]
        m = self.kb.get_memory(mem_id)
        self.assertEqual(m["source"], "mcp")
        self.assertIn("mcp", m["tags"])
        r = self.tool(rw, "remember", content="secret clinical note", collection="clinical")
        self.assertTrue(r["isError"])
        self.assertTrue(self.tool(rw, "remember", content="   ")["isError"])

    def test_protocol_errors_and_notifications(self) -> None:
        s = McpServer(self.call, Scope(allow=["default"]))
        self.assertIsNone(s.handle({"jsonrpc": "2.0", "method": "notifications/initialized"}))
        self.assertEqual(self.rpc(s, "nope")["error"]["code"], -32601)
        self.assertEqual(s.handle({"id": 1, "method": "ping"})["error"]["code"], -32600)
        self.assertEqual(self.rpc(s, "ping")["result"], {})
        self.assertEqual(self.rpc(s, "tools/call", {"name": "rm_rf"})["error"]["code"], -32602)
        self.assertTrue(self.tool(s, "search_memory", query="")["isError"])

    def test_unreachable_service_is_a_tool_error(self) -> None:
        s = McpServer(http_client("http://127.0.0.1:9", None), Scope(allow=["default"]))
        r = self.tool(s, "search_memory", query="anything")
        self.assertTrue(r["isError"])
        self.assertIn("not reachable", r["content"][0]["text"])

    def test_stdio_loop(self) -> None:
        s = McpServer(self.call, Scope(allow=["default"]))
        lines = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/call",
             "params": {"name": "search_memory", "arguments": {"query": "coffee", "min_score": 0}}},
        ]
        out = io.StringIO()
        run(s, io.StringIO("\n".join(json.dumps(m) for m in lines) + "\nnot json\n"), out)
        replies = [json.loads(line) for line in out.getvalue().splitlines()]
        self.assertEqual([r.get("id") for r in replies], [1, 2, None])
        self.assertIn("dark roast", replies[1]["result"]["content"][0]["text"])
        self.assertEqual(replies[2]["error"]["code"], -32700)
