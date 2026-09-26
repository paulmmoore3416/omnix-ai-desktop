"""HTTP API (standard library ``http.server``).

Routes (see ``docs/kb-core-contract.md``; the first six are the contract
OMNIX relies on, the rest are kb-core extensions):

    GET    /health                     liveness + counts
    GET    /memories?limit&offset&category&include_superseded&hidden_only&order
    POST   /memories                   save (consolidating)
    DELETE /memories/{id}
    POST   /search                     hybrid search
    POST   /documents                  index / replace by name
    GET    /memories/{id}              one memory with its links
    PATCH  /memories/{id}              edit, pin, restore a superseded memory
    POST   /memories/{id}/feedback     {"helpful": bool}: recall feedback (penalty)
    GET    /documents                  list
    GET    /documents/{id}             content + chunks
    DELETE /documents/{id}
    POST   /extract                    capture facts from conversation text
    GET    /stats                      analytics
    GET    /export                     NDJSON dump (memories + documents)
    POST   /import                     {"records": [...]} from an export
    POST   /maintenance                consolidate, backfill, compact
    POST   /sync                       re-scan watched folders now
    GET    /collections                knowledge bases with counts
    POST   /collections                {"name", "description"}
    DELETE /collections/{name}         delete one and everything in it
    POST   /documents/batch            {"documents": [{name, content, mime_type, collection}]}
    GET    /metrics                    Prometheus text format

Browser hardening. The service listens on loopback, but any web page the
user visits can make its browser send requests to 127.0.0.1. So:

* without a token, the ``Host`` header must name this service (defeats DNS
  rebinding; with a token a rebinding page can't authenticate anyway);
* requests carrying an ``Origin`` header are refused (browsers always add
  it to cross-origin requests; OMNIX's Rust client never sends one);
* bodies must be ``application/json`` (a "simple" cross-site form post
  can't be), and no CORS headers are ever sent.
"""

from __future__ import annotations

import hmac
import json
import logging
import re
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any, Callable
from urllib.parse import parse_qs, urlparse

from .config import Config
from .store import BadRequest, KbError, KnowledgeBase, NotFound, check_id
from .sync import FolderSync

log = logging.getLogger("kb-core.http")

MAX_BODY = 8 * 1024 * 1024  # OMNIX sends documents up to 5 MiB (+ JSON escaping)
MAX_IMPORT_BODY = 64 * 1024 * 1024


