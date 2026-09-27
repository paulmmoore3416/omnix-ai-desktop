"""Command line: ``python -m kb_core <command>``.

``serve`` runs the service. Every other command is a thin HTTP client of a
running service, so there is exactly one writer and the in-memory vector
index is always authoritative.

    serve                           run the service (default)
    status                          health + analytics summary
    collections                     list collections (knowledge bases)
    search QUERY [-n 8] [--json]    hybrid search
    remember TEXT [-t tag,...] [-i 7] [-c category]
    extract TEXT [--dry-run]        capture facts from text with the local LLM
    ingest DIR [--dry-run]          index a folder once (watching: KB_CORE_WATCH)
    sync                            re-scan watched folders now
    connectors                      list knowledge connectors (folder, obsidian, plugins)
    export [FILE]                   NDJSON backup (stdout by default)
    import FILE                     restore/merge an export
    maintenance                     consolidate duplicates, backfill, compact
    mcp (--collections a,b | --all [--exclude x]) [--allow-write]
                                    MCP server on stdio for Claude Code & co.
    migrate-legacy DB               import memories from the kb-core 1.x database
    encrypt                         encrypt the database at rest (service stopped)
    decrypt                         turn an encrypted database back into plain SQLite
"""

from __future__ import annotations

import argparse
import json
import logging
import os
import sqlite3
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

from . import __version__
from .config import Config


def _client(args: argparse.Namespace):  # noqa: ANN202
    base = args.url.rstrip("/")
    token = None
    if args.token_file:
        token = Path(args.token_file).expanduser().read_text(encoding="utf-8").strip()

    def call(method: str, path: str, body: Any = None, raw: bool = False, timeout: float = 600) -> Any:
        headers = {"Content-Type": "application/json"}
        if token:
            headers["Authorization"] = f"Bearer {token}"
        data = json.dumps(body).encode() if body is not None else None
        req = urllib.request.Request(base + path, data=data, headers=headers, method=method)
        try:
            with urllib.request.urlopen(req, timeout=timeout) as r:
                # Read inside the `with`: the response is closed on exit.
                return r.read() if raw else json.load(r)
        except urllib.error.HTTPError as e:
            try:
                msg = json.load(e).get("error", "")
            except ValueError:
                msg = ""
            raise SystemExit(f"kb-core: {e.code} {msg}".rstrip()) from None
        except urllib.error.URLError as e:
            raise SystemExit(
                f"kb-core is not reachable at {base} ({e.reason}); "
                "check: systemctl --user status omnix-kb-core"
            ) from None

    return call


def cmd_serve(args: argparse.Namespace) -> int:
    from .llm import OllamaChat, OllamaEmbedder
    from .server import serve
    from .store import KnowledgeBase
    from .sync import FolderSync

    cfg = Config.from_env()
    cfg.validate()
    nli = None
    if cfg.nli_model:
        from .llm import ModelUnavailable
        from .nli import NliJudge

        try:
            nli = NliJudge(cfg.nli_model)
        except ModelUnavailable as e:
            # Optional: the LLM judge (if any) keeps working on its own.
            logging.getLogger("kb-core").warning("NLI judge off: %s", e)
    kb = KnowledgeBase(
        cfg,
        OllamaEmbedder(cfg.ollama, cfg.embed_model),
        OllamaChat(cfg.ollama, cfg.llm_model) if cfg.llm_model else None,
        nli,
    )
    kb.start_worker()
    if cfg.connector_plugins:
        from .connectors import load_plugins

        load_plugins(cfg.connector_plugins)
    sync = FolderSync(kb, cfg.watch, cfg.watch_interval)
    sync.start()
    logging.getLogger("kb-core").info(
        "kb-core %s: embed=%s llm=%s nli=%s encrypted=%s watch=%s", __version__, cfg.embed_model,
        cfg.llm_model or "off", nli.model if nli else "off", "yes" if cfg.db_key else "no",
        ", ".join(f"{c + '=' if c else ''}{p}" for c, p in cfg.watch) or "off",
    )
    serve(cfg, kb, sync)
    return 0


def _fmt_hit(h: dict[str, Any]) -> str:
    src = "memory" if h.get("kind") == "memory" else h.get("source", "document")
    sim = h.get("similarity")
    head = f"[{h['score']:.2f}{f' · cos {sim:.2f}' if sim is not None else ''}] {src}"
    body = h["content"].strip().replace("\n", "\n    ")
    return f"{head}\n    {body[:600]}{'…' if len(body) > 600 else ''}"


