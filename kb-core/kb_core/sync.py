"""Live folder sync: keep watched note folders indexed.

Every ``watch_interval`` seconds each watched folder is walked; files whose
mtime changed are re-indexed (content hashing makes an unchanged file a
no-op, and unchanged chunks keep their vectors), new files are added, and
documents whose file disappeared are removed. Polling needs no inotify
limits, works on network mounts, and costs a few stat() calls per file.

Documents are named ``<folder name>/<relative path>`` so the same folder
indexed with ``kb-core ingest`` and via watching produces the same names.

File types: notes (Markdown, text, reStructuredText, Org, AsciiDoc), PDFs
(text extracted with ``pdftotext`` when installed; fixed argv, no shell,
60 s timeout) and, unless ``KB_CORE_INDEX_CODE=0``, source code and config
files (chunked at definitions/sections). A folder can be filed into a
collection with ``KB_CORE_WATCH=name=/path``.
"""

from __future__ import annotations

import logging
import os
import shutil
import subprocess
import threading
import time
from pathlib import Path
from typing import Iterator

from .store import MAX_DOCUMENT_BYTES, KbError, KnowledgeBase, now_iso

log = logging.getLogger("kb-core.sync")

NOTE_EXT = {".md", ".markdown", ".mdx", ".txt", ".rst", ".org", ".adoc"}
CODE_EXT = {
    ".py", ".rs", ".go", ".js", ".mjs", ".cjs", ".ts", ".tsx", ".jsx", ".svelte", ".vue", ".java", ".kt",
    ".c", ".h", ".cc", ".cpp", ".hpp", ".cs", ".rb", ".php", ".swift", ".scala", ".lua", ".sql",
    ".sh", ".bash", ".zsh", ".ps1", ".tf", ".hcl", ".nix",
    ".toml", ".ini", ".cfg", ".conf", ".yaml", ".yml", ".json", ".properties",
}
PDF_EXT = {".pdf"}
EXTENSIONS = NOTE_EXT | CODE_EXT | PDF_EXT
MAX_PDF_BYTES = 50 * 1024 * 1024
SKIP_DIRS = {"node_modules", "__pycache__", "venv", ".venv", "target", "dist", "build"}
SKIP_NAMES = {
    "license", "license.md", "license.txt", "changelog.md", "code_of_conduct.md", "contributors.md",
    "pull_request_template.md",
}


def mime_for(path: Path) -> str:
    ext = path.suffix.lower()
    if ext in {".md", ".markdown", ".mdx"}:
        return "text/markdown"
    if ext in PDF_EXT:
        return "application/pdf"
    if ext in CODE_EXT:
        return f"text/x-{ext.lstrip('.')}"
    return "text/plain"


def read_document(path: Path) -> str | None:
    """Text of a supported file, or None (unreadable / not text / tool missing)."""
    ext = path.suffix.lower()
    if ext in PDF_EXT:
        tool = shutil.which("pdftotext")
        if not tool:
            return None
        try:
            # Fixed argv, no shell; the path is passed as one argument.
            r = subprocess.run([tool, "-layout", "-q", "-enc", "UTF-8", str(path), "-"],
                               capture_output=True, timeout=60, check=False)
        except (OSError, subprocess.TimeoutExpired):
            return None
        text = r.stdout.decode("utf-8", "replace").replace("\f", "\n\n")
        return text if r.returncode == 0 and text.strip() else None
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return None


def iter_files(root: Path, include_code: bool = True) -> Iterator[Path]:
    """Indexable files under ``root``, skipping hidden and build directories."""
    for dirpath, dirnames, filenames in os.walk(root, followlinks=False):
        dirnames[:] = sorted(d for d in dirnames if not d.startswith(".") and d not in SKIP_DIRS)
        for f in sorted(filenames):
            p = Path(dirpath) / f
            if (
                not f.startswith(".")
                and p.suffix.lower() in (EXTENSIONS if include_code else NOTE_EXT | PDF_EXT)
                and f.lower() not in SKIP_NAMES
                and not p.is_symlink()
            ):
                yield p


