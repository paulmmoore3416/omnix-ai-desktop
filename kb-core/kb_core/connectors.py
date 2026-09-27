"""Knowledge connectors: how a watched folder becomes documents.

A connector decides three things for a source folder: which files belong to
it, what text of each file gets indexed, and what MIME type it has. Folder
sync (``sync.py``) and ``kb-core ingest`` never read files themselves; they
ask the connector for the folder. The contract is :class:`Connector`:

``name``
    Short id (``[a-z][a-z0-9_-]*``), shown in ``kb-core status`` / ``/stats``.
``detect(root) -> bool``
    True when this connector recognizes ``root`` (e.g. an Obsidian vault has
    a ``.obsidian/`` directory). Connectors are tried most specific first; the
    plain folder connector matches everything and comes last.
``iter_files(root, include_code) -> Iterator[Path]``
    The files to index, as real paths under ``root`` (no symlinks, no hidden
    or build directories).
``read(path) -> str | None``
    The text to index, or None to skip the file. This is where a connector
    normalizes its format (front matter, wiki links, markup).
``mime(path) -> str``
    The MIME type recorded with the document.

Document names are always ``<folder name>/<relative path>``, whatever the
connector, so re-indexing is stable and a folder can switch connectors.

Built-ins: ``obsidian`` and ``folder``. Additional connectors are ordinary
Python packages exposing a :class:`Connector` subclass (or instance) under the
``kb_core.connectors`` entry-point group. Plugins run inside kb-core with its
access to the memory database, so **none is loaded unless its entry-point name
is listed in** ``KB_CORE_CONNECTOR_PLUGINS`` (comma-separated). Installing a
package into the venv is not enough on its own.

Everything a connector returns is data: it is chunked and embedded, never
executed or rendered as HTML.
"""

from __future__ import annotations

import logging
import os
import re
from pathlib import Path
from typing import Iterable, Iterator

from .sync import CODE_EXT, EXTENSIONS, NOTE_EXT, PDF_EXT, SKIP_DIRS, SKIP_NAMES, mime_for, read_document

log = logging.getLogger("kb-core.connectors")

NAME_RE = re.compile(r"^[a-z][a-z0-9_-]{0,31}$")
ENTRY_POINT_GROUP = "kb_core.connectors"


class Connector:
    """Base class and the plain-folder behaviour (notes, PDFs, optionally code)."""

    name = "folder"
    description = "Any folder: notes, PDFs and (optionally) source code"

    def detect(self, root: Path) -> bool:
        return root.is_dir()

    def extensions(self, include_code: bool) -> set[str]:
        return set(EXTENSIONS) if include_code else NOTE_EXT | PDF_EXT

    def iter_files(self, root: Path, include_code: bool = True) -> Iterator[Path]:
        exts = self.extensions(include_code)
        for dirpath, dirnames, filenames in os.walk(root, followlinks=False):
            dirnames[:] = sorted(d for d in dirnames if not d.startswith(".") and d not in SKIP_DIRS)
            for f in sorted(filenames):
                p = Path(dirpath) / f
                if (
                    not f.startswith(".")
                    and p.suffix.lower() in exts
                    and f.lower() not in SKIP_NAMES
                    and not p.is_symlink()
                ):
                    yield p

    def read(self, path: Path) -> str | None:
        return read_document(path)

    def mime(self, path: Path) -> str:
        return mime_for(path)


FolderConnector = Connector


# --------------------------------------------------------------------- Obsidian

_FRONT_MATTER = re.compile(r"\A---[ \t]*\r?\n(.*?)\r?\n---[ \t]*(?:\r?\n|\Z)", re.S)
_EMBED = re.compile(r"!\[\[([^\]|#^]+)(?:[#^][^\]|]*)?(?:\|[^\]]*)?\]\]")
_WIKILINK = re.compile(r"\[\[([^\]|]+?)(?:\|([^\]]+))?\]\]")
_COMMENT = re.compile(r"%%.*?%%", re.S)
_FENCE = re.compile(r"^(```|~~~).*?^\1", re.S | re.M)
_INLINE_TAG = re.compile(r"(?<![\w&/#`])#([A-Za-z_][\w/-]{0,63})")


def _fm_list(value: str, following: list[str]) -> list[str]:
    """A front-matter value as a list: ``[a, b]``, ``a, b``, ``a`` or a
    following block of ``- a`` lines. (A tiny YAML subset: enough for the
    keys Obsidian itself writes, with no YAML dependency or code execution.)"""
    value = value.strip()
    if value.startswith("[") and value.endswith("]"):
        items = value[1:-1].split(",")
    elif value:
        items = value.split(",") if "," in value else value.split()
    else:
        items = [ln.strip()[1:] for ln in following if ln.strip().startswith("-")]
    return [i.strip().strip("\"'").lstrip("#") for i in items if i.strip().strip("\"'")]