def cmd_status(args: argparse.Namespace) -> int:
    s = _client(args)("GET", "/stats")
    if args.json:
        print(json.dumps(s, indent=2))
        return 0
    print(f"kb-core {s.get('version')} — {s['status']} (up {s['uptime_s'] // 60} min)")
    print(f"  memories     {s['memories']}  (superseded {s['superseded']}, links {s['links']})")
    print(f"  documents    {s['documents']}  ({s['chunks']} chunks)")
    print(f"  embeddings   {s['embed_model']} · {s['vector_dimensions']}d · {s['vectors']} vectors "
          f"· {s['vector_backend']} · pending {s['pending_embeddings']}")
    print(f"  llm          {s.get('llm_model') or 'off (extraction and contradiction checks disabled)'}")
    print(f"  storage      {s['storage_bytes'] / 1048576:.1f} MiB")
    if s.get("searches_24h"):
        print(f"  searches 24h {s['searches_24h']} (avg {s['avg_search_ms']} ms)")
    w = s.get("watch") or {}
    for f in w.get("folders", []):
        r = (w.get("last_result") or {}).get(f)
        err = (w.get("errors") or {}).get(f)
        print(f"  watching     {f}" + (f"  ERROR {err}" if err else f"  ({r['files']} files)" if r else ""))
    if s.get("embed_error"):
        print(f"  ! embedding model unavailable: {s['embed_error']}")
    if s.get("categories"):
        print("  categories   " + ", ".join(f"{k} {v}" for k, v in s["categories"].items()))
    return 0


def cmd_search(args: argparse.Namespace) -> int:
    r = _client(args)("POST", "/search", {"query": args.query, "limit": args.limit, "track": False})
    if args.json:
        print(json.dumps(r, indent=2, ensure_ascii=False))
        return 0
    if r.get("degraded"):
        print("(embedding model unavailable: keyword results only)")
    for h in r["results"]:
        print(_fmt_hit(h))
    if not r["results"]:
        print("no results")
    return 0


def cmd_remember(args: argparse.Namespace) -> int:
    tags = [t for t in (args.tags or "").split(",") if t.strip()]
    r = _client(args)("POST", "/memories", {"content": args.text, "tags": tags, "importance": args.importance,
                                             "category": args.category})
    print(f"{r['status']}: {r['id']}")
    for rel in r.get("related", []):
        print(f"  related ({rel['score']:.2f}): {rel['content'][:120]}")
    return 0


def cmd_extract(args: argparse.Namespace) -> int:
    r = _client(args)("POST", "/extract", {"text": args.text, "dry_run": args.dry_run})
    for m in r["memories"]:
        print(f"{m['status']:<10} {m['content']}")
    if not r["memories"]:
        print("nothing worth remembering found")
    return 0


def cmd_ingest(args: argparse.Namespace) -> int:
    from .connectors import connector_for, load_plugins
    from .sync import doc_name

    plugins = Config.from_env().connector_plugins
    if plugins:
        load_plugins(plugins)
    root = Path(args.folder).expanduser().resolve()
    if not root.is_dir():
        print(f"not a directory: {root}", file=sys.stderr)
        return 2
    conn = connector_for(root)
    print(f"connector: {conn.name} ({conn.description})")
    files = list(conn.iter_files(root))
    if args.dry_run:
        for f in files:
            print(doc_name(root, f))
        print(f"{len(files)} files")
        return 0
    call = _client(args)
    totals = {"created": 0, "updated": 0, "unchanged": 0, "failed": 0}
    for n, f in enumerate(files, 1):
        name = doc_name(root, f)
        text = conn.read(f)
        if text is None:
            print(f"[{n}/{len(files)}] skip {name}: not readable as text")
            totals["failed"] += 1
            continue
        body = {"name": name, "content": text, "mime_type": conn.mime(f)}
        if args.collection:
            body["collection"] = args.collection
        r = call("POST", "/documents", body)
        totals[r.get("status", "created")] = totals.get(r.get("status", "created"), 0) + 1
        print(f"[{n}/{len(files)}] {r.get('status', 'ok'):<9} {name} ({r['chunks']} chunks)")
    print(", ".join(f"{k} {v}" for k, v in totals.items()))
    return 1 if totals["failed"] else 0


