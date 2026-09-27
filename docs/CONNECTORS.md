# Knowledge connectors

A **connector** turns a source folder into documents for kb-core. It decides which files belong to the source,
what text of each file gets indexed, and its MIME type. Folder sync (`KB_CORE_WATCH`) and `kb-core ingest` never
read files directly. They always go through the connector for the folder.

Source: `kb-core/kb_core/connectors.py`. List what's available with `kb-core connectors`.

## Built-in connectors

| Name | Detected when | Behaviour |
|---|---|---|
| `obsidian` | the folder has a `.obsidian/` directory | Markdown notes and PDFs. Front matter `title` / `tags` / `aliases` and inline `#tags` become a searchable header (`Title: …`, `Tags: #a #b`). `[[Note\|alias]]` → `alias`, `[[Note#Heading]]` → `Note`, `![[embed]]` → `(embedded: embed)`, and `%% comments %%` are dropped. `.obsidian/` and `.trash/` are skipped. |
| `folder` | always (the fallback) | Notes (`.md .txt .rst .org .adoc …`), PDFs via `pdftotext`, and source/config files unless `KB_CORE_INDEX_CODE=0`. Hidden and build directories are skipped. |

Document names are always `<folder name>/<relative path>`, so a folder can change connector without duplicating
documents. `/stats` → `watch.connectors` shows which connector each watched folder uses.

The front-matter reader handles a small YAML subset (`key: value`, `[a, b]`, `a, b`, and `- a` lists) for the
keys above only. It never evaluates YAML tags, so `!!python/object` and similar are plain text.

## The contract

```python
from pathlib import Path
from typing import Iterator
from kb_core.connectors import Connector

class LogseqConnector(Connector):
    name = "logseq"                      # [a-z][a-z0-9_-]{0,31}
    description = "Logseq graph: pages and journals"

    def detect(self, root: Path) -> bool:        # recognise the source
        return (root / "logseq" / "config.edn").is_file()

    def iter_files(self, root: Path, include_code: bool = True) -> Iterator[Path]:
        for sub in ("pages", "journals"):
            yield from sorted((root / sub).glob("*.md"))

    def read(self, path: Path) -> str | None:    # text to index, or None to skip
        text = path.read_text(encoding="utf-8")
        return text.replace("- ", "", 1)

    def mime(self, path: Path) -> str:
        return "text/markdown"
```

Rules:

- `iter_files` yields real files under `root`: no symlinks, nothing outside the folder.
- `read` returns plain text. It's chunked and embedded as **data**, and never executed or rendered as HTML.
- Keep `detect` cheap and side-effect free. It runs on every sync pass. An exception there is logged and the next
  connector is tried.
- A connector can't replace a built-in (`folder`, `obsidian`). Detection order is plugins, then `obsidian`, then
  `folder`.

## Shipping a connector as a plugin

Package it and declare an entry point in the `kb_core.connectors` group:

```toml
# pyproject.toml of your package
[project.entry-points."kb_core.connectors"]
logseq = "kb_logseq:LogseqConnector"
```

Install it into kb-core's venv (`~/.local/share/omnix/kb-core/venv/bin/pip install ./kb-logseq`), then **enable it
by name** in `~/.config/omnix/kb-core.env` and restart kb-core:

```
KB_CORE_CONNECTOR_PLUGINS=logseq
```

Plugins run inside kb-core with full access to the memory database, so installing one isn't enough to load it.
Only entry points named in `KB_CORE_CONNECTOR_PLUGINS` are loaded. Others are logged as "installed but not
enabled".
