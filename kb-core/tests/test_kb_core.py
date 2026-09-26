"""kb-core tests. Offline: a bag-of-words fake embedder and a fake LLM.

    cd kb-core && python -m unittest -v
"""

from __future__ import annotations

import array
import hashlib
import http.client
import json
import os
import re
import sqlite3
import tempfile
import threading
import time
import unittest
from http.server import ThreadingHTTPServer
from pathlib import Path
from typing import Any

from kb_core.chunking import chunk_code, chunk_document, split_text
from kb_core.config import Config
from kb_core.llm import ModelUnavailable
from kb_core.server import Api, make_handler
from kb_core.store import BadRequest, KnowledgeBase, NotConfigured, NotFound, fts_query, personalize
from kb_core.sync import FolderSync, sync_folder
from kb_core.vectors import VectorIndex, dot, normalise

DIM = 256


class FakeEmbedder:
    """Hashed bag of words: texts sharing words have high cosine."""

    def __init__(self, model: str = "fake-embed", dim: int = DIM) -> None:
        self.model = model
        self.dim = dim
        self.down = False
        self.calls = 0
        self.texts = 0

    def _vec(self, text: str) -> array.array:
        v = [0.0] * self.dim
        for w in re.findall(r"\w+", text.lower()):
            v[int(hashlib.md5(w.encode()).hexdigest(), 16) % self.dim] += 1.0
        return normalise(v if any(v) else [1.0] + [0.0] * (self.dim - 1))

    def documents(self, texts: list[str]) -> list[array.array]:
        if self.down:
            raise ModelUnavailable("fake model is down")
        self.calls += 1
        self.texts += len(texts)
        return [self._vec(t) for t in texts]

    def query(self, text: str) -> array.array:
        if self.down:
            raise ModelUnavailable("fake model is down")
        return self._vec(text)

    def queries(self, texts: list[str]) -> list[array.array]:
        return [self.query(t) for t in texts]


class FakeChat:
    model = "fake-llm"

    def __init__(self, reply: Any) -> None:
        self.reply = reply
        self.prompts: list[str] = []

    def json(self, system: str, user: str, schema: dict[str, Any], timeout: float = 0) -> Any:
        self.prompts.append(user)
        return self.reply(user) if callable(self.reply) else self.reply