def cmd_connectors(args: argparse.Namespace) -> int:
    from .connectors import available, load_plugins

    plugins = Config.from_env().connector_plugins
    if plugins:
        load_plugins(plugins)
    for c in available():
        print(f"{c.name:<12} {c.description}")
    return 0


def cmd_collections(args: argparse.Namespace) -> int:
    for c in _client(args)("GET", "/collections")["collections"]:
        print(f"{c['name']:<20} {c['memories']:>5} memories  {c['documents']:>5} documents  {c['description']}")
    return 0


def cmd_sync(args: argparse.Namespace) -> int:
    print(json.dumps(_client(args)("POST", "/sync"), indent=2))
    return 0


def cmd_export(args: argparse.Namespace) -> int:
    lines = _client(args)("GET", "/export", raw=True).splitlines(keepends=True)
    if args.file in (None, "-"):
        for line in lines:
            sys.stdout.write(line.decode())
        return 0
    # The export holds personal data: write it private.
    fd = os.open(args.file, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    n = 0
    with os.fdopen(fd, "wb") as out:
        for line in lines:
            out.write(line)
            n += 1
    print(f"wrote {n} records to {args.file}")
    return 0


def cmd_import(args: argparse.Namespace) -> int:
    with open(args.file, encoding="utf-8") as fh:
        recs = [json.loads(line) for line in fh if line.strip()]
    print(json.dumps(_client(args)("POST", "/import", {"records": recs}), indent=2))
    return 0


def cmd_maintenance(args: argparse.Namespace) -> int:
    print(json.dumps(_client(args)("POST", "/maintenance"), indent=2))
    return 0


def cmd_mcp(args: argparse.Namespace) -> int:
    from .mcp import McpServer, Scope, http_client, run

    split = lambda v: [c.strip().lower() for c in (v or "").split(",") if c.strip()]  # noqa: E731
    allow, exclude = split(args.collections), split(args.exclude)
    if bool(allow) == bool(args.all):
        # Scope is a deliberate choice: never expose every knowledge base by default.
        print("kb-core mcp: choose --collections a,b or --all (optionally with --exclude x,y)", file=sys.stderr)
        return 2
    if exclude and not args.all:
        print("kb-core mcp: --exclude only applies with --all", file=sys.stderr)
        return 2
    scope = Scope(allow=allow, exclude=exclude, everything=args.all, write=args.allow_write)
    # stdout carries the protocol; everything else goes to stderr.
    logging.getLogger().handlers[:] = [logging.StreamHandler(sys.stderr)]
    logging.getLogger("kb_core.mcp").info(
        "kb-core MCP on stdio: %s, %s", "all" + (f" except {','.join(exclude)}" if exclude else "") if args.all
        else ",".join(allow), "read-write" if args.allow_write else "read-only")
    run(McpServer(http_client(args.url, args.token_file), scope))
    return 0


def cmd_migrate_legacy(args: argparse.Namespace) -> int:
    """Import memories from a kb-core 1.x SQLite file (``services/kb-core``).

    Documents are not migrated: 1.x kept only chunks, not the source text.
    Re-index the folders (or add them to KB_CORE_WATCH) instead.
    """
    path = Path(args.db).expanduser()
    db = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    try:
        rows = db.execute("SELECT content, tags, importance, category, created_at FROM memories").fetchall()
        docs = [r[0] for r in db.execute("SELECT name FROM documents")]
    except sqlite3.DatabaseError as e:
        raise SystemExit(f"{path} is not a kb-core 1.x database: {e}") from None
    finally:
        db.close()
    recs = [
        {"type": "memory", "content": c, "tags": json.loads(t or "[]"), "importance": i, "category": cat,
         "created_at": ts}
        for c, t, i, cat, ts in rows
    ]
    result = _client(args)("POST", "/import", {"records": recs}) if recs else {}
    print(f"memories: {len(recs)} read, {result}")
    if docs:
        print(f"{len(docs)} documents were indexed in 1.x; re-index their folders (not migrated).")
    return 0


def _require_stopped(args: argparse.Namespace) -> Path:
    """Conversion rewrites the file under the service: refuse while it runs."""
    try:
        urllib.request.urlopen(args.url.rstrip("/") + "/health", timeout=2).close()
    except urllib.error.HTTPError:
        pass  # something answered (e.g. 401): it is running
    except (urllib.error.URLError, OSError):
        from .config import default_data_dir

        return Path(os.environ.get("KB_CORE_DB") or default_data_dir() / "kb.sqlite3").expanduser()
    raise SystemExit("kb-core is running: stop it first (systemctl --user stop omnix-kb-core)")


def cmd_encrypt(args: argparse.Namespace) -> int:
    from .crypto import convert, create_key, is_plain_sqlite

    path = _require_stopped(args)
    if not path.exists():
        raise SystemExit(f"{path} does not exist yet: start kb-core once, or set KB_CORE_ENCRYPTION=keyring "
                         "before the first start to create it encrypted")
    if not is_plain_sqlite(path):
        print(f"{path} is already encrypted")
        return 0
    key = create_key()
    items, docs, links = convert(path, None, key)
    print(f"encrypted {path} ({items} items, {docs} documents, {links} links); key in the OS keyring. "
          "Set KB_CORE_ENCRYPTION=keyring in kb-core.env.")
    return 0


def cmd_decrypt(args: argparse.Namespace) -> int:
    from .crypto import convert, is_plain_sqlite, load_key

    path = _require_stopped(args)
    if is_plain_sqlite(path):
        print(f"{path} is not encrypted")
        return 0
    items, docs, links = convert(path, load_key(), None)
    print(f"decrypted {path} ({items} items, {docs} documents, {links} links). "
          "Remove KB_CORE_ENCRYPTION from kb-core.env.")
    return 0


def main(argv: list[str] | None = None) -> int:
    logging.basicConfig(
        level=os.environ.get("KB_CORE_LOG", "INFO").upper(),
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
    )
    p = argparse.ArgumentParser(prog="kb-core", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--version", action="version", version=f"kb-core {__version__}")
    p.add_argument("--url", default=os.environ.get("KB_CORE_URL", f"http://127.0.0.1:{os.environ.get('KB_CORE_PORT', '8100')}"))
    p.add_argument("--token-file", default=os.environ.get("KB_CORE_TOKEN_FILE"))
    sub = p.add_subparsers(dest="cmd")
    sub.add_parser("serve").set_defaults(fn=cmd_serve)
    s = sub.add_parser("status")
    s.add_argument("--json", action="store_true")
    s.set_defaults(fn=cmd_status)
    s = sub.add_parser("search")
    s.add_argument("query")
    s.add_argument("-n", "--limit", type=int, default=8)
    s.add_argument("--json", action="store_true")
    s.set_defaults(fn=cmd_search)
    s = sub.add_parser("remember")
    s.add_argument("text")
    s.add_argument("-t", "--tags")
    s.add_argument("-i", "--importance", type=int, default=6)
    s.add_argument("-c", "--category", default="general")
    s.set_defaults(fn=cmd_remember)
    s = sub.add_parser("extract")
    s.add_argument("text")
    s.add_argument("--dry-run", action="store_true")
    s.set_defaults(fn=cmd_extract)
    s = sub.add_parser("ingest")
    s.add_argument("folder")
    s.add_argument("-c", "--collection", help="file the documents into this collection")
    s.add_argument("--dry-run", action="store_true")
    s.set_defaults(fn=cmd_ingest)
    sub.add_parser("sync").set_defaults(fn=cmd_sync)
    sub.add_parser("connectors", help="list knowledge connectors").set_defaults(fn=cmd_connectors)
    sub.add_parser("collections").set_defaults(fn=cmd_collections)
    s = sub.add_parser("export")
    s.add_argument("file", nargs="?")
    s.set_defaults(fn=cmd_export)
    s = sub.add_parser("import")
    s.add_argument("file")
    s.set_defaults(fn=cmd_import)
    sub.add_parser("maintenance").set_defaults(fn=cmd_maintenance)
    sub.add_parser("encrypt").set_defaults(fn=cmd_encrypt)
    sub.add_parser("decrypt").set_defaults(fn=cmd_decrypt)
    s = sub.add_parser("mcp", help="MCP server on stdio")
    s.add_argument("--collections", help="comma-separated knowledge bases to expose (default = plain memories)")
    s.add_argument("--all", action="store_true", help="expose every knowledge base (see --exclude)")
    s.add_argument("--exclude", help="with --all: comma-separated knowledge bases to keep hidden")
    s.add_argument("--allow-write", action="store_true", help="also offer the remember tool")
    s.set_defaults(fn=cmd_mcp)
    s = sub.add_parser("migrate-legacy")
    s.add_argument("db")
    s.set_defaults(fn=cmd_migrate_legacy)
    args = p.parse_args(argv)
    return getattr(args, "fn", cmd_serve)(args)


if __name__ == "__main__":
    sys.exit(main())
