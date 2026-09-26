# kb-core: OMNIX long-term memory

kb-core is the memory engine behind OMNIX. It runs locally as one small service with one SQLite file. It remembers
what matters about you, keeps your notes searchable, and hands the assistant the right context before it answers.
Nothing leaves the machine. Embeddings and the optional "learning" model both run in your local Ollama.

```
OMNIX ──HTTP──▶ kb-core (127.0.0.1:8100) ──▶ SQLite  (memory.db, mode 600)
                    │                     └─▶ Ollama   nomic-embed-text (vectors)
                    │                                   your chat model (fact capture, judge)
                    └── watches ~/notes, … (live re-index)
```

## What it does

| Capability | How |
|---|---|
| **Hybrid search** | BM25 keyword search (SQLite FTS5, Porter stemming) plus exact dense-vector search, merged into one **calibrated 0–1 relevance**. Relevance is the lift over the query's own background similarity, so it means the same thing across embedding models. Nonsense queries score ≤ 0.25 and on-topic hits 0.5–1.0, so OMNIX can threshold safely. |
| **Personalised retrieval** | Questions arrive in first person ("what are my skills"), memories are stored in third person ("Paul's skills"). Each query also runs as a rewrite using your name and "the user", and each item keeps its best match. With nomic-embed-text this doubles the similarity lift. |
| **Memory dynamics** | Every memory has an *activation* built from importance, recency (45-day half-life), how often it was deliberately looked up, and how often it was re-stated. Vivid memories rank higher. Pinned memories never fade. Recall counts as use. |
| **Consolidation** | Near-verbatim restatements merge into the existing memory (tags and importance carry over; the richer wording wins). Similar memories are linked. |
| **Contradiction handling** | Embeddings can't tell "prefers **morning** meetings" from "prefers **afternoon** meetings" (cosine 0.95, higher than a paraphrase). Similar pairs therefore go to the local LLM in the background, which rules each one *duplicate* (folded into the new memory), *obsolete* (superseded), or *compatible*. Either way the older memory is hidden, never deleted, and can be restored, so a bad ruling from a small model is undoable. |
| **Fact capture** | `POST /extract` turns a chat message into zero to eight durable facts with the local LLM, using JSON-schema output. It keeps only what you said about yourself, never secrets (those are filtered again after the model), and runs everything through consolidation. |
| **Knowledge bases** | Named collections (`homelab`, `work`, `conversations`, …). Memories, documents and watched folders can be filed into one; search one, several, or all while excluding some. Merging and contradiction checks stay inside a collection. |
| **Your files, not just notes** | Markdown, text, reStructuredText, Org and AsciiDoc; PDFs via `pdftotext`; source code and config files (Python, Rust, Go, JS/TS, Java, C/C++, shell, SQL, Terraform, TOML, YAML, JSON, INI) chunked at functions, classes and sections, so *"the script that monitors the system"* finds `monitor.sh`. |
| **Built to scale** | A growable in-memory matrix (no rebuild per write, O(1) removal), optional float16 storage (half the RAM), per-thread read-only SQLite connections so searches run in parallel with writes, a query-embedding cache, batch ingest, and Prometheus metrics. |
| **Live folders** | `KB_CORE_WATCH` folders are re-scanned every 2 minutes. Changed files are re-indexed and deleted files removed. Unchanged chunks keep their vectors, so editing one paragraph re-embeds one chunk. |
| **Structure-aware chunking** | Markdown heading breadcrumbs (`notes.md › Backups › ZFS`) are embedded with each chunk. Code fences stay whole. Tiny FAQ sections fold into their parent. Long sections split with overlap. |
| **Diverse results** | Maximal marginal relevance, and at most 3 chunks per document. |
| **Never loses a write** | If the embedding model is down, saves still succeed. The item is keyword-searchable at once and embedded by the background worker when the model is back. Changing the embedding model re-embeds everything in the background (keyword search keeps working). |
| **Analytics** | `/stats` reports counts, categories, sources, storage, search latency, watch status and an activity feed that never contains memory text. |
| **Portable** | `/export` writes NDJSON (memories plus full document text). `/import` merges it, deduplicating as it goes. |

## Install

From the repository root:

```bash
./scripts/setup-memory.sh                 # install/update, start, point OMNIX at it
./scripts/setup-memory.sh ~/notes         # … and keep ~/notes indexed live
./scripts/setup-memory.sh --lan           # serve other machines (bearer token required)
```

This installs the code to `~/.local/share/omnix/kb-core/app` with a private venv (numpy for fast search; kb-core
falls back to pure Python if numpy is missing). It writes `~/.config/omnix/kb-core.env`, starts the systemd **user**
unit `omnix-kb-core`, migrates memories from kb-core 1.x, sets `memory.backend_url`, and installs a `kb-core`
command. Re-running it is safe: it keeps your env file and appends new folders.

## Command line