def doc_name(root: Path, path: Path) -> str:
    return f"{root.name}/{path.relative_to(root).as_posix()}"


def sync_folder(kb: KnowledgeBase, root: Path, collection: str | None = None) -> dict[str, int]:
    """One pass over ``root``. Returns counts of what changed."""
    counts = {"files": 0, "indexed": 0, "unchanged": 0, "removed": 0, "skipped": 0}
    if not root.is_dir():
        raise FileNotFoundError(f"{root} is not a directory")
    known = kb.documents_by_source(root)
    seen: set[str] = set()
    for path in iter_files(root, kb.cfg.index_code):
        counts["files"] += 1
        key = str(path)
        seen.add(key)
        try:
            st = path.stat()
        except OSError:
            continue
        prev = known.get(key)
        if prev and prev[1] is not None and abs(prev[1] - st.st_mtime) < 1e-6:
            counts["unchanged"] += 1
            continue
        is_pdf = path.suffix.lower() in PDF_EXT
        if st.st_size > (MAX_PDF_BYTES if is_pdf else MAX_DOCUMENT_BYTES):
            counts["skipped"] += 1
            continue
        text = read_document(path)
        if text is None or len(text.encode("utf-8")) > MAX_DOCUMENT_BYTES:
            counts["skipped"] += 1
            continue
        try:
            res = kb.index_document(doc_name(root, path), text, mime_for(path), key, st.st_mtime,
                                    collection=collection)
        except KbError as e:
            log.warning("could not index %s: %s", path, e)
            counts["skipped"] += 1
            continue
        counts["unchanged" if res["status"] == "unchanged" else "indexed"] += 1
    for key, (doc_id, _) in known.items():
        if key not in seen:
            try:
                kb.delete_document(doc_id)
                counts["removed"] += 1
            except KbError:
                pass
    return counts


class FolderSync:
    """Background thread that syncs the configured folders periodically."""

    def __init__(self, kb: KnowledgeBase, roots: tuple, interval: float) -> None:
        self.kb = kb
        # Each root is a Path or (collection | None, Path).
        self.targets: list[tuple[str | None, Path]] = [r if isinstance(r, tuple) else (None, r) for r in roots]
        self.roots = tuple(p for _, p in self.targets)
        self.interval = interval
        self._now = threading.Event()
        self._stop = threading.Event()
        self._running = threading.Lock()  # the timer and POST /sync never overlap
        kb.sync_status = {"folders": [str(p) for p in self.roots], "interval_s": interval, "last_run": None,
                          "last_result": {}, "errors": {}}

    def start(self) -> None:
        if self.roots:
            threading.Thread(target=self._loop, name="kb-sync", daemon=True).start()

    def trigger(self) -> None:
        self._now.set()

    def stop(self) -> None:
        self._stop.set()
        self._now.set()

    def run_once(self) -> dict[str, dict[str, int]]:
        with self._running:
            return self._run()

    def _run(self) -> dict[str, dict[str, int]]:
        results: dict[str, dict[str, int]] = {}
        errors: dict[str, str] = {}
        for coll, root in self.targets:
            t0 = time.time()
            try:
                results[str(root)] = sync_folder(self.kb, root, coll)
                r = results[str(root)]
                if r["indexed"] or r["removed"]:
                    log.info("synced %s: %s (%.1fs)", root, r, time.time() - t0)
            except Exception as e:  # noqa: BLE001 - report, keep syncing other folders
                errors[str(root)] = str(e)
                log.warning("sync of %s failed: %s", root, e)
        self.kb.sync_status.update(last_run=now_iso(), last_result=results, errors=errors)
        return results

    def _loop(self) -> None:
        while not self._stop.is_set():
            self.run_once()
            self._now.wait(timeout=self.interval)
            self._now.clear()
