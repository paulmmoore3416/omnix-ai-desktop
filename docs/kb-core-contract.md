# kb-core REST contract (as assumed by OMNIX)

> **Status: needs owner confirmation.** The kb-core service's real API
> contract is not in this repository. OMNIX's `KbCoreStore` adapter
> (`src-tauri/src/memory/kb_core.rs`) is written against the minimal contract
> below. If kb-core differs, either adjust the adapter (one file) or add a thin
> compatibility layer in kb-core.

## Connection

* Base URL: **Settings → Memory → kb-core URL** (`memory.backend_url`). Empty
  means memory is disabled.
* With `security.local_only` on (the default), the URL must resolve to a local
  or private-network address (loopback, RFC 1918, Tailscale `100.64.0.0/10`,
  IPv6 ULA).
* Optional auth: `Authorization: Bearer <token>`, where the token is stored in
  the OS keychain under provider id `kb_core` (never in settings).
* All bodies are JSON (`Content-Type: application/json`), UTF-8.
* Timeouts: 30 s (documents: 300 s).

## Endpoints

### `GET /health`

Liveness. Any 2xx means healthy. Body is ignored.

### `GET /memories?limit=<n>`

Most recent memories, newest first. `n` ≤ 500.

```json
{ "memories": [
  { "id": "m_123", "content": "Paul prefers morning meetings",
    "tags": ["prefs"], "importance": 7, "category": "personal",
    "created_at": "2026-09-25T14:00:00Z" }
] }
```

`tags`, `importance`, `category`, `created_at` are optional (defaults: `[]`, `0`, `""`, `null`).

### `POST /memories`

```json
{ "content": "text (1–20000 chars)", "tags": ["a", "b"], "importance": 5, "category": "general" }
```

Response: `{ "id": "m_124" }`. The service is responsible for embedding.

### `DELETE /memories/{id}`

`id` matches `[A-Za-z0-9_-]{1,128}` (OMNIX refuses anything else before
building the URL). Any 2xx = deleted; `404` is reported as "not found".

### `POST /search`

```json
{ "query": "meeting preferences", "limit": 5 }
```

`limit` is clamped to 1–50. Response:

```json
{ "results": [
  { "id": "m_123", "content": "…", "score": 0.83, "tags": [], "created_at": "…" }
] }
```

`score`: higher is more similar. Results may come from memories or document
chunks.

### `POST /documents`

```json
{ "name": "notes.md", "content": "full UTF-8 text", "mime_type": "text/plain" }
```

Response: `{ "id": "d_9", "chunks": 12 }`. The service chunks and embeds. OMNIX
only sends text files ≤ 5 MiB that the user picked in a native file dialog and
that passed the credential-path policy.

## Errors

| Status | OMNIX behaviour |
|--------|-----------------|
| 2xx | success |
| 401 / 403 | "kb-core rejected the token" (`secret` error) |
| 404 | "not found" (`invalid_input`) |
| other | message with the first 300 chars of the body (`unavailable`) |

## Security notes for kb-core

* Treat every memory and document as **untrusted text**. OMNIX wraps
  `search_memory` results as untrusted tool output before the model sees them,
  but kb-core should not execute or render stored content.
* Memories may contain personal data; on healthcare networks keep kb-core on
  the local network (`local_only`) and apply the same data-handling policy as
  other PHI stores.