class Api:
    """Routes requests to the knowledge base. Separate from the handler so
    tests can call it without sockets."""

    def __init__(self, kb: KnowledgeBase, sync: FolderSync | None = None) -> None:
        self.kb = kb
        self.sync = sync
        self.routes: list[tuple[str, re.Pattern[str], Callable[..., Any]]] = [
            ("GET", re.compile(r"/health"), self.health),
            ("GET", re.compile(r"/stats"), lambda q, b: self.kb.stats()),
            ("GET", re.compile(r"/memories"), self.list_memories),
            ("POST", re.compile(r"/memories"), self.save_memory),
            ("GET", re.compile(r"/memories/([^/]+)"), lambda q, b, i: self.kb.get_memory(i)),
            ("PATCH", re.compile(r"/memories/([^/]+)"), self.patch_memory),
            ("DELETE", re.compile(r"/memories/([^/]+)"), self.delete_memory),
            ("POST", re.compile(r"/memories/([^/]+)/feedback"),
             lambda q, b, i: self.kb.feedback(i, self._obj(b).get("helpful"))),
            ("POST", re.compile(r"/search"), self.search),
            ("GET", re.compile(r"/documents"), lambda q, b: {"documents": self.kb.list_documents(q.get("collection", [None])[0])}),
            ("POST", re.compile(r"/documents"), self.index_document),
            ("GET", re.compile(r"/documents/([^/]+)"), lambda q, b, i: self.kb.get_document(i)),
            ("DELETE", re.compile(r"/documents/([^/]+)"), self.delete_document),
            ("POST", re.compile(r"/extract"), self.extract),
            ("POST", re.compile(r"/import"), self.import_),
            ("POST", re.compile(r"/maintenance"), lambda q, b: self.kb.maintenance()),
            ("POST", re.compile(r"/sync"), self.sync_now),
            ("GET", re.compile(r"/collections"), lambda q, b: {"collections": self.kb.list_collections()}),
            ("POST", re.compile(r"/collections"), self.create_collection),
            ("DELETE", re.compile(r"/collections/([^/]+)"), lambda q, b, n: self.kb.delete_collection(n)),
            ("POST", re.compile(r"/documents/batch"), self.index_batch),
        ]

    def dispatch(self, method: str, path: str, query: dict[str, list[str]], body: Any) -> tuple[int, Any]:
        path = path.rstrip("/") or "/"
        allowed = False
        for m, rx, fn in self.routes:
            match = rx.fullmatch(path)
            if not match:
                continue
            allowed = True
            if m != method:
                continue
            try:
                return 200, fn(query, body, *match.groups())
            except KbError as e:
                return e.status, {"error": str(e)}
        return (405, {"error": "method not allowed"}) if allowed else (404, {"error": "no such endpoint"})

    # -- helpers

    @staticmethod
    def _obj(body: Any) -> dict[str, Any]:
        if not isinstance(body, dict):
            raise BadRequest("body must be a JSON object")
        return body

    @staticmethod
    def _int(q: dict[str, list[str]], key: str, default: int) -> int:
        try:
            return int(q.get(key, [default])[0])
        except (TypeError, ValueError):
            raise BadRequest(f"{key} must be an integer") from None

    @staticmethod
    def _flag(q: dict[str, list[str]], key: str) -> bool:
        return q.get(key, ["false"])[0].lower() in {"1", "true", "yes"}

    # -- handlers

    def health(self, q: Any, b: Any) -> dict[str, Any]:
        return self.kb.health()

    def list_memories(self, q: dict[str, list[str]], b: Any) -> dict[str, Any]:
        return {
            "memories": self.kb.list_memories(
                limit=self._int(q, "limit", 50),
                offset=max(0, self._int(q, "offset", 0)),
                category=q.get("category", [None])[0],
                include_superseded=self._flag(q, "include_superseded"),
                order=q.get("order", ["recent"])[0],
                collection=q.get("collection", [None])[0],
                hidden_only=self._flag(q, "hidden_only"),
            )
        }

    def save_memory(self, q: Any, b: Any) -> dict[str, Any]:
        b = self._obj(b)
        return self.kb.save_memory(
            b.get("content"), b.get("tags"), b.get("importance"), b.get("category"),
            source=b.get("source") if isinstance(b.get("source"), str) else "user",
            collection=b.get("collection"),
        )

    def patch_memory(self, q: Any, b: Any, item_id: str) -> dict[str, Any]:
        return self.kb.update_memory(item_id, dict(self._obj(b)))

    def delete_memory(self, q: Any, b: Any, item_id: str) -> dict[str, Any]:
        self.kb.delete_memory(item_id)
        return {"deleted": item_id}

    def search(self, q: Any, b: Any) -> dict[str, Any]:
        b = self._obj(b)
        return self.kb.search(
            b.get("query"), b.get("limit", 5), kinds=b.get("kinds"), tags=b.get("tags"),
            category=b.get("category"), min_score=b.get("min_score", 0.0), mode=b.get("mode", "hybrid"),
            include_superseded=bool(b.get("include_superseded", False)), track=bool(b.get("track", True)),
            collections=b.get("collections"), exclude_collections=b.get("exclude_collections"),
        )

    def index_document(self, q: Any, b: Any) -> dict[str, Any]:
        b = self._obj(b)
        return self.kb.index_document(b.get("name"), b.get("content"), b.get("mime_type", "text/plain"),
                                      collection=b.get("collection"))

    def index_batch(self, q: Any, b: Any) -> dict[str, Any]:
        docs = self._obj(b).get("documents")
        if not isinstance(docs, list) or not 1 <= len(docs) <= 500:
            raise BadRequest("documents must be a list of 1–500 documents")
        results = []
        for d in docs:
            if not isinstance(d, dict):
                results.append({"error": "not an object"})
                continue
            try:
                results.append(self.kb.index_document(d.get("name"), d.get("content"), d.get("mime_type", "text/plain"),
                                                      collection=d.get("collection")))
            except KbError as e:
                results.append({"name": d.get("name"), "error": str(e)})
        return {"results": results}

    def create_collection(self, q: Any, b: Any) -> dict[str, Any]:
        b = self._obj(b)
        return self.kb.create_collection(b.get("name"), b.get("description", ""))

    def delete_document(self, q: Any, b: Any, doc_id: str) -> dict[str, Any]:
        self.kb.delete_document(doc_id)
        return {"deleted": doc_id}

    def extract(self, q: Any, b: Any) -> dict[str, Any]:
        b = self._obj(b)
        return self.kb.extract(b.get("text"), dry_run=bool(b.get("dry_run", False)), context=b.get("context"))

    def import_(self, q: Any, b: Any) -> dict[str, Any]:
        b = self._obj(b)
        recs = b.get("records")
        if not isinstance(recs, list):
            raise BadRequest("records must be a list")
        return self.kb.import_records(recs)

    def sync_now(self, q: Any, b: Any) -> dict[str, Any]:
        if self.sync is None or not self.sync.roots:
            raise NotFound("no watched folders are configured (KB_CORE_WATCH)")
        return {"folders": self.sync.run_once()}