```bash
kb-core status                         # health, counts, models, watched folders
kb-core search "proxmox backups"       # what the assistant would see
kb-core remember "Paul's NAS is 10.0.0.5" -t homelab -i 7
kb-core extract "I switched to Fedora on my laptop" --dry-run
kb-core ingest ~/some/folder -c work   # one-off index into a knowledge base (watching is better)
kb-core collections                    # knowledge bases with counts
kb-core sync                           # re-scan watched folders now
kb-core export ~/omnix-memory.jsonl    # backup (mode 600)
kb-core import ~/omnix-memory.jsonl
kb-core maintenance                    # merge duplicates, backfill, VACUUM
```

## Configuration (`~/.config/omnix/kb-core.env`)

| Variable | Default | Meaning |
|---|---|---|
| `KB_CORE_HOST` / `KB_CORE_PORT` | `127.0.0.1` / `8100` | Bind address. A non-loopback bind **requires** `KB_CORE_TOKEN_FILE`. |
| `KB_CORE_DB` | `~/.local/share/omnix/kb-core/memory.db` | SQLite file (created with mode 600). |
| `KB_CORE_OLLAMA` | `http://127.0.0.1:11434` | Ollama base URL. |
| `KB_CORE_EMBED_MODEL` | `nomic-embed-text` | Embedding model. Changing it triggers a background re-embed. |
| `KB_CORE_LLM_MODEL` | OMNIX's chat model | Enables `/extract` and the duplicate/contradiction judge. Empty = off. |
| `KB_CORE_USER_NAME` | account's first name | Used for personalised retrieval and in captured memories. |
| `KB_CORE_WATCH` | empty | `:`-separated folders to keep indexed; `name=/path` files a folder into a knowledge base. |
| `KB_CORE_INDEX_CODE` | `1` | Also index source and config files in watched folders (`0` = notes and PDFs only). |
| `KB_CORE_VECTOR_DTYPE` | `float32` | `float16` halves the vector index's RAM for very large stores. |
| `KB_CORE_WATCH_INTERVAL` | `120` | Seconds between scans (min 10). |
| `KB_CORE_TOKEN_FILE` | empty | File holding a bearer token; when set, every request needs it. |
| `KB_CORE_ALLOWED_HOSTS` | empty | Extra `Host` names accepted when no token is set. |
| `KB_CORE_DUPLICATE_THRESHOLD` / `KB_CORE_RELATED_THRESHOLD` | `0.985` / `0.75` | Cosine cut-offs for auto-merge and linking. |
| `KB_CORE_HALF_LIFE_DAYS` | `45` | Recency half-life for activation. |
| `KB_CORE_LOG` | `INFO` | Log level. Logs never contain memory text. |

After editing: `systemctl --user restart omnix-kb-core`.

## API

The contract OMNIX relies on, plus these extensions, is documented in
[`docs/kb-core-contract.md`](../docs/kb-core-contract.md). Quick tour:

```bash
curl -s localhost:8100/health | jq
curl -s localhost:8100/search -H 'Content-Type: application/json' \
     -d '{"query":"what editor do I use","limit":5,"min_score":0.4}' | jq
```

## Security

* **Loopback by default.** Off-loopback binds refuse to start without a bearer token (compared in constant time).
* **Browser-proof.** Any web page can make your browser call `127.0.0.1`, so kb-core rejects requests that carry
  an `Origin` header. It requires `Content-Type: application/json` on bodies (which a form post can't send), never
  answers CORS preflights, and, without a token, checks the `Host` header to defeat DNS rebinding.
* **Private data at rest.** The DB file and exports are mode 600 and the service runs with `UMask=0077`. Logs
  and analytics hold ids, counts and document names, never memory text.
* **Stored text is data.** kb-core never executes or renders it. The LLM prompts frame it as material to analyse
  and tell the model to ignore instructions inside it. OMNIX wraps everything it reads back as untrusted.
* **Model-proposed changes are reversible.** Merged and superseded memories are hidden, not deleted, and can be
  restored (`PATCH /memories/{id}` with `{"superseded_by": null}`, or delete the memory that replaced them). List
  them with `GET /memories?hidden_only=true`. Each ruling counts as `needs_review` in `/stats` until the user restores
  it or keeps it (`PATCH {"reviewed": true}`).
* **Provenance travels with every hit.** Search results carry `origin` (`user`, `extract`, `assistant`, `import`,
  `document`) so the caller can label unverified text as such.
* **Standard library only**, apart from optional numpy (and `pdftotext` for PDFs, run with fixed arguments and a
  timeout). The whole service is about 3,200 lines of Python.

## Development

```bash
cd kb-core
python3 -m unittest discover -s tests -t . -v     # offline: fake embedder + fake LLM
KB_CORE_PORT=8199 KB_CORE_DB=/tmp/kb.db python3 -m kb_core serve
```

Layout: `config.py` (env), `vectors.py` (index + math), `llm.py` (Ollama), `chunking.py`, `store.py` (engine),
`sync.py` (folder watcher), `server.py` (HTTP + guards), `cli.py`.
