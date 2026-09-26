# kb-core REST contract

> OMNIX's `KbCoreStore` adapter (`src-tauri/src/memory/kb_core.rs`) talks to a
> kb-core service over this contract. The reference implementation lives in
> [`kb-core/`](../kb-core/README.md) (Python standard library + optional numpy,
> SQLite, Ollama embeddings) and is installed by `scripts/setup-memory.sh`.
>
> The **core** endpoints are required. The **extension** endpoints are optional:
> when a service answers 404/405/501 for one, OMNIX reports that feature as
> `not_implemented` and keeps working. So any service that implements the core
> (for example a PostgreSQL/pgvector kb-core) can be used instead.

## Connection

* Base URL: **Settings → Memory → kb-core URL** (`memory.backend_url`). Empty
  means memory is disabled.
* With `security.local_only` on (the default), the URL must resolve to a local
  or private-network address (loopback, RFC 1918, Tailscale `100.64.0.0/10`,
  IPv6 ULA).
* Optional auth: `Authorization: Bearer <token>`, where the token is stored in
  the OS keychain under provider id `kb_core` (never in settings). The bundled
  service requires one when bound off loopback.
* All bodies are JSON (`Content-Type: application/json`), UTF-8. OMNIX never
  sends an `Origin` header; the bundled service rejects requests that do.
* Timeouts: 30 s (documents 300 s, extraction 180 s, import/sync 1800 s).

## Core endpoints

### `GET /health`

Liveness. Any 2xx means healthy. The bundled service returns
`{status: "ok"|"degraded", memories, documents, chunks, pending_embeddings, embed_model, embed_error}`.

### `GET /memories?limit=<n>`

Most recent memories, newest first. `n` ≤ 500. Superseded memories are excluded
(extension: `include_superseded=true`, `offset`, `category`, `order=activation`).

```json
{ "memories": [
  { "id": "m_123", "content": "Paul prefers morning meetings",
    "tags": ["prefs"], "importance": 7, "category": "preference",
    "created_at": "2026-09-25T14:00:00Z",
    "source": "user", "pinned": false, "reinforced": 2, "access_count": 5, "activation": 0.81 }
] }
```

Only `id` and `content` are required. `tags`, `importance`, `category` and
`created_at` default to `[]`, `0`, `""` and `null`. The last line holds
optional extension fields.

### `POST /memories`

```json
{ "content": "text (1–20000 chars)", "tags": ["a", "b"], "importance": 5, "category": "general",
  "source": "user" }
```

The service is responsible for embedding. The response must contain `id`. The
bundled service adds consolidation info:

```json
{ "id": "m_124", "status": "created", "embedded": true,
  "related": [{ "id": "m_77", "score": 0.84, "content": "…" }] }
```

`status` is `created`, `reinforced` (a near-verbatim duplicate: the existing
memory's id is returned and strengthened) or `updated` (the duplicate carried
more detail, so the existing memory adopted the new wording). `source`
(`user`, `assistant`, `extract` or `import`) is an optional extension; OMNIX
sends `assistant` for the model's `remember` tool.

### `DELETE /memories/{id}`

`id` matches `[A-Za-z0-9_-]{1,128}` (OMNIX refuses anything else before
building the URL). Any 2xx means deleted; `404` is reported as "not found".
Memories the deleted one had superseded become active again.

### `POST /search`

```json
{ "query": "meeting preferences", "limit": 5 }
```

`limit` is clamped to 1–50. Response:

```json
{ "results": [
  { "id": "m_123", "content": "…", "score": 0.83, "tags": [], "created_at": "…",
    "similarity": 0.80, "kind": "memory", "source": "memory", "category": "preference",
    "importance": 7, "superseded_by": null,
    "explain": { "semantic_rank": 1, "keyword_rank": 3, "activation": 0.81 } }
], "degraded": false }
```

`score` is relevance: higher is better. The bundled service calibrates it to
0–1: lift over the query's background similarity, a boost for exact keyword
coverage, and memory activation. Unrelated text scores ≤ 0.25; on-topic hits
score 0.5–1. Results may be memories or document chunks. For a chunk,
`content` starts with its breadcrumb (`notes.md › Backups › ZFS`), `kind` is
`document`, and `source` is the document name. `degraded: true` means the
embedding model was unreachable and results are keyword-only.