class Base(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.cfg = Config(db_path=Path(self.tmp.name) / "kb.sqlite3", duplicate_threshold=0.95,
                          related_threshold=0.5)
        self.emb = FakeEmbedder()
        self.kb = KnowledgeBase(self.cfg, self.emb)
        self.addCleanup(lambda: self.kb.close())

    def reopen(self, embedder: FakeEmbedder | None = None, chat: Any = None) -> KnowledgeBase:
        self.kb.close()
        self.emb = embedder or self.emb
        self.kb = KnowledgeBase(self.cfg, self.emb, chat)
        return self.kb


class ChunkingTests(unittest.TestCase):
    def test_breadcrumbs_and_fences(self) -> None:
        doc = "# Backups\nintro\n## ZFS\n```bash\n# not a heading\nzfs snapshot\n```\n## Restic\nuse restic\n"
        chunks = chunk_document("notes.md", doc)
        ctxs = [c.context for c in chunks]
        self.assertTrue(all(c.startswith("notes.md › Backups") for c in ctxs), ctxs)
        joined = "\n".join(c.text for c in chunks)
        self.assertIn("# not a heading", joined)
        self.assertNotIn("not a heading", " ".join(ctxs))

    def test_long_sections_split_with_overlap(self) -> None:
        text = "\n\n".join(f"Paragraph {i} " + "word " * 60 for i in range(40))
        parts = split_text(text, limit=800, overlap=150)
        self.assertGreater(len(parts), 5)
        self.assertTrue(all(len(p) <= 800 for p in parts))
        self.assertTrue(parts[1].startswith("…"))

    def test_small_children_merge_into_parent(self) -> None:
        doc = "# FAQ\n" + "".join(f"## Q{i}\nshort answer {i}\n" for i in range(5))
        chunks = chunk_document("faq.md", doc)
        self.assertEqual(len(chunks), 1)
        self.assertIn("Q3:", chunks[0].text)

    def test_plain_text_without_headings(self) -> None:
        chunks = chunk_document("a.txt", "just text")
        self.assertEqual([(c.context, c.text) for c in chunks], [("a.txt", "just text")])


class QueryTests(unittest.TestCase):
    def test_fts_query_is_quoted(self) -> None:
        self.assertEqual(fts_query('proxmox NEAR(backup) "x" col:*'), '"proxmox" OR "near" OR "backup" OR "col"')
        self.assertEqual(fts_query("what is it"), '"what" OR "is" OR "it"')
        self.assertIsNone(fts_query("?!"))

    def test_personalize(self) -> None:
        self.assertEqual(
            personalize("what are my skills and what am I good at?", "Paul"),
            ["what are my skills and what am I good at?",
             "what are Paul's skills and what am Paul good at?",
             "what are the user's skills and what am the user good at?"],
        )
        self.assertEqual(personalize("I'm moving; tell me", "Chris")[1], "Chris is moving; tell Chris")
        self.assertEqual(personalize("proxmox backups", "Paul"), ["proxmox backups"])
        self.assertEqual(len(personalize("my car", "")), 2)

    def test_vector_index(self) -> None:
        idx = VectorIndex()
        idx.add("a", normalise([1, 0, 0]))
        idx.add("b", normalise([0.9, 0.1, 0]))
        idx.add("c", normalise([0, 1, 0]))
        self.assertEqual([i for i, _ in idx.search(normalise([1, 0, 0]), 2)], ["a", "b"])
        self.assertEqual([i for i, _ in idx.search(normalise([1, 0, 0]), 5, allow={"c"})], ["c"])
        idx.remove(["a"])
        self.assertEqual(len(idx), 2)
        with self.assertRaises(ValueError):
            idx.add("d", normalise([1, 0]))


class MemoryTests(Base):
    def test_save_list_get_delete(self) -> None:
        r = self.kb.save_memory("Paul prefers morning meetings", ["Prefs", "#prefs"], 7, "preference")
        self.assertEqual(r["status"], "created")
        mems = self.kb.list_memories()
        self.assertEqual(mems[0]["tags"], ["prefs"])
        self.assertEqual(mems[0]["importance"], 7)
        self.assertTrue(mems[0]["embedded"])
        self.assertEqual(self.kb.get_memory(r["id"])["content"], "Paul prefers morning meetings")
        self.kb.delete_memory(r["id"])
        with self.assertRaises(NotFound):
            self.kb.delete_memory(r["id"])
        with self.assertRaises(BadRequest):
            self.kb.get_memory("../x")

    def test_validation(self) -> None:
        for bad in ("", "  ", None, 5, "x" * 20001):
            with self.assertRaises(BadRequest):
                self.kb.save_memory(bad)
        with self.assertRaises(BadRequest):
            self.kb.save_memory("ok", tags="notalist")
        with self.assertRaises(BadRequest):
            self.kb.save_memory("ok", tags=["x" * 65])
        self.assertEqual(self.kb.list_memories(), [])

    def test_duplicates_reinforce_instead_of_piling_up(self) -> None:
        a = self.kb.save_memory("Paul prefers morning meetings", ["a"], 5)
        b = self.kb.save_memory("paul prefers morning meetings", ["b"], 8)
        self.assertEqual(b["id"], a["id"])
        self.assertEqual(b["status"], "reinforced")
        m = self.kb.get_memory(a["id"])
        self.assertEqual((m["tags"], m["importance"], m["reinforced"]), (["a", "b"], 8, 1))
        self.assertEqual(len(self.kb.list_memories()), 1)

    def test_related_memories_are_linked(self) -> None:
        a = self.kb.save_memory("Paul runs Proxmox on the home server")
        b = self.kb.save_memory("The home server runs Proxmox with ZFS storage pools")
        self.assertEqual(b["status"], "created")
        self.assertEqual([r["id"] for r in b["related"]], [a["id"]])
        links = self.kb.get_memory(a["id"])["links"]
        self.assertEqual(links[0]["id"], b["id"])

    def test_save_survives_embedding_outage_and_backfills(self) -> None:
        self.emb.down = True
        r = self.kb.save_memory("The NAS lives in the basement rack")
        self.assertFalse(r["embedded"])
        again = self.kb.save_memory("The NAS lives in the basement rack")
        self.assertEqual(again["id"], r["id"], "exact duplicates are caught without vectors")
        hits = self.kb.search("basement NAS")
        self.assertTrue(hits["degraded"])
        self.assertEqual(hits["results"][0]["id"], r["id"])
        self.assertEqual(self.kb.health()["status"], "degraded")
        self.emb.down = False
        self.kb._embed_retry_at = 0
        self.assertEqual(self.kb.backfill(), 1)
        self.assertTrue(self.kb.get_memory(r["id"])["embedded"])
        self.assertEqual(self.kb.health()["pending_embeddings"], 0)

    def test_embedding_model_change_reembeds(self) -> None:
        self.kb.save_memory("Paul likes tea")
        kb = self.reopen(FakeEmbedder(model="other-embed", dim=128))
        self.assertEqual(kb.health()["pending_embeddings"], 1)
        self.assertEqual(len(kb.index), 0)
        kb.backfill()
        self.assertEqual(kb.index.dim, 128)
        self.assertEqual(kb.search("tea")["results"][0]["content"], "Paul likes tea")

    def test_patch_pin_and_restore(self) -> None:
        r = self.kb.save_memory("Paul lives in Springfield")
        m = self.kb.update_memory(r["id"], {"pinned": True, "tags": ["home"], "content": "Paul lives in Joplin"})
        self.assertTrue(m["pinned"])
        self.assertEqual(m["content"], "Paul lives in Joplin")
        self.assertEqual(self.kb.search("Joplin")["results"][0]["id"], r["id"])
        with self.assertRaises(BadRequest):
            self.kb.update_memory(r["id"], {"superseded_by": "m_x"})
        with self.assertRaises(BadRequest):
            self.kb.update_memory(r["id"], {"vec": "x"})

    def test_supersession_with_llm(self) -> None:
        old = self.kb.save_memory("Paul's favourite editor is Vim")
        kb = self.reopen(chat=FakeChat({"duplicate": [], "obsolete": [1]}))
        new = kb.save_memory("Paul's favourite editor is now VS Code, not Vim")
        self.assertEqual(new["related"][0]["id"], old["id"])
        job = kb._jobs.get_nowait()
        kb._judge_related(*job[1])
        self.assertEqual(kb.get_memory(old["id"])["superseded_by"], new["id"])
        ids = [h["id"] for h in kb.search("favourite editor")["results"]]
        self.assertNotIn(old["id"], ids)
        self.assertIn(old["id"], [h["id"] for h in kb.search("favourite editor", include_superseded=True)["results"]])
        kb.update_memory(old["id"], {"superseded_by": None})
        self.assertIsNone(kb.get_memory(old["id"])["superseded_by"])
        # Deleting the superseding memory also restores what it replaced.
        kb._judge_related(new["id"], [old["id"]])
        kb.delete_memory(new["id"])
        self.assertIsNone(kb.get_memory(old["id"])["superseded_by"])

    def test_llm_merges_paraphrases(self) -> None:
        old = self.kb.save_memory("Paul likes meetings in the morning", ["cal"], 8)
        self.kb.search("morning meetings")  # one recall
        kb = self.reopen(chat=FakeChat({"duplicate": [1], "obsolete": []}))
        new = kb.save_memory("Paul prefers morning meetings, before 10am", ["prefs"], 5)
        self.assertEqual(new["status"], "created")
        kb._judge_related(*kb._jobs.get_nowait()[1])
        [m] = kb.list_memories()
        self.assertEqual(m["id"], new["id"])
        self.assertEqual((m["tags"], m["importance"], m["reinforced"], m["access_count"]), (["prefs", "cal"], 8, 1, 1))

    def test_extract(self) -> None:
        with self.assertRaises(NotConfigured):
            self.kb.extract("hello")
        reply = {"memories": [
            {"content": "The user's daughter is named Ava", "category": "personal", "importance": 9, "tags": ["family"]},
            {"content": "The user's password is hunter2", "category": "personal", "importance": 9, "tags": []},
            {"content": "hi", "category": "general", "importance": 1, "tags": []},
        ]}
        kb = self.reopen(chat=FakeChat(reply))
        out = kb.extract("My daughter Ava starts school Monday. My password is hunter2")
        self.assertEqual([m["content"] for m in out["memories"]], ["The user's daughter is named Ava"])
        m = kb.list_memories()[0]
        self.assertEqual((m["source"], m["category"]), ("extract", "personal"))
        self.assertIn("auto", m["tags"])
        self.assertEqual(kb.extract("again", dry_run=True)["memories"][0]["status"], "proposed")
        self.assertEqual(len(kb.list_memories()), 1)


class PersonalizedSearchTests(Base):
    def test_first_person_query_finds_third_person_memory(self) -> None:
        self.cfg.user_name = "Paul"
        m = self.kb.save_memory("Paul's favourite editor is Neovim")["id"]
        self.kb.save_memory("The staging cluster runs Kubernetes")
        r = self.kb.search("what is my favourite editor", min_score=0.2)["results"]
        self.assertEqual(r[0]["id"], m)


class SearchTests(Base):
    def setUp(self) -> None:
        super().setUp()
        self.m1 = self.kb.save_memory("Paul prefers morning meetings before ten", ["prefs"], 8, "preference")["id"]
        self.m2 = self.kb.save_memory("The staging cluster runs Kubernetes 1.31", ["work"], 5, "work")["id"]
        self.doc = self.kb.index_document(
            "proxmox.md", "# Proxmox\n## Backups\nUse vzdump nightly to the NAS with zstd compression.\n"
            "## Networking\nBridge vmbr0 carries the VLAN trunk.\n",
        )

    def test_hybrid_finds_memories_and_documents(self) -> None:
        r = self.kb.search("when does Paul like meetings", limit=3)["results"]
        self.assertEqual(r[0]["id"], self.m1)
        self.assertEqual(r[0]["kind"], "memory")
        self.assertIsNotNone(r[0]["similarity"])
        self.assertGreater(r[0]["score"], 0)
        d = self.kb.search("vzdump backups nightly", limit=3)["results"][0]
        self.assertEqual((d["kind"], d["source"]), ("document", "proxmox.md"))
        self.assertTrue(d["content"].startswith("proxmox.md › Proxmox"))

    def test_filters(self) -> None:
        only_docs = self.kb.search("Paul meetings Proxmox", kinds=["document"])["results"]
        self.assertTrue(all(h["kind"] == "document" for h in only_docs))
        tagged = self.kb.search("cluster meetings", tags=["work"])["results"]
        self.assertEqual([h["id"] for h in tagged], [self.m2])
        self.assertEqual(self.kb.search("meetings", category="work")["results"], [])
        with self.assertRaises(BadRequest):
            self.kb.search("x", kinds=["secrets"])
        with self.assertRaises(BadRequest):
            self.kb.search("   ")

    def test_min_score_filters_irrelevant(self) -> None:
        self.assertEqual(self.kb.search("zebra giraffe safari", min_score=0.3)["results"], [])
        self.assertTrue(self.kb.search("morning meetings", min_score=0.3)["results"])

    def test_keyword_mode_and_semantic_mode(self) -> None:
        self.assertEqual(self.kb.search("Kubernetes", mode="keyword")["results"][0]["id"], self.m2)
        self.assertEqual(self.kb.search("Kubernetes staging", mode="semantic")["results"][0]["id"], self.m2)

    def test_recall_tracks_access_unless_disabled(self) -> None:
        self.kb.search("morning meetings", track=False)
        self.assertEqual(self.kb.get_memory(self.m1)["access_count"], 0)
        self.kb.search("morning meetings")
        self.assertEqual(self.kb.get_memory(self.m1)["access_count"], 1)

    def test_per_document_cap(self) -> None:
        body = "\n".join(f"## Section {i}\n" + f"alpha beta gamma delta {i} " * 30 for i in range(8))
        self.kb.index_document("big.md", "# Big\n" + body)
        r = self.kb.search("alpha beta gamma", limit=8, kinds=["document"])["results"]
        self.assertLessEqual(sum(h["source"] == "big.md" for h in r), 3)


class DocumentTests(Base):
    def test_reindex_is_incremental(self) -> None:
        doc = "# A\n" + "\n".join(f"## S{i}\n" + f"topic{i} " * 80 for i in range(6))
        first = self.kb.index_document("n.md", doc)
        self.assertEqual(first["status"], "created")
        self.assertEqual(self.kb.index_document("n.md", doc)["status"], "unchanged")
        before = self.emb.texts
        second = self.kb.index_document("n.md", doc.replace("topic5 ", "changed5 ", 1))
        self.assertEqual(second["id"], first["id"])
        self.assertEqual(second["status"], "updated")
        self.assertEqual(self.emb.texts - before, 1, "only the edited chunk is re-embedded")
        self.assertEqual(second["reused"], first["chunks"] - 1)
        self.assertEqual(len(self.kb.list_documents()), 1)
        got = self.kb.get_document(first["id"])
        self.assertEqual(len(got["chunks"]), second["chunks"])
        self.kb.delete_document(first["id"])
        self.assertEqual(self.kb.search("topic1")["results"], [])
        self.assertEqual(len(self.kb.index), 0)

    def test_limits(self) -> None:
        with self.assertRaises(BadRequest):
            self.kb.index_document("", "x")
        with self.assertRaises(BadRequest):
            self.kb.index_document("big", "x" * (5 * 1024 * 1024 + 1))


class LifecycleTests(Base):
    def test_export_import_roundtrip(self) -> None:
        self.kb.save_memory("Paul drives a 2019 Tacoma", ["car"], 6, "personal")
        pinned = self.kb.save_memory("Paul's wife is named Sarah", ["family"], 9, "personal")
        self.kb.update_memory(pinned["id"], {"pinned": True})
        self.kb.index_document("d.md", "# D\nsome content")
        recs = list(self.kb.export())
        self.assertEqual(recs[0]["type"], "kb-core-export")
        other = KnowledgeBase(Config(db_path=Path(self.tmp.name) / "b.sqlite3"), FakeEmbedder())
        self.addCleanup(other.close)
        counts = other.import_records(json.loads(json.dumps(recs)))
        self.assertEqual((counts["memories_created"], counts["documents"]), (2, 1))
        self.assertTrue(any(m["pinned"] for m in other.list_memories()))
        again = other.import_records(recs)
        self.assertEqual(again["memories_merged"], 2, "re-import merges instead of duplicating")

    def test_maintenance_merges_duplicates(self) -> None:
        a = self.kb.save_memory("Paul uses Neovim daily", ["a"], consolidate=False)
        self.kb.save_memory("paul uses neovim daily", ["b"], 9, consolidate=False)
        self.assertEqual(len(self.kb.list_memories()), 2)
        rep = self.kb.maintenance()
        self.assertEqual(rep["merged_duplicates"], 1)
        [m] = self.kb.list_memories()
        self.assertEqual((m["id"], m["tags"], m["importance"]), (a["id"], ["a", "b"], 9))

    def test_stats_and_activation(self) -> None:
        self.kb.save_memory("x fact one", importance=10)
        self.kb.search("fact")
        s = self.kb.stats()
        self.assertEqual(s["memories"], 1)
        self.assertEqual(s["searches_24h"], 1)
        self.assertEqual(s["vector_dimensions"], DIM)
        self.assertGreater(s["storage_bytes"], 0)
        self.assertEqual(s["recent_activity"][0]["kind"], "memory_saved")
        self.assertNotIn("x fact one", json.dumps(s), "analytics never contain memory text")
        self.assertGreater(self.kb.list_memories()[0]["activation"], 0.7)

    def test_db_file_is_private(self) -> None:
        self.assertEqual(os.stat(self.cfg.db_path).st_mode & 0o777, 0o600)

    def test_folder_sync(self) -> None:
        root = Path(self.tmp.name) / "notes"
        (root / "sub").mkdir(parents=True)
        (root / ".git").mkdir()
        (root / "a.md").write_text("# A\nalpha facts")
        (root / "sub" / "b.txt").write_text("bravo facts")
        (root / ".git" / "c.md").write_text("hidden")
        (root / "LICENSE").write_text("mit")
        r = sync_folder(self.kb, root)
        self.assertEqual((r["files"], r["indexed"]), (2, 2))
        self.assertEqual(sorted(d["name"] for d in self.kb.list_documents()), ["notes/a.md", "notes/sub/b.txt"])
        self.assertEqual(sync_folder(self.kb, root)["unchanged"], 2)
        (root / "a.md").write_text("# A\nalpha facts changed")
        os.utime(root / "a.md", (time.time() + 5, time.time() + 5))
        (root / "sub" / "b.txt").unlink()
        r = sync_folder(self.kb, root)
        self.assertEqual((r["indexed"], r["removed"]), (1, 1))
        self.assertIn("changed", self.kb.search("alpha")["results"][0]["content"])
        fs = FolderSync(self.kb, (root, Path(self.tmp.name) / "missing"), 60)
        fs.run_once()
        self.assertIn(str(Path(self.tmp.name) / "missing"), self.kb.stats()["watch"]["errors"])


class ApiTests(Base):
    def setUp(self) -> None:
        super().setUp()
        self.api = Api(self.kb)

    def test_contract_shapes(self) -> None:
        st, body = self.api.dispatch("POST", "/memories", {}, {"content": "c1", "tags": ["t"], "importance": 5,
                                                                "category": "general"})
        self.assertEqual(st, 200)
        mid = body["id"]
        st, body = self.api.dispatch("GET", "/memories", {"limit": ["10"]}, None)
        self.assertEqual(body["memories"][0]["id"], mid)
        for key in ("id", "content", "tags", "importance", "category", "created_at"):
            self.assertIn(key, body["memories"][0])
        st, body = self.api.dispatch("POST", "/search", {}, {"query": "c1", "limit": 5})
        self.assertEqual((st, body["results"][0]["id"]), (200, mid))
        for key in ("id", "content", "score", "tags", "created_at"):
            self.assertIn(key, body["results"][0])
        st, body = self.api.dispatch("POST", "/documents", {}, {"name": "n.md", "content": "hello", "mime_type": "text/plain"})
        self.assertEqual(st, 200)
        self.assertTrue(body["id"].startswith("d_"))
        self.assertEqual(body["chunks"], 1)
        self.assertEqual(self.api.dispatch("DELETE", f"/memories/{mid}", {}, None)[0], 200)
        self.assertEqual(self.api.dispatch("DELETE", f"/memories/{mid}", {}, None)[0], 404)
        self.assertEqual(self.api.dispatch("GET", "/health", {}, None)[0], 200)

    def test_errors(self) -> None:
        self.assertEqual(self.api.dispatch("GET", "/nope", {}, None)[0], 404)
        self.assertEqual(self.api.dispatch("PUT", "/memories", {}, None)[0], 405)
        self.assertEqual(self.api.dispatch("POST", "/memories", {}, ["x"])[0], 400)
        self.assertEqual(self.api.dispatch("POST", "/extract", {}, {"text": "x"})[0], 501)
        self.assertEqual(self.api.dispatch("POST", "/sync", {}, None)[0], 404)
        self.assertEqual(self.api.dispatch("GET", "/memories", {"limit": ["x"]}, None)[0], 400)


class HttpTests(Base):
    """Real sockets: the browser/CSRF guards and auth."""

    def _serve(self, cfg: Config) -> int:
        httpd = ThreadingHTTPServer(("127.0.0.1", 0), make_handler(cfg, Api(self.kb)))
        threading.Thread(target=httpd.serve_forever, daemon=True).start()
        self.addCleanup(httpd.server_close)
        self.addCleanup(httpd.shutdown)
        return httpd.server_address[1]

    def _req(self, port: int, method: str, path: str, body: Any = None, headers: dict[str, str] | None = None,
             host: str | None = None) -> tuple[int, Any]:
        c = http.client.HTTPConnection("127.0.0.1", port, timeout=10)
        h = {"Content-Type": "application/json", **(headers or {})}
        data = json.dumps(body).encode() if body is not None else None
        c.putrequest(method, path, skip_host=True)
        c.putheader("Host", host or f"127.0.0.1:{port}")
        for k, v in h.items():
            c.putheader(k, v)
        if data is not None:
            c.putheader("Content-Length", str(len(data)))
        c.endheaders(data)
        r = c.getresponse()
        raw = r.read()
        c.close()
        try:
            return r.status, json.loads(raw)
        except ValueError:
            return r.status, raw

    def test_guards(self) -> None:
        port = self._serve(self.cfg)
        self.assertEqual(self._req(port, "GET", "/health")[0], 200)
        self.assertEqual(self._req(port, "GET", "/health", host=f"localhost:{port}")[0], 200)
        self.assertEqual(self._req(port, "GET", "/health", host="evil.example:80")[0], 421)
        self.assertEqual(self._req(port, "POST", "/memories", {"content": "x"},
                                   headers={"Origin": "https://evil.example"})[0], 403)
        self.assertEqual(self._req(port, "POST", "/memories", {"content": "x"},
                                   headers={"Content-Type": "text/plain"})[0], 415)
        self.assertEqual(self._req(port, "OPTIONS", "/memories")[0], 405)
        self.assertEqual(self._req(port, "DELETE", "/memories/..%2Fx")[0], 400)
        st, body = self._req(port, "POST", "/memories", {"content": "real memory"})
        self.assertEqual(st, 200)
        st, raw = self._req(port, "GET", "/export")
        self.assertEqual(st, 200)
        lines = raw.decode().splitlines() if isinstance(raw, bytes) else [json.dumps(raw)]
        self.assertEqual(json.loads(lines[-1])["content"], "real memory")

    def test_cli_export_writes_records(self) -> None:
        from kb_core.cli import main
        port = self._serve(self.cfg)
        self.kb.save_memory("exported fact")
        out = Path(self.tmp.name) / "export.jsonl"
        self.assertEqual(main(["--url", f"http://127.0.0.1:{port}", "export", str(out)]), 0)
        lines = out.read_text().splitlines()
        self.assertEqual(len(lines), 2, "header + one memory")
        self.assertEqual(json.loads(lines[1])["content"], "exported fact")
        self.assertEqual(os.stat(out).st_mode & 0o777, 0o600)

    def test_token(self) -> None:
        cfg = Config(db_path=self.cfg.db_path, token="s3cret-token")
        port = self._serve(cfg)
        self.assertEqual(self._req(port, "GET", "/health")[0], 401)
        self.assertEqual(self._req(port, "GET", "/health", headers={"Authorization": "Bearer nope"})[0], 401)
        self.assertEqual(self._req(port, "GET", "/health", headers={"Authorization": "Bearer s3cret-token"},
                                   host="192.168.1.5:8100")[0], 200)

    def test_off_loopback_bind_requires_token(self) -> None:
        with self.assertRaises(SystemExit):
            Config(host="0.0.0.0").validate()
        Config(host="0.0.0.0", token="t").validate()
        Config(host="127.0.0.1").validate()


if __name__ == "__main__":
    unittest.main()


class CollectionTests(Base):
    def test_collections_scope_everything(self) -> None:
        self.kb.create_collection("work", "Job stuff")
        w = self.kb.save_memory("The staging cluster runs Kubernetes 1.31", collection="work")
        p = self.kb.save_memory("Paul's daughter plays the violin", collection="personal")
        self.kb.index_document("runbook.md", "# Runbook\nrestart the staging cluster", collection="work")
        names = {c["name"]: c for c in self.kb.list_collections()}
        self.assertEqual(set(names), {"default", "work", "personal"})
        self.assertEqual((names["work"]["memories"], names["work"]["documents"]), (1, 1))
        self.assertEqual(names["work"]["description"], "Job stuff")
        only_work = self.kb.search("staging cluster violin", collections=["work"])["results"]
        self.assertTrue(only_work and all(h["collection"] == "work" for h in only_work))
        no_work = self.kb.search("staging cluster violin", exclude_collections=["work"])["results"]
        self.assertTrue(all(h["collection"] != "work" for h in no_work))
        self.assertEqual([m["id"] for m in self.kb.list_memories(collection="personal")], [p["id"]])
        # The same fact in another collection is not merged across collections.
        again = self.kb.save_memory("The staging cluster runs Kubernetes 1.31", collection="personal")
        self.assertEqual(again["status"], "created")
        self.assertEqual(self.kb.save_memory("The staging cluster runs Kubernetes 1.31", collection="work")["id"], w["id"])
        with self.assertRaises(BadRequest):
            self.kb.save_memory("x", collection="Bad Name!")
        with self.assertRaises(BadRequest):
            self.kb.delete_collection("default")
        gone = self.kb.delete_collection("work")
        # 1 memory (the later save to "work" reinforced it) + 1 runbook chunk.
        self.assertEqual((gone["documents"], gone["items"]), (1, 2))
        self.assertEqual(self.kb.search("staging cluster", collections=["work"])["results"], [])
        self.assertNotIn(w["id"], self.kb.index)

    def test_export_keeps_collections(self) -> None:
        self.kb.save_memory("fact in a kb", collection="kb1")
        other = KnowledgeBase(Config(db_path=Path(self.tmp.name) / "c.sqlite3"), FakeEmbedder())
        self.addCleanup(other.close)
        other.import_records(list(self.kb.export()))
        self.assertEqual(other.list_memories()[0]["collection"], "kb1")

    def test_migrates_v1_database(self) -> None:
        import sqlite3
        path = Path(self.tmp.name) / "v1.sqlite3"
        db = sqlite3.connect(path)
        db.executescript("""
            CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE documents (id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, mime_type TEXT NOT NULL DEFAULT 'text/plain',
                size INTEGER NOT NULL, content_hash TEXT NOT NULL, content TEXT NOT NULL, source_path TEXT, source_mtime REAL,
                chunks INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
            CREATE TABLE items (pk INTEGER PRIMARY KEY, id TEXT NOT NULL UNIQUE, kind TEXT NOT NULL, doc_id TEXT, ord INTEGER NOT NULL DEFAULT 0,
                context TEXT NOT NULL DEFAULT '', content TEXT NOT NULL, tags TEXT NOT NULL DEFAULT '[]', category TEXT NOT NULL DEFAULT '',
                importance INTEGER NOT NULL DEFAULT 5, source TEXT NOT NULL DEFAULT 'user', pinned INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL, last_accessed TEXT, access_count INTEGER NOT NULL DEFAULT 0,
                reinforced INTEGER NOT NULL DEFAULT 0, superseded_by TEXT, content_hash TEXT NOT NULL, vec BLOB);
            INSERT INTO items(id, kind, content, created_at, updated_at, content_hash) VALUES ('m_old', 'memory', 'old fact', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'h');
            PRAGMA user_version = 1;
        """)
        db.close()
        kb = KnowledgeBase(Config(db_path=path), FakeEmbedder())
        self.addCleanup(kb.close)
        self.assertEqual(kb.list_memories()[0]["collection"], "default")
        self.assertFalse(kb.list_memories()[0]["reviewed"], "v1 → v2 → v3 in one open")
        self.assertEqual(kb.db.execute("PRAGMA user_version").fetchone()[0], 3)
        kb.backfill()
        self.assertEqual(kb.search("old fact")["results"][0]["id"], "m_old", "FTS still works after migration")


class ScaleTests(unittest.TestCase):
    def test_index_grows_and_swap_removes(self) -> None:
        idx = VectorIndex()
        for i in range(3000):
            idx.add(f"v{i}", normalise([float(i % 7), 1.0, float(i % 3)]))
        self.assertEqual(len(idx), 3000)
        idx.remove([f"v{i}" for i in range(0, 3000, 2)])
        self.assertEqual(len(idx), 1500)
        self.assertIsNone(idx.get("v0"))
        self.assertIsNotNone(idx.get("v2999"))
        top = idx.search(normalise([6.0, 1.0, 2.0]), 1)[0][0]
        self.assertTrue(top.startswith("v"))
        # Every remaining id still resolves to its own vector.
        for i in range(1, 3000, 250):
            v = idx.get(f"v{i}")
            self.assertAlmostEqual(dot(v, normalise([float(i % 7), 1.0, float(i % 3)])), 1.0, places=5)

    def test_float16_index(self) -> None:
        idx = VectorIndex("float16")
        idx.add("a", normalise([1, 0, 0]))
        idx.add("b", normalise([0, 1, 0]))
        self.assertEqual(idx.search(normalise([1, 0.1, 0]), 1)[0][0], "a")

    def test_watch_spec(self) -> None:
        from kb_core.config import parse_watch
        w = parse_watch("/tmp/a:notes=/tmp/b")
        self.assertEqual(w[0], (None, Path("/tmp/a").resolve()))
        self.assertEqual(w[1], ("notes", Path("/tmp/b").resolve()))


class CodeTests(Base):
    def test_code_chunks_by_definition(self) -> None:
        src = "import os\n\n" + "".join(
            f"def func_{i}(x):\n" + "".join(f"    y{j} = x * {j}  # some work\n" for j in range(25)) + "    return y1\n\n"
            for i in range(4)
        )
        chunks = chunk_code("tools.py", src)
        self.assertTrue(any(c.context == "tools.py › func_2" for c in chunks), [c.context for c in chunks])
        conf = "[server]\nport = 80\n\n[database]\nurl = 'x'\n" + "\n".join(f"k{i} = {i}" for i in range(200))
        self.assertTrue(any("database" in c.context for c in chunk_code("app.toml", conf)))
        res = self.kb.index_document("tools.py", src, "text/x-py")
        self.assertGreater(res["chunks"], 1)
        hit = self.kb.search("func_3")["results"][0]
        self.assertIn("func_3", hit["content"])

    def test_sync_indexes_code_into_collection(self) -> None:
        from kb_core.sync import mime_for
        root = Path(self.tmp.name) / "proj"
        root.mkdir()
        (root / "app.py").write_text("def main():\n    print('hi')\n")
        (root / "notes.md").write_text("# Notes\nhello")
        (root / "image.png").write_bytes(b"\x89PNG")
        r = sync_folder(self.kb, root, "proj")
        self.assertEqual(r["indexed"], 2)
        docs = {d["name"]: d for d in self.kb.list_documents()}
        self.assertEqual(docs["proj/app.py"]["mime_type"], "text/x-py")
        self.assertEqual(docs["proj/app.py"]["collection"], "proj")
        self.assertEqual(mime_for(Path("x.pdf")), "application/pdf")


class ApiExtensionTests(Base):
    def test_collections_batch_and_metrics(self) -> None:
        from kb_core.server import prometheus
        api = Api(self.kb)
        self.assertEqual(api.dispatch("POST", "/collections", {}, {"name": "ops", "description": "d"})[0], 200)
        st, body = api.dispatch("GET", "/collections", {}, None)
        self.assertIn("ops", [c["name"] for c in body["collections"]])
        st, body = api.dispatch("POST", "/documents/batch", {}, {"documents": [
            {"name": "a.md", "content": "alpha", "collection": "ops"},
            {"name": "", "content": "bad"},
        ]})
        self.assertEqual(st, 200)
        self.assertEqual(body["results"][0]["collection"], "ops")
        self.assertIn("error", body["results"][1])
        st, body = api.dispatch("POST", "/search", {}, {"query": "alpha", "collections": ["ops"]})
        self.assertEqual(body["results"][0]["source"], "a.md")
        self.assertEqual(api.dispatch("DELETE", "/collections/ops", {}, None)[0], 200)
        self.assertEqual(api.dispatch("DELETE", "/collections/default", {}, None)[0], 400)
        text = prometheus(self.kb)
        self.assertIn("kb_core_searches_total 1", text)
        self.assertIn('kb_core_items{kind="memory"} 0', text)


class ReviewHardeningTests(Base):
    """Merges are reversible, decay never filters, hits carry provenance."""

    def test_llm_merge_is_soft_and_restorable(self) -> None:
        old = self.kb.save_memory("Paul likes meetings in the morning", ["cal"], 8)
        kb = self.reopen(chat=FakeChat({"duplicate": [1], "obsolete": []}))
        new = kb.save_memory("Paul prefers morning meetings, before 10am", ["prefs"], 5)
        kb._judge_related(*kb._jobs.get_nowait()[1])
        # Hidden from the live list and from search, but not deleted.
        self.assertEqual([m["id"] for m in kb.list_memories()], [new["id"]])
        self.assertNotIn(old["id"], [h["id"] for h in kb.search("morning meetings")["results"]])
        hidden = kb.get_memory(old["id"])
        self.assertEqual((hidden["superseded_by"], hidden["hidden_reason"]), (new["id"], "merged"))
        self.assertEqual(hidden["content"], "Paul likes meetings in the morning")
        [h] = kb.list_memories(hidden_only=True)
        self.assertEqual((h["id"], h["hidden_reason"]), (old["id"], "merged"))
        # Undo the ruling: the original comes back unchanged.
        kb.update_memory(old["id"], {"superseded_by": None})
        restored = kb.get_memory(old["id"])
        self.assertIsNone(restored["superseded_by"])
        self.assertIsNone(restored["hidden_reason"])
        self.assertEqual(restored["tags"], ["cal"])
        self.assertFalse([l for l in restored["links"] if l["kind"] == "merged"])
        self.assertEqual(kb.list_memories(hidden_only=True), [])

    def test_deleting_the_survivor_restores_merged_memory(self) -> None:
        old = self.kb.save_memory("Paul likes meetings in the morning")
        kb = self.reopen(chat=FakeChat({"duplicate": [1], "obsolete": []}))
        new = kb.save_memory("Paul prefers morning meetings, before 10am")
        kb._judge_related(*kb._jobs.get_nowait()[1])
        kb.delete_memory(new["id"])
        self.assertEqual([m["id"] for m in kb.list_memories()], [old["id"]])

    def test_maintenance_merge_is_soft(self) -> None:
        a = self.kb.save_memory("Paul uses Neovim daily", ["a"], consolidate=False)
        b = self.kb.save_memory("paul uses neovim daily", ["b"], 9, consolidate=False)
        self.assertEqual(self.kb.maintenance()["merged_duplicates"], 1)
        self.assertEqual(self.kb.get_memory(b["id"])["superseded_by"], a["id"])
        self.assertEqual(self.kb.stats()["memories"], 1)
        # A second pass doesn't re-merge what is already hidden.
        self.assertEqual(self.kb.maintenance()["merged_duplicates"], 0)

    def test_hidden_reason_distinguishes_superseded(self) -> None:
        old = self.kb.save_memory("Paul's favourite editor is Vim")
        kb = self.reopen(chat=FakeChat({"duplicate": [], "obsolete": [1]}))
        kb.save_memory("Paul's favourite editor is now VS Code, not Vim")
        kb._judge_related(*kb._jobs.get_nowait()[1])
        self.assertEqual(kb.get_memory(old["id"])["hidden_reason"], "superseded")

    def test_import_skips_hidden_history(self) -> None:
        old = self.kb.save_memory("Paul likes meetings in the morning")
        kb = self.reopen(chat=FakeChat({"duplicate": [1], "obsolete": []}))
        kb.save_memory("Paul prefers morning meetings, before 10am")
        kb._judge_related(*kb._jobs.get_nowait()[1])
        recs = list(kb.export())
        self.assertTrue(any(r.get("id") == old["id"] for r in recs), "history is still exported")
        other = KnowledgeBase(Config(db_path=Path(self.tmp.name) / "b.sqlite3"), FakeEmbedder())
        self.addCleanup(other.close)
        counts = other.import_records(recs)
        self.assertEqual((counts["memories_created"], counts["history_skipped"]), (1, 1))

    def test_faded_memory_is_not_filtered_by_decay(self) -> None:
        m = self.kb.save_memory("Paul's anniversary is October 12", importance=2)["id"]
        self.kb.save_memory("The staging cluster runs Kubernetes")
        # Three years untouched: activation is near its floor.
        self.kb.db.execute("UPDATE items SET created_at = '2023-01-01T00:00:00Z', "
                           "updated_at = '2023-01-01T00:00:00Z', last_accessed = NULL WHERE id = ?", (m,))
        [hit] = [h for h in self.kb.search("anniversary October", track=False)["results"] if h["id"] == m]
        self.assertLess(hit["explain"]["rank_score"], hit["score"], "activation still lowers the ordering key")
        cut = (hit["explain"]["rank_score"] + hit["score"]) / 2  # above the decayed rank, below relevance
        again = self.kb.search("anniversary October", min_score=cut, track=False)["results"]
        self.assertIn(m, [h["id"] for h in again], "the cut-off uses relevance, not decayed rank")

    def test_vivid_memory_ranks_first_among_equals(self) -> None:
        faded = self.kb.save_memory("Backups go to the NAS nightly", importance=2)["id"]
        vivid = self.kb.save_memory("Backups go to the NAS nightly with zstd", importance=10)["id"]
        self.kb.db.execute("UPDATE items SET created_at = '2023-01-01T00:00:00Z', "
                           "updated_at = '2023-01-01T00:00:00Z' WHERE id = ?", (faded,))
        ids = [h["id"] for h in self.kb.search("backups NAS nightly", track=False)["results"]]
        self.assertEqual(ids[0], vivid)

    def test_hits_carry_origin(self) -> None:
        self.kb.save_memory("Paul's NAS is at 10.0.0.5", source="user")
        self.kb.save_memory("The NAS runs TrueNAS SCALE", source="assistant")
        self.kb.index_document("nas.md", "# NAS\nThe NAS exports an NFS share for backups.")
        got = {h["content"].split("\n")[0]: h["origin"] for h in self.kb.search("NAS", limit=10)["results"]}
        self.assertEqual(got["Paul's NAS is at 10.0.0.5"], "user")
        self.assertEqual(got["The NAS runs TrueNAS SCALE"], "assistant")
        self.assertEqual(got["nas.md › NAS"], "document")

    def test_http_hidden_only(self) -> None:
        old = self.kb.save_memory("Paul likes meetings in the morning")
        kb = self.reopen(chat=FakeChat({"duplicate": [1], "obsolete": []}))
        kb.save_memory("Paul prefers morning meetings, before 10am")
        kb._judge_related(*kb._jobs.get_nowait()[1])
        status, body = Api(kb).dispatch("GET", "/memories", {"hidden_only": ["true"]}, None)
        self.assertEqual(status, 200)
        self.assertEqual([m["id"] for m in body["memories"]], [old["id"]])


class ReviewQueueTests(Base):
    """Model rulings stay soft and count as "needs review" until the user
    keeps or restores them."""

    def _merged_pair(self) -> tuple[KnowledgeBase, str, str]:
        old = self.kb.save_memory("Paul likes meetings in the morning")["id"]
        kb = self.reopen(chat=FakeChat({"duplicate": [1], "obsolete": []}))
        new = kb.save_memory("Paul prefers morning meetings, before 10am")["id"]
        kb._judge_related(*kb._jobs.get_nowait()[1])
        return kb, old, new

    def test_new_rulings_need_review_and_keep_clears_it(self) -> None:
        kb, old, _ = self._merged_pair()
        self.assertEqual(kb.stats()["needs_review"], 1)
        self.assertFalse(kb.get_memory(old)["reviewed"])
        kept = kb.update_memory(old, {"reviewed": True})
        self.assertTrue(kept["reviewed"])
        self.assertIsNotNone(kept["superseded_by"], "keeping the ruling leaves the memory hidden")
        self.assertEqual(kb.stats()["needs_review"], 0)
        self.assertEqual(kb.list_memories(hidden_only=True)[0]["reviewed"], True)

    def test_restore_resets_review_and_a_new_ruling_needs_review_again(self) -> None:
        kb, old, new = self._merged_pair()
        kb.update_memory(old, {"reviewed": True})
        kb.update_memory(old, {"superseded_by": None})
        self.assertFalse(kb.get_memory(old)["reviewed"])
        kb._judge_related(new, [old])  # the model merges it again
        self.assertEqual(kb.stats()["needs_review"], 1)

    def test_superseded_rulings_need_review_too(self) -> None:
        old = self.kb.save_memory("Paul's favourite editor is Vim")["id"]
        kb = self.reopen(chat=FakeChat({"duplicate": [], "obsolete": [1]}))
        kb.save_memory("Paul's favourite editor is now VS Code, not Vim")
        kb._judge_related(*kb._jobs.get_nowait()[1])
        self.assertEqual(kb.stats()["needs_review"], 1)
        self.assertFalse(kb.get_memory(old)["reviewed"])

    def test_reviewed_is_validated(self) -> None:
        live = self.kb.save_memory("Paul drives a Tacoma")["id"]
        with self.assertRaises(BadRequest):
            self.kb.update_memory(live, {"reviewed": True})  # nothing to review
        kb, old, _ = self._merged_pair()
        with self.assertRaises(BadRequest):
            kb.update_memory(old, {"reviewed": "yes"})
        with self.assertRaises(BadRequest):
            kb.update_memory(old, {"reviewed": True, "superseded_by": None})

    def test_http_patch_reviewed(self) -> None:
        kb, old, _ = self._merged_pair()
        status, body = Api(kb).dispatch("PATCH", f"/memories/{old}", {}, {"reviewed": True})
        self.assertEqual((status, body["reviewed"]), (200, True))

    def test_v2_database_migrates(self) -> None:
        m = self.kb.save_memory("Paul drives a Tacoma")["id"]
        self.kb.close()
        db = sqlite3.connect(self.cfg.db_path)
        db.execute("ALTER TABLE items DROP COLUMN reviewed")
        db.execute("PRAGMA user_version = 2")
        db.commit()
        db.close()
        kb = self.reopen()
        self.assertFalse(kb.get_memory(m)["reviewed"])
        self.assertEqual(kb.db.execute("PRAGMA user_version").fetchone()[0], 3)