def parse_front_matter(text: str) -> tuple[dict[str, list[str] | str], str]:
    """Split ``---`` front matter off ``text``. Returns (fields, body); only
    ``tags``, ``aliases`` and ``title`` are interpreted."""
    m = _FRONT_MATTER.match(text)
    if not m:
        return {}, text
    lines = m.group(1).splitlines()
    fields: dict[str, list[str] | str] = {}
    for i, line in enumerate(lines):
        if not line or line[0] in " \t-#":
            continue
        key, sep, value = line.partition(":")
        key = key.strip().lower()
        if not sep:
            continue
        if key in {"tags", "tag", "aliases", "alias"}:
            block = []
            for nxt in lines[i + 1:]:
                if nxt.strip().startswith("-"):
                    block.append(nxt)
                elif nxt.strip():
                    break
            fields["tags" if key.startswith("tag") else "aliases"] = _fm_list(value, block)
        elif key == "title":
            fields["title"] = value.strip().strip("\"'")
    return fields, text[m.end():]


class ObsidianConnector(Connector):
    """Obsidian vaults: Markdown notes (and PDFs), front matter and tags kept
    as searchable text, wiki links flattened to their visible text, ``%%
    comments %%`` dropped, ``.obsidian/`` and ``.trash/`` skipped (hidden)."""

    name = "obsidian"
    description = "Obsidian vault: front matter, #tags and [[wiki links]]"

    def detect(self, root: Path) -> bool:
        return (root / ".obsidian").is_dir()

    def extensions(self, include_code: bool) -> set[str]:
        # A vault is notes; code blocks inside notes are indexed as note text.
        return {".md", ".markdown"} | PDF_EXT

    def read(self, path: Path) -> str | None:
        text = read_document(path)
        if text is None or path.suffix.lower() in PDF_EXT:
            return text
        return self.normalize(path.stem, text)

    @staticmethod
    def normalize(stem: str, text: str) -> str:
        fields, body = parse_front_matter(text)
        body = _COMMENT.sub("", body)
        body = _EMBED.sub(lambda m: f"(embedded: {m.group(1).strip()})", body)
        body = _WIKILINK.sub(lambda m: (m.group(2) or m.group(1).split("#")[0]).strip(), body)
        # Inline #tags outside fenced code.
        prose = _FENCE.sub("", body)
        tags = list(dict.fromkeys([*fields.get("tags", []), *(_INLINE_TAG.findall(prose))]))  # type: ignore[misc]
        header = [f"Title: {fields.get('title') or stem}"]
        if fields.get("aliases"):
            header.append("Aliases: " + ", ".join(fields["aliases"]))  # type: ignore[arg-type]
        if tags:
            header.append("Tags: " + " ".join(f"#{t}" for t in tags))
        return "\n".join(header) + "\n\n" + body.lstrip("\n")


# --------------------------------------------------------------------- registry

_BUILTIN: tuple[Connector, ...] = (ObsidianConnector(), Connector())
_registry: dict[str, Connector] = {c.name: c for c in _BUILTIN}


def register(connector: Connector) -> None:
    """Add a connector (tried before the built-ins' catch-all folder one)."""
    if not isinstance(connector, Connector):
        raise TypeError("connectors must subclass kb_core.connectors.Connector")
    if not NAME_RE.match(getattr(connector, "name", "")):
        raise ValueError(f"bad connector name {connector.name!r}")
    if connector.name in {c.name for c in _BUILTIN}:
        raise ValueError(f"cannot replace the built-in {connector.name!r} connector")
    _registry[connector.name] = connector


def available() -> list[Connector]:
    """Connectors in detection order: plugins, then obsidian, then folder."""
    plugins = [c for n, c in _registry.items() if n not in {b.name for b in _BUILTIN}]
    return [*plugins, *_BUILTIN]


def get(name: str) -> Connector:
    try:
        return _registry[name]
    except KeyError:
        raise ValueError(f"unknown connector {name!r} (have: {', '.join(sorted(_registry))})") from None


def connector_for(root: Path) -> Connector:
    """The most specific connector that recognizes ``root``."""
    for c in available():
        try:
            if c.detect(root):
                return c
        except Exception as e:  # noqa: BLE001 - a broken plugin must not stop syncing
            log.warning("connector %s failed to inspect %s: %s", c.name, root, e)
    return _registry["folder"]


def load_plugins(allowed: Iterable[str]) -> list[str]:
    """Load the allowlisted entry points of group ``kb_core.connectors``.
    Returns the names loaded. Unlisted plugins are ignored (and logged)."""
    allowed = {a.strip().lower() for a in allowed if a.strip()}
    from importlib.metadata import entry_points

    loaded: list[str] = []
    for ep in entry_points(group=ENTRY_POINT_GROUP):
        if ep.name.lower() not in allowed:
            log.info("connector plugin %s is installed but not enabled (KB_CORE_CONNECTOR_PLUGINS)", ep.name)
            continue
        try:
            obj = ep.load()
            register(obj() if isinstance(obj, type) else obj)
            loaded.append(ep.name)
        except Exception as e:  # noqa: BLE001 - report, keep the service up
            log.warning("could not load connector plugin %s: %s", ep.name, e)
    for missing in sorted(allowed - {n.lower() for n in loaded}):
        log.warning("connector plugin %s is enabled but was not loaded", missing)
    return loaded


__all__ = [
    "CODE_EXT", "Connector", "FolderConnector", "ObsidianConnector", "available", "connector_for", "get",
    "load_plugins", "parse_front_matter", "register",
]