Optional request fields (extension): `min_score` (0–1; OMNIX's auto-recall
sends `memory.recall_min_score`, default 0.4), `kinds` (`["memory"]`,
`["document"]`), `tags`, `category`, `mode` (`hybrid`, `semantic`,
`keyword`), `include_superseded`, and `track` (default true; recalls count as
use of a memory).

### `POST /documents`

```json
{ "name": "notes.md", "content": "full UTF-8 text", "mime_type": "text/plain" }
```

Response: `{ "id": "d_9", "chunks": 12 }` (bundled: plus `status`
`created|updated|unchanged`, `embedded_new`, `reused`, `pending`). The
service chunks and embeds. A document with the same name is replaced; identical
content is a no-op. OMNIX only sends text files ≤ 5 MiB that the user picked in
a native file dialog and that passed the credential-path policy.

## Extension endpoints (bundled kb-core)

| Endpoint | Purpose | OMNIX uses it for |
|---|---|---|
| `GET /stats` | Counts, categories, sources, storage, models, search latency, watch status, activity feed (no memory text) | Knowledge view header and Analytics |
| `GET /memories/{id}` | One memory plus its links (`related`, `supersedes`) | — |
| `PATCH /memories/{id}` | Edit `content`, `tags`, `importance`, `category`, `pinned`; `{"superseded_by": null}` restores a superseded memory | Pin button |
| `GET /documents` | `{documents: [{id, name, mime_type, size, chunks, source_path, pending, …}]}` | Documents tab |
| `GET /documents/{id}` | Full text plus chunks | — |
| `DELETE /documents/{id}` | Remove a document and its chunks | Documents tab |
| `POST /extract` | `{text, dry_run?}` → `{memories: [{id, status, content}]}`. The local LLM captures durable facts and runs them through consolidation. 501 if no chat model is configured. | `memory.auto_capture` |
| `GET /export` | NDJSON: a header line, then `memory` and `document` records | Export button (file saved mode 600) |
| `POST /import` | `{records: [...]}` from an export; memories are deduplicated against existing ones | Import button |
| `POST /maintenance` | Merge duplicates, embed pending items, prune the activity log, optimise FTS, VACUUM | Optimize button |
| `POST /sync` | Re-scan the watched folders now | "Sync folders" button |
| `GET /collections` | `{collections: [{name, description, memories, documents, chunks, created_at, updated_at}]}` | Knowledge Bases tab |
| `POST /collections` | `{name, description}` → the collection (creates or re-describes) | Create knowledge base |
| `DELETE /collections/{name}` | Delete a collection and everything in it (`default` can't be deleted) → `{items, documents}` | Delete knowledge base (after a native confirmation) |
| `POST /documents/batch` | `{documents: [{name, content, mime_type, collection}]}` (≤ 500) → per-document results | — |
| `GET /metrics` | Prometheus text (counts and timings only) | — (for Grafana/Prometheus) |

Collection-aware fields (optional everywhere): `collection` on `POST /memories` and `POST /documents` (default
`default`), `GET /memories?collection=` and `GET /documents?collection=`, and `collections` / `exclude_collections`
on `POST /search`. OMNIX's auto-recall excludes `conversations` (its own chat archive).

## Errors

| Status | OMNIX behaviour |
|--------|-----------------|
| 2xx | success |
| 401 / 403 | "kb-core rejected the token" (`secret` error) |
| 404 | core endpoints: "not found" (`invalid_input`). Extension endpoints: "not found" if the body says a record wasn't found, else `not_implemented` |
| 405 / 501 on an extension | `not_implemented` (the control is shown as unavailable) |
| other | message with the first 300 chars of the body (`unavailable`) |

Error bodies are `{ "error": "message" }`.

## Security notes for kb-core implementations

* Treat every memory and document as **untrusted text**. OMNIX wraps
  `search_memory` results and the automatic recall block as untrusted data
  before the model sees them, but kb-core must not execute or render stored
  content.
* Memories may contain personal data. Keep kb-core on loopback (or on a
  firewalled LAN/tailnet with a bearer token), create files private, and keep
  memory text out of logs. On healthcare networks apply the same
  data-handling policy as other PHI stores.
* Loopback services are reachable from any web page through the user's
  browser. Reject requests with an `Origin` header, require
  `application/json`, and validate `Host`.