def make_handler(cfg: Config, api: Api) -> type[BaseHTTPRequestHandler]:
    allowed_hosts = {
        "127.0.0.1", "localhost", "[::1]", cfg.host.lower(),
        *(h for h in cfg.allowed_hosts),
    }

    class Handler(BaseHTTPRequestHandler):
        server_version = "kb-core"
        sys_version = ""
        protocol_version = "HTTP/1.1"

        def log_message(self, fmt: str, *args: Any) -> None:  # route through logging, no bodies
            log.debug("%s %s", self.address_string(), fmt % args)

        def _send(self, status: int, payload: Any) -> None:
            data = json.dumps(payload, ensure_ascii=False).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Cache-Control", "no-store")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.end_headers()
            self.wfile.write(data)

        def _guard(self) -> bool:
            host = (self.headers.get("Host") or "").lower()
            hostname = host.rsplit(":", 1)[0] if not host.endswith("]") else host
            # With a token, a rebinding page can't authenticate anyway, and LAN
            # clients legitimately use the server's IP or name.
            if not cfg.token and hostname not in allowed_hosts:
                self._send(421, {"error": "unexpected Host header"})
                return False
            if self.headers.get("Origin") is not None:
                self._send(403, {"error": "browser requests are not accepted"})
                return False
            if cfg.token:
                auth = self.headers.get("Authorization", "")
                given = auth[7:] if auth.startswith("Bearer ") else ""
                if not hmac.compare_digest(given.encode(), cfg.token.encode()):
                    self._send(401, {"error": "missing or invalid bearer token"})
                    return False
            return True

        def _body(self, path: str) -> tuple[bool, Any]:
            length = int(self.headers.get("Content-Length") or 0)
            if length == 0:
                return True, None
            limit = MAX_IMPORT_BODY if path.rstrip("/") == "/import" else MAX_BODY
            if length > limit:
                self._send(413, {"error": f"body larger than {limit} bytes"})
                self.close_connection = True
                return False, None
            ctype = (self.headers.get("Content-Type") or "").split(";")[0].strip().lower()
            if ctype != "application/json":
                self._send(415, {"error": "Content-Type must be application/json"})
                self.close_connection = True
                return False, None
            try:
                return True, json.loads(self.rfile.read(length))
            except (ValueError, UnicodeDecodeError):
                self._send(400, {"error": "invalid JSON"})
                return False, None

        def _handle(self, method: str) -> None:
            t0 = time.perf_counter()
            if not self._guard():
                return
            url = urlparse(self.path)
            ok, body = self._body(url.path)
            if not ok:
                return
            if method == "GET" and url.path.rstrip("/") == "/export":
                self._export()
                return
            if method == "GET" and url.path.rstrip("/") == "/metrics":
                self._metrics()
                return
            for part in url.path.split("/")[2:3]:
                # ids are interpolated nowhere, but reject junk early anyway
                try:
                    check_id(part)
                except BadRequest as e:
                    self._send(400, {"error": str(e)})
                    return
            try:
                status, payload = api.dispatch(method, url.path, parse_qs(url.query), body)
            except Exception:  # noqa: BLE001 - never leak a traceback to the client
                log.exception("unhandled error on %s %s", method, url.path)
                status, payload = 500, {"error": "internal error (see the kb-core log)"}
            self._send(status, payload)
            log.info("%s %s -> %d (%.0f ms)", method, url.path, status, (time.perf_counter() - t0) * 1000)

        def _export(self) -> None:
            self.send_response(200)
            self.send_header("Content-Type", "application/x-ndjson; charset=utf-8")
            self.send_header("Content-Disposition", 'attachment; filename="kb-core-export.jsonl"')
            self.send_header("Cache-Control", "no-store")
            self.send_header("Connection", "close")
            self.end_headers()
            self.close_connection = True
            for rec in api.kb.export():
                self.wfile.write(json.dumps(rec, ensure_ascii=False).encode() + b"\n")

        def _metrics(self) -> None:
            data = prometheus(api.kb).encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; version=0.0.4; charset=utf-8")
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self) -> None:  # noqa: N802
            self._handle("GET")

        def do_POST(self) -> None:  # noqa: N802
            self._handle("POST")

        def do_PATCH(self) -> None:  # noqa: N802
            self._handle("PATCH")

        def do_DELETE(self) -> None:  # noqa: N802
            self._handle("DELETE")

        def do_OPTIONS(self) -> None:  # noqa: N802 - never answer CORS preflights
            self._send(405, {"error": "method not allowed"})

    return Handler


