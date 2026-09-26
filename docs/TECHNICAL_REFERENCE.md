# OMNIX Technical Reference

The detailed engineering reference for OMNIX and its memory engine, kb-core. It complements:

| Document | Scope |
|---|---|
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Module map, sequence diagrams, key decisions |
| [`SECURITY.md`](SECURITY.md) | Threat model and every security control |
| [`kb-core-contract.md`](kb-core-contract.md) | The REST contract between OMNIX and kb-core |
| [`SERVER_DEPLOYMENT.md`](SERVER_DEPLOYMENT.md) | Installation layouts, operations, troubleshooting |
| [`USER_GUIDE.md`](USER_GUIDE.md) | End-user documentation |
| [`../kb-core/README.md`](../kb-core/README.md) | kb-core overview, CLI and configuration |

## Contents

1. [System overview](#1-system-overview)
2. [Runtime components and ports](#2-runtime-components-and-ports)
3. [Files and locations](#3-files-and-locations)
4. [Backend (Rust) internals](#4-backend-rust-internals)
5. [IPC command reference](#5-ipc-command-reference)
6. [Chat event protocol](#6-chat-event-protocol)
7. [Agent loop and tools](#7-agent-loop-and-tools)
8. [Long-term memory integration](#8-long-term-memory-integration)
9. [Settings reference](#9-settings-reference)
10. [Errors](#10-errors)
11. [Audit log reference](#11-audit-log-reference)
12. [kb-core internals](#12-kb-core-internals)
13. [Extending OMNIX](#13-extending-omnix)
14. [Testing and quality gates](#14-testing-and-quality-gates)
15. [Performance characteristics](#15-performance-characteristics)
16. [Host operations engine](#16-host-operations-engine)

---

## 1. System overview

OMNIX is a Tauri 2 desktop application. A SvelteKit 5 frontend runs in the system webview; a Rust backend owns every
privileged operation. Local services provide the model (Ollama), speech-to-text (Speaches/faster-whisper), text-to-speech
(Piper) and long-term memory (kb-core).

```
┌──────────────────────────── OMNIX process ─────────────────────────────┐
│  Webview (untrusted)                 Rust backend (trust boundary)     │
│  SvelteKit 5 · Tailwind   ──IPC──▶   commands/* ─▶ security/* ─▶ OS    │
│  avatar · chat · views               ai/* (providers, agent loop)      │
│                                      memory/* (kb-core adapter)        │
│                                      mcp.rs · voice.rs · system/*      │
└──────────────┬──────────────┬──────────────┬──────────────┬────────────┘
               ▼              ▼              ▼              ▼
          Ollama :11434  Speaches :8000  Piper (subproc)  kb-core :8100 ──▶ Ollama
                                                               └─ SQLite memory.db
```

Trust model in one sentence: **the webview and the model are untrusted; everything that touches the system passes a
Rust policy decision, a native confirmation where required, and the audit log.**

## 2. Runtime components and ports

| Component | Default address | Run as | Required |
|---|---|---|---|
| OMNIX app | — | desktop app (`/usr/bin/omnix`) | yes |
| Ollama | `127.0.0.1:11434` | `ollama.service` (system) | yes (default provider) |
| Speaches (STT) | `127.0.0.1:8000` | Docker container `omnix-speaches` | for voice input |
| Piper (TTS) | — | subprocess from `~/.local/share/omnix/piper-venv` | for read-aloud |
| kb-core | `127.0.0.1:8100` | `omnix-kb-core.service` (systemd **user** unit) | for memory |
| Grafana Loki | configured URL | external | optional audit shipping |
| MCP servers | stdio or HTTP | user-registered | optional |

With `security.local_only` on (default), every one of these endpoints must resolve to loopback or a private network
(`ai/endpoint.rs`). None of Ollama, Speaches or kb-core should be exposed beyond a firewalled LAN/tailnet; kb-core
refuses to bind off loopback without a bearer token.

## 3. Files and locations

| Path | Content | Mode |
|---|---|---|
| `~/.config/omnix/settings.json` | All settings, no secrets | 600, atomic writes, `*.bak-*` backups by scripts |
| `~/.config/omnix/ops.json` | Alerts, automations, scheduled tasks, activity (signed approvals, no secrets) | 600, atomic writes, protected from executed commands |
| OS keychain (Secret Service / Keychain / Credential Manager) | API keys, MCP secrets, kb-core token | — |
| `~/.local/share/com.paulmmoore.omnix/logs/` | App log, `audit.jsonl` | user |
| `~/.local/share/omnix/piper-venv`, voices | Piper TTS | user |
| `~/.local/share/omnix/kb-core/app` | Installed kb-core code | user |
| `~/.local/share/omnix/kb-core/venv` | kb-core venv (numpy) | user |
| `~/.local/share/omnix/kb-core/memory.db` (+ `-wal`) | Memory database | 600 (dir 700) |
| `~/.config/omnix/kb-core.env` | kb-core configuration | 600 |
| `~/.config/omnix/kb-core.token` | Bearer token (`--lan` only) | 600 |
| `~/.config/systemd/user/omnix-kb-core.service` | kb-core unit | — |
| `~/.local/bin/kb-core` | kb-core CLI wrapper | 755 |
| `/etc/systemd/system/ollama.service.d/omnix.conf` | Ollama drop-in (bind, flash attention, keep-alive) | root |

## 4. Backend (Rust) internals

| Module | Responsibility |
|---|---|
| `lib.rs` | Tauri builder, plugins (dialog, single-instance, autostart, global shortcut, window state, tray), command registration |
| `state.rs` | `AppState`: settings (`RwLock`), HTTP client, secret store, audit log, conversation (`Mutex`), cancel flag, MCP registry |
| `error.rs` | `AppError` (`thiserror`), serialized as `{kind, message}` |
| `settings.rs` | Schema, defaults, validation, `security_changes()` (which changes need native confirmation), secret-flag refresh |
| `commands/*` | Thin IPC wrappers: validate, delegate, map errors |
| `security/policy.rs` | Shell lexer + argv parsing, recursive unwrapping, four risk tiers, sensitive/protected path rules |
| `security/confirm.rs` | Native dialogs built from the parsed request, serialized, default-deny, timeout = deny |
| `security/executor.rs` | `tokio::process`, cleared env, timeout, output cap, process-group kill, audit |
| `security/elevation.rs` | pkexec / macOS admin prompt / UAC (only with `enable_sudo`) |
| `security/files.rs` | Guarded read/list/write; native-dialog file helpers for memory export/import |
| `security/audit.rs` | Hash-chained JSONL, redaction, verification, optional Loki sink |
| `security/secrets.rs` | Keychain abstraction (`SecretStore`), allowed provider ids, migration from plaintext |
| `ai/provider.rs` | `LlmProvider` trait (`id`, `list_models`, `health_check`, `chat_stream`), message/tool types |
| `ai/ollama.rs`, `anthropic.rs`, `openai_compat.rs`, `stream.rs` | Provider implementations and stream parsers (NDJSON, SSE) |
| `ai/agent.rs` | Agent loop, system prompt, tool specs, untrusted wrapping, auto-recall, `remember`, fact capture |
| `ai/context.rs` | Context-window-aware history truncation (≈4 chars/token estimate) |
| `ai/endpoint.rs` | `local_only` egress guard (all resolved addresses must be private) |
| `ai/aiorc.rs` | AIORC gRPC scaffold (`--features aiorc`) |
| `memory/mod.rs` | `MemoryStore` trait (core + optional extensions), record types, `from_state` / `require` |
| `memory/kb_core.rs` | HTTP adapter for kb-core |
| `mcp.rs` | MCP client (rmcp; stdio + streamable HTTP), tool naming `mcp__<server>__<tool>`, policy + audit |
| `ai/metrics.rs` | Agent/model metrics: turns, time-to-first-token, tokens/s (backend-reported via `ChatEvent::Usage`), tool latency, recall, capture |
| `ai/ollama_admin.rs` | Model manager: installed/loaded (`/api/tags`, `/api/ps`), load/unload (`keep_alive`), pull with progress, delete |
| `system/probe.rs` | Fixed-argv read-only host queries (no shell, minimal env, capped, timed out) |
| `system/gpu.rs` | All GPUs: sysfs discovery + `nvidia-smi` + `amdgpu` sysfs counters |
| `system/history.rs` | 1-hour ring buffer of samples (5 s) for sparklines and sustained conditions |
| `system/services.rs`, `system/docker.rs` | List via probes; control/logs via the executor |
| `system/cleanup.rs`, `system/advisor.rs` | Reclaimable-space scan + allowlisted cleanup; measured optimization findings |
| `system/snapshot.rs` | One measured JSON snapshot for `host_status` and AI reports |
| `ops/` | Alerts, automations, scheduler: `model.rs`, `rules.rs` (validation + approvals), `cron.rs`, `approval.rs` (HMAC), `engine.rs` (loop + action runner) |
| `voice.rs` | Speaches STT client (multipart WAV), Piper TTS subprocess (fixed argv) |
| `system/*` | Metrics (sysinfo), processes, kill |
| `observability.rs` | Loki shipping |
| `desktop.rs` | Tray, close-to-tray, global shortcut, microphone permission (WebKitGTK) |

Secret provider ids accepted by the keychain: `openai`, `anthropic`, `gemini`, `xai`, `github`, `google_drive`,
`jira`, `notion`, `kb_core` (and MCP-scoped secrets). Internal ids (`internal.ops_signing`, the approval HMAC key)
are usable from Rust only: every IPC secret command rejects them.

## 5. IPC command reference

All commands return `AppResult<T>`; errors arrive as `{kind, message}` (§10). Arguments are camelCase on the JS side
(`call('update_memory', { id, patch })`). Commands marked 🚧 return `not_implemented` and their UI controls are disabled
(`src/lib/api.ts → NOT_IMPLEMENTED`).

### Chat

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `chat_send` | `message`, `onEvent: Channel<UiEvent>` | `()` | Streams §6 events; resolves when the turn ends. Triggers background fact capture when enabled |
| `chat_cancel` | — | `()` | Stops between stream events / before the next tool |
| `chat_reset` | — | `()` | Clears history (refused while a turn runs) |
| `process_command` | `command` | `string` | Slash commands: `/execute`, `/file read\|list\|write`, `/monitor` (`/search` 🚧) |

### Execution and audit

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `request_execution` | `command`, `cwd?` | `ExecResult {request_id, exit_code, stdout, stderr, truncated, duration_ms, tier}` | Policy → confirm → execute → audit |
| `verify_audit_log` | — | `VerifyReport {valid, entries, …}` | Recomputes the hash chain |
| `select_file` | — | `string \| null` | Native open dialog; the user picks the path |

### Settings and secrets

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `load_settings` | — | `Settings` | Secret flags refreshed from the keychain |
| `save_settings` | `settings` | `Settings` | Validated; security-weakening changes need a native confirmation |
| `reset_settings` | — | `Settings` | |
| `export_settings` / `import_settings` | — | path / `Settings` | Native dialogs; secrets never exported |
| `set_secret` / `delete_secret` / `has_secret` | `provider`, `value?` | `Settings` / `bool` | Write-only: no command returns a secret |
| `test_ai_model` | `config: AiSettings` | `string` | |
| `list_models` | `provider?` | `string[]` | Runtime discovery; no hardcoded ids |
| `list_ollama_models` | `host?` | `string[]` | |
| `test_memory_backend` | — | `string` | kb-core `/health` |
| `mcp_test_server` | `name` | `ServerStatus` | |
| `test_memresort_connection` | `config` | `string` | |

### Knowledge (long-term memory)

| Command | Arguments | Returns | kb-core endpoint |
|---|---|---|---|
| `get_knowledge_data` | — | `KnowledgeData` (counts, stats, memories, documents, activity) | `/memories`, `/stats`, `/documents` |
| `save_memory` | `content`, `tags`, `importance`, `category`, `collection?` | `{id, status, related}` | `POST /memories` |
| `delete_memory` | `id` | `()` | `DELETE /memories/{id}` |
| `update_memory` | `id`, `patch` (`pinned`, `superseded_by: null`, `reviewed`, `importance`, `category`, `tags`, `content`; anything else is refused by `clean_memory_patch`) | memory | `PATCH /memories/{id}` |
| `memory_feedback` | `id`, `helpful` | memory | `POST /memories/{id}/feedback` (ranking only; never hides) |
| `list_hidden_memories` | `limit` | `[{id, content, superseded_by, hidden_reason, reviewed, …}]` (only rows that really are hidden) | `GET /memories?hidden_only=true` |
| `semantic_search` | `query`, `limit`, `collection?` | `[{id, content, score, similarity, kind, source, collection, tags, timestamp}]` | `POST /search` |
| `index_document` | `path`, `collection?` | chunks (`u32`) | guarded read → `POST /documents` |
| `delete_document` | `id` | `()` | `DELETE /documents/{id}` |
| `sync_knowledge_folders` | — | `{folders}` | `POST /sync` |
| `export_knowledge` | — | path or `null` | `GET /export` → native save dialog → file mode 600, audited |
| `import_knowledge` | — | counts or `null` | native open dialog → `POST /import`, audited |
| `optimize_vector_db` | — | `{merged_duplicates, embedded_pending, seconds, storage_bytes}` | `POST /maintenance` |
| `list_knowledge_bases` | — | `[{name, description, memories, documents, chunks, updated_at}]` | `GET /collections` |
| `create_knowledge_base` | `name`, `description` | collection | `POST /collections` |
| `delete_knowledge_base` | `name` | `{items, documents}` | native confirmation → `DELETE /collections/{name}`, audited `kb_delete` |

### System, services and containers

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `get_system_status`, `get_system_info`, `get_real_time_stats`, `get_processes`, `get_gpus` | — | metrics | Read-only |
| `get_performance` | `historySecs?` (≤ 3600, default 900) | `{host, gpus, history, agent, models, memory_store}` | Everything the Overview/Performance tabs show |
| `kill_process` | `pid` | `()` | Confirmed and audited |
| `list_services` | — | `[Service]` (system + user scope, failed first) | Probe |
| `toggle_service` | `scope`, `unit`, `action` (`start/stop/restart/reload/enable/disable`) | `{exit_code, output}` | Executor: Mutating → native confirmation; system units → polkit |
| `service_logs` | `scope`, `unit`, `lines?` | text | `journalctl -u`, read-only, audited |
| `list_containers` | — | `{available, reason, containers}` | Probe (`docker ps`, `docker stats`) |
| `control_container` | `name`, `action` (`start/stop/restart/pause/unpause`) | `{exit_code, output}` | Executor, native confirmation |
| `container_logs` | `name`, `lines?` | text | `docker logs`, read-only, audited |
| `scan_cleanup` | — | `[CleanupItem]` | Sizes only |
| `run_system_cleanup` | `ids` | `[{id, ok, freed, message}]` | One native confirmation for directory categories; commands via executor |
| `optimize_system` | — | `[Recommendation]` | Measured findings |
| `apply_recommendation` | `kind` (`unload_model`, `restart_service`, `kb_maintenance`), `params` | message | Guarded paths only |

### Alerts, automations, scheduler

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `get_system_control_data` | — | `{alerts[{alert, description, value, unit}], automations[…], tasks[…], activity}` | |
| `create_alert` | `input: {name, condition, notify?, cooldown_secs?}` | `Alert` | Validated |
| `create_automation` | `input: {name, trigger, action, cooldown_secs?}` | `Automation` | Command actions: one-time native approval, HMAC-signed |
| `create_scheduled_task` | `input: {name, schedule, action}` | `ScheduledTask` | Same approval rule |
| `toggle_alert` / `toggle_automation` / `toggle_scheduled_task` | `id`, `enabled` | `()` | |
| `delete_rule` | `ruleKind` (`alert/automation/task`), `id` | `()` | Deleting an alert disables automations it triggers |
| `delete_automation` | `id` | `()` | |
| `run_rule_now` | `ruleKind`, `id` | `()` | Same guarded runner as a triggered run |

### Models

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `list_models_detail` | — | `{installed, loaded, configured}` | |
| `model_load` | `model`, `keepAlive?` (`30m`, `2h`, `-1`) | `()` | Audited `model_load` |
| `model_unload` | `model` | `()` | Audited `model_unload` |
| `model_delete` | `model` | `()` | Native confirmation, audited |
| `model_pull` | `model`, `onProgress: Channel<{status, completed, total}>` | `()` | Native confirmation, audited |

### Voice

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `voice_transcribe` | raw WAV body | `string` | Speaches `/v1/audio/transcriptions` |
| `voice_speak` | `text` | WAV bytes | Piper subprocess, audited (`tts`) |

Still 🚧 (`not_implemented`): `test_integration`.

## 6. Chat event protocol

`chat_send` streams `UiEvent` values over a Tauri `Channel`, serialized with a `type` tag:

| `type` | Fields | Meaning |
|---|---|---|
| `token` | `text` | Assistant text fragment |
| `tool_call` | `id`, `name`, `arguments` | A tool was requested (also emitted for automatic `memory_recall`) |
| `tool_result` | `id`, `name`, `ok`, `summary` (≤ 200 chars) | A tool finished |
| `recalled` | `memories: [{id, preview (≤ 160 chars, plain text), origin}]` | The memories auto-recall used this turn (documents excluded); drives 👍/👎 recall feedback |
| `notice` | `message` | Informational (step limit, "Stopped.", MCP warnings, "🧠 Remembered: …") |
| `error` | `message` | The turn failed |
| `done` | — | The turn finished |

**Global event `ops://event`** (Tauri event, any window): `{kind: alert_fired|alert_resolved|automation|schedule,
name, ok, level: info|warning|critical, summary, detail?}`. The main page shows it as a toast and pulses the avatar;
System Control reloads its rules.

A `notice` from fact capture may arrive **after** `done`; the frontend appends it to the same reply. The avatar maps
`tool_call` → amber ripple and a satellite, `tool_result` → green/red, `notice` → white, `error` → red.

## 7. Agent loop and tools

`ai/agent.rs::run_turn`:

1. Lock the conversation (a second concurrent turn is refused), reset the cancel flag.
2. Build the provider (`local_only` enforced), tool specs (built-ins + memory + MCP), system prompt.
3. **Auto-recall** (§8) appends an untrusted memory block to this turn's system prompt.
4. Loop up to `max_rounds` (1, or `max_autonomous_steps` with `autonomous_mode`): stream the reply; collect tool calls;
   execute each through its guarded path; wrap each result as
   `<tool_result tool="…" untrusted="true">…</tool_result>` (closing tags inside escaped, output clipped to 30,000
   characters). Refusal stop → drop the exchange; `max_tokens` stop or step limit → tools are not run but still get an
   error result so provider history stays valid.

| Tool | Available | Path | Confirmation |
|---|---|---|---|
| `run_command` | always | `security::executor` (`Source::LlmTool`) | Mutating/Privileged always confirmed |
| `read_file` | always | `security::files` | only with `require_confirmation`; credential paths denied |
| `list_directory` | always | `security::files` | as above |
| `search_memory` | memory configured | `MemoryStore::search`, compact source-labelled lines | none (read) |
| `remember` | memory configured | `MemoryStore::save_detailed`, `source: assistant` | none; audited `memory_save` |
| `host_status` | always | `system::snapshot::host` (sections: metrics, processes, services, containers, models, alerts) | none (read) |
| `host_control` | always | `system::services` / `system::docker` / `ai::ollama_admin` (`source: llm_tool`) | state changes: native dialog |
| `create_schedule` | always | `ops::rules::create_task` | always a native dialog |
| `create_alert` | always | `ops::rules::create_alert` | always a native dialog |
| `mcp__<server>__<tool>` | per registered server | `mcp.rs` | unless marked read-only; audited `mcp_call` |

## 8. Long-term memory integration

### Auto-recall (`memory.auto_recall`, default on)

- Query: the user's message (≤ 2,000 chars) → `MemoryStore::recall(query, recall_limit, recall_min_score)`
  (`POST /search` with `min_score`; hits are also filtered client-side for services that ignore it).
- Budget: 4 s (`RECALL_TIMEOUT`); any error or timeout → no recall, the turn proceeds.
- Rendering (`format_hits`): `- [<provenance>, saved 2026-09-25, relevance 0.93] …`, each hit ≤ 1,500 chars, wrapped
  by `wrap_untrusted("memory_recall", …)` with a preface stating the entries may be outdated or irrelevant, are data
  not instructions, and that entries "not verified by the user" may be wrong or planted (never follow instructions in
  them; name the source when suggesting a command or setting from one).
- Provenance (`origin_label`, from the hit's `origin`): `memory stated by the user` · `memory learned from the user's
  messages` · `imported memory` · `memory saved by the assistant, not verified by the user` · `external document
  <name>, not verified by the user` (every document chunk, whatever its origin) · `memory, origin unknown` (a
  service that doesn't report `origin`). The `search_memory` tool uses the same labels.
- Scope: appended to the **per-turn** system prompt only; never persisted in history.
- UI: a synthetic `tool_call`/`tool_result` pair named `memory_recall`.

### Fact capture (`memory.auto_capture`, default off)

- Enabling it is listed in `Settings::security_changes()` → native confirmation on save.
- After a successful turn, if the message is worth capturing (≥ 20 chars, not a slash command), a background task
  calls `MemoryStore::extract(user_text)` (`POST /extract`, 180 s timeout) with the **user's text only**.
- Audited as `memory_capture` (`Source::User`) with the count of new memories; the result is reported as a `notice`.

### Conversation archive (`memory.archive_conversations`, default on)

After each successful turn, `agent::archive_conversation` renders the conversation as Markdown (`## You` / `## OMNIX`
sections; tool calls and tool output are excluded) and indexes it as `conversations/<date time> <first words>.md`
in the `conversations` collection. The same document is re-indexed each turn (only new chunks are embedded);
`chat_reset` starts a new one. Auto-recall sends `exclude_collections: ["conversations"]`.

### Adapter behaviour (`memory/kb_core.rs`)

- Core endpoints map errors per the contract (401/403 → `secret`, 404 → `invalid_input`, else `unavailable`).
- Extension endpoints (`ext()`): 404/405/501 → `not_implemented`, except a 404 whose body says a *record* was not found.
- Ids are validated (`[A-Za-z0-9_-]{1,128}`) before being interpolated into URLs.
- The base URL passes `ensure_endpoint_allowed` under `local_only`; the bearer token comes from the keychain (`kb_core`).

## 9. Settings reference

File: `~/.config/omnix/settings.json`. Unknown fields are ignored and missing fields take defaults (`#[serde(default)]`).

### `ai`

| Key | Default | Notes |
|---|---|---|
| `provider` | `"ollama"` | `ollama`, `openai`, `anthropic`, `gemini`, `xai` (cloud blocked by `local_only`) |
| `ollama_host` | `"http://localhost:11434"` | |
| `ollama_model` / `cloud_model` | `""` | Chosen at runtime from discovered models |
| `temperature` | `0.7` | |
| `max_tokens` | `2048` | 1–200,000 |
| `context_window` | `8192` | 512–2,000,000; drives history truncation |
| `stream_responses` | `true` | |
| `has_*_key` | derived | Refreshed from the keychain; never a secret |

### `memory`

| Key | Default | Notes |
|---|---|---|
| `backend_url` | `""` | kb-core URL; empty = memory disabled. `setup-memory.sh` sets `http://127.0.0.1:8100` |
| `auto_recall` | `true` | §8 |
| `recall_limit` | `4` | 1–10 |
| `recall_min_score` | `0.4` | 0–1 (kb-core calibrated relevance) |
| `auto_capture` | `false` | §8; enabling requires native confirmation |
| `archive_conversations` | `true` | §8 conversation archive |
| `max_memory_size`, `auto_summarize`, `retention_days`, `enable_semantic_search` | `1000`, `false`, `90`, `true` | Reserved / planned |

### `security`

| Key | Default | Notes |
|---|---|---|
| `local_only` | `true` | Turning off requires confirmation |
| `enable_sudo` | `false` | Turning on requires confirmation |
| `require_confirmation` | `false` | Also confirm read-only commands and reads |
| `allowed_commands` / `blocked_commands` | `[]` | Allowlist mode when `allowed_commands` is non-empty; built-in denials always win |
| `command_timeout_secs` | `60` | |
| `max_output_bytes` | `1048576` | |
| `confirmation_timeout_secs` | `60` | Timeout = deny |
| `autonomous_mode` / `max_autonomous_steps` | `false` / `10` | Up to 50 |
| `log_all_commands`, `audit_log` | `true` | Informational (always on) |
| `encrypt_memory` | `false` | Unused. Encryption at rest is a kb-core setting: `setup-memory.sh --encrypt` |

### `voice`

| Key | Default | Notes |
|---|---|---|
| `enabled` | `false` | Bootstrap turns it on |
| `stt_url` | `""` | Speaches base URL |
| `whisper_model` | `"base"` | Bootstrap sets a faster-whisper model |
| `language` | `"en"` | |
| `piper_path` / `tts_voice` | `"piper"` / `"en_US-lessac-medium"` | Voice must be an absolute `.onnx` path in practice |
| `tts_engine` | `"piper"` | |
| `wake_word`, `continuous_listening` | `"omnix"`, `false` | Not used: voice is push-to-talk only |

### Others

`general` (`theme "cosmic"`, `language`, `auto_start false`, `notifications true`, `sound_effects true`,
`minimize_to_tray true`, `check_updates false`), `mcp.servers[]` (`name`, `enabled`, `transport`, `read_only_tools`),
`observability.loki_url`, `memresort` (`enabled false`, `host "localhost"`, `port 8080`), `performance` (UI tuning).

## 10. Errors

`AppError` is serialized as `{ "kind": "...", "message": "..." }`:

| kind | Meaning | Typical UI behaviour |
|---|---|---|
| `not_implemented` | Feature not built / not offered by the backend service | Control disabled or "not available" |
| `policy_denied` | Blocked by the policy engine | Red avatar condition "blocked by policy" |
| `not_approved` | User declined or the dialog timed out | Note on the reply |
| `invalid_input` | Bad arguments / not found | Inline message |
| `local_only` | Blocked by local-only mode | Explains how to change it |
| `unavailable` | Ollama, Speaches, kb-core, keychain unreachable | "Connection issue" condition |
| `execution` | Process failed to start / timed out | |
| `secret` | Keychain failure or a token rejected | |
| `io`, `http`, `json`, `internal` | Lower-level failures | |

## 11. Audit log reference

`audit.jsonl`, one JSON object per line:

```json
{"ts":"2026-09-25T21:40:12.345Z","id":"<uuid>","source":"llm_tool","action":"exec","command":"df -h",
 "cwd":"/home/paul","tier":"read_only","decision":"allowed","confirmation":"not_required","exit_code":0,
 "duration_ms":12,"detail":null,"prev_hash":"<sha256 of previous line>"}
```

| Field | Values |
|---|---|
| `source` | `user`, `llm_tool` |
| `action` | `exec`, `read_file`, `list_directory`, `write_file`, `kill_process`, `settings_change`, `mcp_start`, `mcp_call`, `tts`, `memory_save`, `memory_capture`, `memory_export`, `memory_import`, `kb_delete`, `model_load`, `model_unload`, `model_pull`, `model_delete`, `cleanup`, `ops_create`, `ops_approve` |
| `tier` | `read_only`, `mutating`, `privileged`, `denied` |
| `decision` | `allowed`, `denied`, `not_approved`, `failed` |
| `confirmation` | `not_required`, `approved`, `declined`, `timed_out`, `skipped`, `pre_approved` (unattended run of a signed rule; `detail` names the rule) |

`command` and `detail` are redacted (API keys, bearer tokens) before writing. The first line's `prev_hash` is 64
zeros. `verify_audit_log` detects any edited or deleted line except truncation of the tail; Loki shipping anchors the
chain externally.

## 12. kb-core internals

Source: `kb-core/kb_core/`. Standard library + optional numpy. See also the [contract](kb-core-contract.md).

### 12.1 Process model

- `cli.py serve` builds `Config` (env), `KnowledgeBase`, starts the **worker** thread and the **folder-sync** thread,
  then serves HTTP with `ThreadingHTTPServer`.
- One SQLite connection (WAL, `foreign_keys`, `synchronous=NORMAL`) guarded by one `RLock`. Embedding and LLM calls
  happen **outside** the lock.
- The worker: warms the calibration set, then loops (wake on event or every 30 s) running queued judge jobs (LLM
  and/or NLI; with both, a merge or replacement needs both to agree, a disagreement is logged as `judge_disagreed`)
  and the embedding backfill.
- Every other CLI subcommand is an HTTP client of the running service (single writer; the in-memory index stays
  authoritative).

### 12.2 Schema (`PRAGMA user_version = 4`)

v4 adds `items.rejected` (count of the user's "this recalled memory was wrong" flags; rank × 0.6 per flag, at most
four count; ordering only) and `items.judge` (who hid a memory: `llm`, `nli`, `llm+nli`, `similarity`; cleared on
restore). `migrate_v4` is idempotent. An encrypted database (SQLCipher, `KB_CORE_ENCRYPTION=keyring`) has the same
schema; every connection, including the per-thread read-only ones, is keyed with the raw 256-bit key from the OS
keyring (`crypto.py`), and `kb-core encrypt`/`decrypt` convert with `sqlcipher_export` into an integrity-checked
temporary file that atomically replaces the original.

v3 adds `items.reviewed` (0/1, default 0): set when the user keeps a merged or superseded ruling, cleared by every
new ruling and by a restore. `migrate_v3` runs automatically and is idempotent; a v1 database migrates to v3 in one
open.

v2 adds **collections** (named knowledge bases): a `collections(name, description, created_at)` table and a
`collection` column (default `default`) on `items` and `documents`, indexed. `migrate_v2` runs automatically and is
idempotent. Consolidation, duplicate merging and maintenance never cross collections. Search takes `collections`
(include) and `exclude_collections`.


| Table | Purpose |
|---|---|
| `meta(key, value)` | `embed_model` |
| `documents(id, name UNIQUE, mime_type, size, content_hash, content, source_path, source_mtime, chunks, created_at, updated_at)` | Full document text is kept (re-chunking, export) |
| `items(pk INTEGER PK, id UNIQUE, kind 'memory'\|'chunk', doc_id → documents ON DELETE CASCADE, ord, context, content, tags JSON, category, importance, source, pinned, created_at, updated_at, last_accessed, access_count, reinforced, superseded_by, content_hash, vec BLOB, reviewed, rejected, judge)` | Memories and chunks; `vec` NULL = pending embedding |
| `items_fts` (FTS5, external content, `porter unicode61 remove_diacritics 2`) | Columns `content`, `context`, `tags`; kept in sync by triggers |
| `links(src, dst, kind 'related'\|'supersedes', weight, created_at)` | Memory graph, cascades on delete |
| `events(pk, ts, kind, detail JSON)` | Activity for analytics; ids/counts/names only; pruned to 2,000 |

The explicit `pk INTEGER PRIMARY KEY` keeps FTS rowids stable across `VACUUM`.

### 12.3 Vectors

L2-normalised float32 (`array('f')` blobs). `VectorIndex` holds all vectors in memory. With numpy they live in one
preallocated matrix that doubles when full (amortised O(1) appends, no rebuild per write); removal swaps the last row
into the hole (O(1)). `KB_CORE_VECTOR_DTYPE=float16` halves RAM (≈1.5 KB per 768-d vector); scoring is done in
float32 chunks of 65,536 rows. Without numpy, a pure-Python scan.

**Concurrency.** Writes go through one connection under a lock. Reads (search, listings, stats) use per-thread
read-only connections (`file:…?mode=ro`, WAL), so concurrent searches don't queue on the write lock; search takes the
lock only for its access-count update. Query embeddings are cached (LRU, 512 entries). `scan()` also
returns the mean similarity over the whole index (used for calibration).

Embedding (`llm.py`): Ollama `/api/embed`, batches of 32, `truncate: true`. Task prefixes for retrieval models:
`search_document:` / `search_query:` (nomic), a query instruction for mxbai.

### 12.4 Chunking (`chunking.py`)

- Sections by Markdown heading; headings inside fenced code are ignored.
- Breadcrumb `context` = `name › H1 › H2 …`, embedded and indexed with the chunk (`embed_text = context + body`).
- Small (< 350 chars) child or sibling sections fold into the pending chunk, re-rooted at the common parent, with
  `Subheading:` labels.
- Long sections split at paragraph → line → sentence → hard boundaries to ≤ 1,400 chars, carrying a ≤ 200-char
  word-aligned overlap (prefixed `…`).

### 12.5 Retrieval (`KnowledgeBase.search`)

1. **Variants** — `personalize(query, user_name)`: the query, first person rewritten to the owner's name, and to
   "the user" (`I am → X is`, `I have → X has`, `my/mine → X's`, `I/me/myself → X`). Embedded in one call.
2. **Dense candidates** per variant: top `pool = max(40, 8·limit)` from the index (optionally restricted by filters).
3. **Baseline** per variant: `b = (n·μ_corpus + 40·μ_calib) / (n + 40)`, where `μ_corpus` is the query's mean
   cosine over the index and `μ_calib` over 16 fixed neutral sentences. Only candidates above `b` are kept.
4. **Lift**: `lift = (cos − b) / (1 − b)`; an item keeps its best lift across variants.
5. **Lexical candidates**: FTS5 `MATCH` of every non-stopword term, each quoted and OR-joined (FTS syntax in user text
   is neutralised), ranked by `bm25(items_fts, 1.0, 0.6, 0.8)`, top `pool`.
6. **Scoring** for each candidate:

   ```
   lex      = 1 / (1 + 0.25·(bm25_rank − 1))           (0 if no keyword hit)
   coverage = fraction of query terms present (prefix match)
   exact    = 0.5 · lex · coverage²
   sem      = clamp(1.5 · lift, 0, 1)
   rel      = max(0.8·sem + 0.2·lex·min(1, 2·sem), exact)        with a vector
            = max(0.5·lex·coverage, exact)                        keyword-only (no vector / model down)
   rank     = rel · (0.8 + 0.4·activation)   for memories;  rank = rel  for chunks
   ```

   Hits with `rel < min_score` are dropped, and `score` = `rel`. Activation only orders results (`rank`, returned as
   `explain.rank_score`): decay must never hide a memory that answers the query just because it is old or unused.
7. **Diversify** (MMR, λ = 0.72) over the top `4·limit` by `rank`: `λ·rank − (1 − λ)·max_cos_to_selected`, at most 3
   chunks per document.
8. **Track**: returned memories get `access_count + 1`, `last_accessed = now` (unless `track: false`). OMNIX's
   auto-recall sends `track: false` so recall can't reinforce itself (a filter bubble); the `search_memory` tool,
   restating and pinning are what count as use.

Measured on `nomic-embed-text` over a 21-document / 279-chunk corpus: unrelated queries ≤ 0.25, on-topic hits
0.5–1.0; "what are my strongest skills?" → the third-person skill memory rose from 0.33 to 0.62 with personalisation.

### 12.6 Activation

```
age      = now − max(last_accessed, updated_at, created_at)
recency  = 1 if pinned else 0.5^(age_days / half_life)      (half_life = 45 d)
freq     = min(1, ln(1 + access_count + 2·reinforced) / ln 30)
activation = 0.45·importance/10 + 0.35·recency + 0.20·freq   (× 0.3 if superseded)
```

### 12.7 Consolidation

On `save_memory`:

| Condition (cosine to nearest active memory) | Action |
|---|---|
| ≥ `duplicate_threshold` (0.985) | **Reinforce** the existing memory: merge tags, `importance = max`, `reinforced + 1`; adopt the new wording if ≥ 15 % longer. Status `reinforced` / `updated` |
| ≥ `related_threshold` (0.75) | Store; add `related` links; queue an LLM **judge** job (if a chat model is configured) |
| no vector (model down) | Exact (case-insensitive) duplicate check by content hash; otherwise store with `vec = NULL` |

The judge (`_judge_related`) asks the chat model (JSON schema, temperature 0, `think: false`) which older memories
are `duplicate` (→ `_fold_into`: tags, importance, pin, reinforcement, access counts and earliest `created_at` are
copied to the new memory; the old row is kept unchanged and hidden with `superseded_by = new` and a `merged` link) or
`obsolete` (→ `superseded_by = new`, `supersedes` link). Nothing is deleted, because a small model's ruling can be
wrong: `PATCH {"superseded_by": null}` restores either kind, and deleting a memory re-activates everything it merged or
superseded. Hidden rows keep their vectors (restores need no re-embedding) and are excluded from search, consolidation
and the `memories` counts. `GET /memories?hidden_only=true` lists them with `hidden_reason`; import skips them.

Why not vectors alone: on nomic-embed-text, "prefers morning meetings" vs "prefers afternoon meetings" = 0.953, higher
than a true paraphrase (0.951).

`maintenance()` repeats (soft) duplicate folding across all active memories (keeping the older), backfills, prunes events,
runs FTS `optimize`, `PRAGMA optimize` and `VACUUM`.

### 12.8 Fact extraction

`POST /extract`: text (≤ 12,000 chars) → chat model with a JSON schema (≤ 8 memories: `content`, `category` from a
fixed enum, `importance` 1–10, ≤ 5 `tags`). The prompt restricts to durable self-statements, third person with the
owner's name, no secrets, and frames the text as data. Post-filters: 8–1,000 chars and a secret regex (private keys,
`sk-…`, `ghp_…`, AWS keys, Slack tokens, `password: …`). Each fact goes through `save_memory(source="extract")` with
an `auto` tag.

### 12.9 Folder sync

File types: notes (`.md .markdown .mdx .txt .rst .org .adoc`), PDFs (`pdftotext -layout`, fixed argv, 60 s timeout,
≤ 50 MB source), and unless `KB_CORE_INDEX_CODE=0`, source and config files (`.py .rs .go .js .ts .java .c .cpp
.sh .sql .tf .toml .yaml .json .ini .conf …`). Code is chunked by `chunk_code`: boundaries at top-level definitions
(`def/class/fn/impl/struct/func/function/interface …`, one indent level deep for methods), TOML/INI sections, YAML
top-level keys, and shell functions. The breadcrumb is `file › symbol`; small neighbours are merged. MIME types
are `text/x-<ext>`. `KB_CORE_WATCH=name=/path` files a folder into a collection.


Every `KB_CORE_WATCH_INTERVAL` (≥ 10 s, default 120): walk each folder (skipping hidden dirs, `node_modules`,
`venv`, `build`, …, symlinks, licence/changelog boilerplate), extensions `.md .markdown .mdx .txt .rst .org .adoc`,
≤ 5 MiB, UTF-8. Changed mtime → `index_document(name = "<folder>/<relpath>")`; unchanged hash → no-op; files that
disappeared → document deleted. Re-indexing reuses stored vectors for chunks whose `embed_text` hash is unchanged. A
lock serialises timer and `POST /sync` runs.

### 12.10 Resilience

- Embedding failure → `ModelUnavailable`; writes store `vec = NULL`; a 20 s back-off avoids adding timeouts to every
  request; `/health` reports `degraded` with the error; the worker backfills in batches of 64.
- Embedding model change (`meta.embed_model` ≠ config) or dimension change → all vectors cleared and backfilled.

### 12.11 Extra endpoints (v2)

`GET/POST /collections`, `DELETE /collections/{name}` (not `default`), `POST /documents/batch` (≤ 500 documents,
per-document errors), `GET /metrics` (Prometheus text: items by kind, documents, collections, vectors, index
memory, storage, pending embeddings, embedding up/down, searches and latency, saves, documents indexed, uptime;
never content).

### 12.12 HTTP guards (`server.py`)

Order: `Host` allowlist (only without a token) → `Origin` present → 403 → bearer token (constant-time) → body size
(8 MiB, import 64 MiB) → `Content-Type: application/json` → JSON parse → id path-segment validation → dispatch.
`OPTIONS` → 405; no CORS headers; unhandled exceptions → 500 with no traceback. Logs contain method, path, status and
latency only.

## 13. Extending OMNIX

**A new agent tool** — add a `ToolSpec` in `ai/agent.rs::tool_specs`, a match arm in `execute_tool` that routes through
an existing guarded path (never a new unguarded one), mention it in `system_prompt`, add tests, and document it in
`SECURITY.md` §9.

**A new IPC command** — implement in `commands/<area>.rs` (thin; delegate to a domain module), register in `lib.rs`,
add the TypeScript types in `src/lib/types.ts`. Unbuilt? Return `AppError::NotImplemented` and add it to
`NOT_IMPLEMENTED` in `src/lib/api.ts`. Never add Tauri plugin permissions to `capabilities/default.json`.

**A new LLM provider** — implement `LlmProvider` (`id`, `list_models`, `health_check`, `chat_stream` emitting
`ChatEvent`s), wire it into `ai::build_provider`, respect `local_only`, and add stream-parser tests.

**A new network endpoint** — must pass `ai::endpoint::ensure_endpoint_allowed` under `local_only`.

**A different memory backend** — implement the core contract; optional endpoints can be omitted (OMNIX reports them
as `not_implemented`). Or implement `MemoryStore` directly in Rust.

**kb-core features** — engine logic in `store.py`; routes in `server.py::Api`; add tests to
`kb-core/tests/test_kb_core.py` (use `FakeEmbedder`/`FakeChat`), document in `kb-core-contract.md`.

## 14. Testing and quality gates

```bash
cd src-tauri && cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all && cd ..
npm run check && npm test && npm run build
(cd kb-core && python3 -m unittest discover -s tests -t .)
```

| Suite | Count | Covers |
|---|---|---|
| Rust | 148 (+2 opt-in live tests: Ollama, host probes) | Policy bypasses, audit chain tampering, secret migration, provider stream parsers, MCP stdio end-to-end, executor limits, settings validation/security changes, recall-block escaping, NDJSON import parsing, kb-core response shapes |
| Frontend (Vitest) | 29 | API/error helpers, write-only secret field, disabled-state UI, Knowledge view (configured and not, restoring and keeping model rulings), Markdown sanitizer, voice/WAV helpers |
| kb-core | 61 | Soft merges, restore and review, schema migration v1→v3, decay-independent cut-off, hit provenance, import skipping history, chunking, FTS query safety, vector index, consolidation, outage/backfill, model change, supersession/merge judge, extraction filters, search filters/threshold/modes, personalisation, incremental re-index, export/import, maintenance, folder sync, API shapes, HTTP guards and auth |

CI (`.github/workflows/ci.yml`) runs all of the above (kb-core with and without numpy), plus `cargo audit`, `npm audit`,
secret scanning and a three-OS build matrix, with SHA-pinned actions.

## 15. Performance characteristics

Measured on the reference machine (i7, 48 GB RAM, GTX 1060 6 GB, `qwen3:8b` on GPU, `nomic-embed-text`):

| Operation | Time |
|---|---|
| kb-core search, warm (3 query variants, 282 vectors) | ~180 ms |
| First search after a kb-core restart (calibration pre-warmed) | ~67 ms |
| Indexing 21 Markdown documents → 279 chunks (first time) | ~50 s |
| Re-sync with no changes | a few stat() calls; no embedding |
| Fact extraction (`/extract`, 8B model) | 5–10 s, after the reply |
| Auto-recall budget | 4 s hard timeout |

Vector search is exact and in memory: numpy handles ~100k × 768-d vectors in milliseconds; memory use is ≈ 3 KB per
vector.

## 16. Host operations engine

`ops::engine::start` spawns one loop at app start (`SAMPLE_SECS` = 5 s):

1. **Sample**: `Monitor::real_time` + `gpu::snapshot` → `History` (720 samples = 1 h).
2. **Probe**: only if an enabled rule needs them, at most every 30 s: services, containers, Ollama, kb-core.
3. **Alerts**: numeric metrics hold only if every sample in the `sustain_secs` window satisfies the comparison *and*
   the window is fully covered (no firing on a cold start). GPU metrics take a GPU index or "any". Boolean metrics
   (`process_missing`, `service_down`, `container_down`, `ollama_down`, `kb_core_down`) track a `true_since` instant.
   Firing → `alert_fired` activity, `ops://event`, optional desktop notification, and alert-triggered automations.
   `cooldown_secs` limits re-notification. Recovery → `alert_resolved`.
4. **Automations**: condition and idle triggers fire on false → true edges; alert, process start/stop (full process-
   name set diff) and file change (mtime) triggers are already edges; `cooldown_secs` applies.
5. **Scheduler**: due tasks run and `next_run` is recomputed from now; runs more than 24 h late are skipped.
6. **Actions** run in spawned tasks under a 4-slot semaphore: notify; command (HMAC verify → `execute_preapproved`,
   or refuse); AI report (provider without tools, measured snapshot, 5-minute timeout, optional save to kb-core as
   `reports/<date> <name>.md`). Each run updates the rule's `RunState`, the activity feed, emits `ops://event`, and
   (unless it was a successful notify) shows a desktop notification.

**Cron** (`ops/cron.rs`): 5 fields with `*`, lists, ranges, steps, `n/s`; day-of-week 0–7; Vixie OR semantics
when both day fields are restricted; `@hourly/@daily/@weekly/@monthly/@yearly`; `@every <n>m|h|d` (1 min–366 days).
Next-run search jumps by month/day/hour and skips nonexistent DST local times.

**Approvals** (`ops/approval.rs`): RFC 2104 HMAC-SHA256 (tested against RFC 4231 vectors) over length-prefixed
`rule_id`, `command`, `cwd`; constant-time comparison; key generated on first use in the keychain.