def prometheus(kb: KnowledgeBase) -> str:
    """Operational metrics (counts and timings only, never content)."""
    h = kb.health()
    c = kb.counters
    lines = [
        "# HELP kb_core_items Stored items by kind.",
        "# TYPE kb_core_items gauge",
        f'kb_core_items{{kind="memory"}} {h["memories"]}',
        f'kb_core_items{{kind="chunk"}} {h["chunks"]}',
        "# TYPE kb_core_documents gauge",
        f"kb_core_documents {h['documents']}",
        "# TYPE kb_core_collections gauge",
        f"kb_core_collections {h['collections']}",
        "# TYPE kb_core_pending_embeddings gauge",
        f"kb_core_pending_embeddings {h['pending_embeddings']}",
        "# TYPE kb_core_vectors gauge",
        f"kb_core_vectors {len(kb.index)}",
        "# TYPE kb_core_index_memory_bytes gauge",
        f"kb_core_index_memory_bytes {kb.index.memory_bytes}",
        "# TYPE kb_core_storage_bytes gauge",
        f"kb_core_storage_bytes {kb.storage_bytes()}",
        "# TYPE kb_core_embedding_up gauge",
        f"kb_core_embedding_up {0 if h['status'] == 'degraded' else 1}",
        "# TYPE kb_core_searches_total counter",
        f"kb_core_searches_total {c['searches']}",
        "# TYPE kb_core_search_seconds_total counter",
        f"kb_core_search_seconds_total {c['search_ms'] / 1000:.3f}",
        "# TYPE kb_core_memory_saves_total counter",
        f"kb_core_memory_saves_total {c['saves']}",
        "# TYPE kb_core_documents_indexed_total counter",
        f"kb_core_documents_indexed_total {c['documents']}",
        "# TYPE kb_core_uptime_seconds gauge",
        f"kb_core_uptime_seconds {int(time.time() - kb.started)}",
    ]
    return "\n".join(lines) + "\n"


def serve(cfg: Config, kb: KnowledgeBase, sync: FolderSync | None) -> None:
    api = Api(kb, sync)
    httpd = ThreadingHTTPServer((cfg.host, cfg.port), make_handler(cfg, api))
    httpd.daemon_threads = True
    log.info("kb-core listening on http://%s:%d (db %s)", cfg.host, cfg.port, cfg.db_path)
    try:
        httpd.serve_forever()
    finally:
        kb.stop()
        if sync:
            sync.stop()
        httpd.server_close()
