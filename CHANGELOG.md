# Changelog

All notable changes to OMNIX will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.1.0] - 2026-09-26

### Docs

- `docs/REMOTE_ACCESS.md`: proposed design for using OMNIX from a phone over Tailscale (gateway inside the Rust
  backend, QR pairing with hardware-held device keys, WebSocket API, desk-only approvals first). Nothing is built yet.

### Memory hardening (external design review)

**Changed**
- **Merges are soft.** When the LLM judge (or `/maintenance`) rules two memories duplicates, the older one is now
  hidden with a `merged` link instead of deleted, exactly like a superseded memory. `PATCH {"superseded_by": null}`
  or deleting the surviving memory restores it unchanged. Previously a wrong ruling by a small local model lost the
  memory permanently.
- **Decay no longer filters.** `min_score` and `score` use calibrated relevance only; activation (importance,
  recency, use) orders results (`explain.rank_score`) but can't push an old, rarely used memory that answers the
  query below the cut-off.
- `/health` and the knowledge-base list count active memories only (hidden ones are in `/stats` → `superseded`).
- **Auto-recall no longer counts as use** (`track: false`). It runs on every message, so counting its hits let a
  memory recalled once rank higher and be recalled again. Deliberate lookups, restating and pinning still count.

**Added**
- **Provenance labels in recall.** Search hits carry `origin` (`user`, `extract`, `assistant`, `import`,
  `document`). The recall block and `search_memory` label each entry ("memory stated by the user", "external document
  <name>, not verified by the user", …), and the model is told unverified entries may be planted and must be cited
  when it suggests a command or setting from them.
- Knowledge → Memories → **Merged & replaced memories**: lists hidden memories with why they were hidden, with a
  **Restore** button (`list_hidden_memories`, `GET /memories?hidden_only=true`).
- **Review of model rulings.** Every merge or supersession counts as "to review" until the user restores it or
  presses **Keep** (`PATCH {"reviewed": true}`). A banner in the Knowledge view shows the count, unchecked rulings
  are listed first, and Analytics shows *Awaiting your review*. kb-core schema v3 (`items.reviewed`, migrated
  automatically); `/stats` → `needs_review`.
- `clean_memory_patch`: the IPC whitelist for memory edits is a tested function; the webview can restore a memory or
  mark a ruling reviewed but never hide one.

- **Second judge for consolidation (optional).** `setup-memory.sh --nli` installs a small NLI cross-encoder
  (`nli-deberta-v3-xsmall`, ONNX int8, CPU, ~40 ms per pair; pinned revision, SHA-256-checked). It rules
  duplicates (entailment ≥ 0.9, no contradiction either way) and replacements (contradiction ≥ 0.8 in *both*
  directions). With the LLM judge also on, a merge or replacement needs both to agree; disagreements keep both
  memories live and are counted (`/stats` → `judge_disagreements_30d`). Hidden memories show who decided
  (`judged_by`). kb-core schema v4 (`items.rejected`, `items.judge`, migrated automatically).
- **Recall feedback.** Replies list the memories auto-recall used (**🧠 Memories used**, new `recalled` chat event)
  with 👍/👎. 👎 (`memory_feedback` → `POST /memories/{id}/feedback`) multiplies the memory's rank by 0.6 per flag,
  so a heavily used but wrong memory stops winning; it is never hidden and still answers when it is the only match.
  Only the user restating the fact clears the flags. The Knowledge view shows 👎 *n*; `/stats` → `flagged_wrong`.
- **Encryption at rest (opt-in).** `setup-memory.sh --encrypt` / `kb-core encrypt` convert the database to SQLCipher
  (whole file, so FTS5 keeps working) with a random 256-bit key kept only in the OS keyring; `kb-core decrypt`
  reverses it. Conversion is integrity-checked and atomic. `doctor.sh` reports the judges, encryption and the
  review backlog.

**Fixed**
- Import no longer brings merged or superseded history back as live memories (`history_skipped` in the result).

### System Control, automation and host control

**Added**
- **GPU telemetry for every GPU**: sysfs discovery (vendor, driver, PCI slot) plus `nvidia-smi` for NVIDIA and
  `amdgpu` sysfs counters for AMD. Includes load, VRAM, temperature, power, fan, clocks and per-process VRAM. On the
  reference machine this surfaced the RX 570/580 (8 GB) that `nvidia-smi` can't see.
- **Detailed host metrics** (per-core CPU, load average, frequency, per-disk, per-interface network, all sensors) and
  a 1-hour metrics history with sparklines. The Performance tab replaces random bars and a hardcoded "85%" score.
- **Agent and model metrics**: replies, errors, time to first token, tokens/s from Ollama's reported counts (new
  `ChatEvent::Usage`), cold starts, per-tool calls/failures/latency, recall hit rate, facts learned.
- **Services & Docker**: list systemd units (system + user) and containers; start/stop/restart/logs via the executor
  (policy + native confirmation + audit; system units ask polkit).
- **Model manager**: installed/loaded models with VRAM/RAM split, load/unload, download with progress, delete
  (confirmed), "use for chat".
- **Alerts, automations, scheduler** (`ops/`): sustained-condition alerts with desktop notifications
  (`tauri-plugin-notification`, driven from Rust only); triggers → notify/command/AI report; cron schedules with
  presets and a weekday "morning briefing"; activity feed; run now.
- **Unattended command safety**: one-time native approval stored as HMAC-SHA256 over rule id, command and cwd, with
  the key in an IPC-unreachable keychain entry. Commands are re-classified at every run and executed via
  `executor::execute_preapproved` (never elevated; audited as `confirmation: pre_approved`). Privileged/denied
  commands can't be scheduled; `ops.json` is protected from commands.
- **Cleanup & Optimize**: allowlisted cache/trash cleanup (one confirmation, symlink-safe, audited), Docker prune
  and journal vacuum through the executor; measured recommendations (models spilling to CPU, idle models, failed
  services, full disks, heat, memory pressure, memory-store upkeep) with guarded one-click fixes.
- **Agent host tools** `host_status`, `host_control`, `create_schedule`, `create_alert` (rules proposed by the model
  always need a native confirmation).
- **Conversation archive** (`memory.archive_conversations`, default on): user/assistant text (never tool output) is
  saved to the `conversations` knowledge base, which auto-recall excludes.
- **Knowledge bases**: kb-core collections exposed in OMNIX (create, delete with confirmation, file memories and
  documents, search one or all).
- kb-core 2.1: collections (schema v2 with automatic migration), growable/float16 vector index with O(1) removal,
  per-thread read connections, query-embedding cache, `POST /documents/batch`, `GET /metrics` (Prometheus), PDF and
  code/config indexing with code-aware chunking, `KB_CORE_WATCH=name=/path`, `kb-core collections`, `ingest -c`.

**Fixed**
- `kb-core export` wrote 0 records (the CLI read the HTTP response after it was closed); now tested end to end.

**Changed**
- `NOT_IMPLEMENTED` now holds only `test_integration`; the System Control "not yet available" banner is gone.

### Documentation

**Added**
- `docs/TECHNICAL_REFERENCE.md`: runtime components and ports, file locations, backend module reference, every IPC
  command, the chat event protocol, agent tools, memory integration, full settings reference with defaults, error
  kinds, audit-log schema, kb-core internals (schema, retrieval and scoring formulas, activation, consolidation,
  extraction, folder sync, HTTP guards), extension guide, test suites and measured performance.
- User Guide §11–13: the Knowledge view tab by tab, getting the most out of memory, and a privacy FAQ.
- Project Overview: kb-core capabilities (§3.12) and "What makes OMNIX different" (§5).

**Changed**
- Project Overview, Showcase and README refreshed: kb-core is no longer described as PostgreSQL/pgvector, test counts
  updated, export/import removed from the roadmap.
- Docs synced with the current tree: test counts (144 Rust + 2 opt-in, 27 Vitest, 46 kb-core) and line counts in the
  Project Overview, Showcase and article; Architecture module map and plugin table cover `system/*`, `ops/*`,
  the model manager and the notification plugin; README architecture diagram, kb-core test command and public-repo
  clone line; CONTRIBUTING lists the full quality gate. Showcase gains a System Control highlight.
- `.gitignore` excludes Python bytecode (`__pycache__/`) and the local `itsme-knowledge-base/` test corpus (a
  separate repository).

### Long-term memory: kb-core 2 and OMNIX integration

**Added**
- `kb-core/`: a local long-term memory engine implementing `docs/kb-core-contract.md` plus extensions. It uses
  standard-library Python (numpy optional), one SQLite file (mode 600) and Ollama embeddings.
  - Hybrid retrieval: FTS5/BM25 plus exact dense vectors with a **calibrated 0–1 relevance** (lift over the query's
    background similarity, self-calibrating per embedding model), exact-keyword coverage, and MMR diversity
    (≤ 3 chunks per document).
  - Personalised multi-query retrieval: first-person questions are also matched as "<owner>'s …" / "the user's …"
    (doubles the similarity lift for third-person memories).
  - Memory dynamics: activation from importance, recency (half-life), recall count and reinforcement; pinning.
  - Consolidation: near-verbatim duplicates are reinforced instead of stored twice, and similar memories are linked.
    The local LLM judges similar pairs as duplicate (merged) or obsolete (superseded, restorable). A calibration run
    showed "prefers morning" vs "prefers afternoon" meetings at cosine 0.95, which is why contradictions are never
    auto-merged on vectors.
  - `POST /extract`: fact capture with JSON-schema output, with secrets filtered.
  - Live folder sync (`KB_CORE_WATCH`): incremental re-index, and unchanged chunks keep their vectors.
  - Resilience: writes succeed with the embedding model down (backfilled later); changing the model re-embeds in the
    background.
  - Extensions: `/stats`, `/documents` (list/get/delete), `PATCH /memories/{id}`, `/export`, `/import`,
    `/maintenance`, `/sync`.
  - Hardening: loopback by default, token required off-loopback, `Origin` rejected, JSON-only bodies, `Host` check,
    no CORS, memory text never logged.
  - `kb-core` CLI (`status`, `search`, `remember`, `extract`, `ingest`, `sync`, `export`, `import`, `maintenance`,
    `migrate-legacy`). 36 offline unit tests run in CI with and without numpy.
- OMNIX **auto-recall** (`memory.auto_recall`, default on): relevant memories and notes are added to that turn's
  system prompt as untrusted data (limit/threshold configurable, 4 s timeout) and shown as a `memory_recall` step.
- `remember` agent tool (audited as `memory_save`, tagged `source: assistant`).
- **Learn from conversations** (`memory.auto_capture`, default off; turning it on requires native confirmation):
  after each reply, facts from the user's message are captured in the background and reported as
  "🧠 Remembered: …". Audited as `memory_capture`.
- Knowledge view: real analytics (categories, sources, engine health, activity feed), document management and
  watched folders, pin/unpin, memory provenance and activation, relevance scores in search.
- Knowledge → **Export / Import / Optimize** now work. Export is written mode 600 to a path chosen in a native
  dialog, and both directions are audited.
- `scripts/setup-memory.sh`: venv install, env file, systemd user unit, one-time migration of kb-core 1.x memories,
  `--lan` with a generated token, `kb-core` CLI. `bootstrap.sh` runs it (`--no-memory` to skip); `doctor.sh`
  checks kb-core health, the embedding backlog, the learning model and watch-folder errors.

**Changed**
- `MemoryStore` gains optional extension methods (default `not_implemented`), so OMNIX still works with a
  contract-only kb-core.
- `search_memory` returns compact, source-labelled lines instead of raw JSON.

**Fixed**
- The Knowledge view's Analytics tab showed random numbers and invented activity ("2m ago"); it now shows real data
  or says analytics are unavailable.
- Settings → Memory described kb-core as PostgreSQL/pgvector on port 8000.

### One-command deployment and docs refresh

**Added**
- `scripts/bootstrap.sh`: idempotent end-to-end setup for Ubuntu/Debian. Installs system packages, Rust,
  Node 24, npm/cargo dependencies; Ollama as a systemd service (flash attention, keep-alive drop-in) with
  `qwen3:8b` + `qwen3:14b`; Speaches speech-to-text in Docker on the smallest NVIDIA GPU (CUDA image, model
  pre-downloaded); Piper TTS in a private venv with a voice; seeds `~/.config/omnix/settings.json` (fills
  blanks only, keeps a backup, mode 600); builds and installs the `.deb`. Flags: `--services-only`,
  `--no-services`, `--lan`, `--cpu`, `--yes`. The NVIDIA driver is deliberately not auto-installed.
- `scripts/doctor.sh`: read-only health check (toolchains, binary, GPUs, settings, Ollama round-trip, STT
  model, Piper voice, container, Secret Service); non-zero exit on failure.
- `docs/SERVER_DEPLOYMENT.md`: layouts (all-in-one vs GPU server + clients), GPU/VRAM plan for 8 GB + 6 GB
  cards, LAN firewalling, operations, uninstall, troubleshooting.
- `CLAUDE.md`: runbook for Claude Code agents (install procedure, human-only steps, diagnosis table, quality
  gate, security invariants).

**Changed**
- README quick start is now bootstrap-first; User Guide setup/voice/troubleshooting, Project Overview and
  Showcase updated for the holographic avatar, Speaches and the new scripts. `scripts/setup.sh` adds
  `libxdo-dev` and points apt users to `bootstrap.sh`.
- Documented that Piper's voice setting must be an absolute `.onnx` path (a bare voice name does not resolve
  when OMNIX launches Piper).
- The stale 1.0.0 "Planned Features" list is now a roadmap with shipped items marked.

### Voice input reliability and avatar animation

**Fixed**
- Microphone button: releasing before the microphone finished opening left a
  recording running forever; Ctrl+Space key auto-repeat could start several
  recordings at once. Push-to-talk is now a single state machine
  (`idle → starting → recording → transcribing`).
- A quick click recorded ~0 s of audio and failed. Tapping now toggles
  recording (tap to start, tap to send); holding still works. Clips under
  0.4 s are rejected with a clear hint.
- The mic button was silently disabled when voice was not configured. It now
  shows a setup badge and opens Settings → Voice with an explanation.
- Recording no longer depends on MediaRecorder codecs in the webview: audio is
  captured with Web Audio and encoded in-app to 16 kHz mono WAV (MediaRecorder
  remains a fallback). getUserMedia errors are mapped to actionable messages.
- Read-aloud now keeps the avatar speaking until playback actually ends.

**Added**
- Live input-level ring and meter, recording timer, Esc to cancel, 2-minute
  safety cap, keyboard (Enter/Space) control of the mic button.
- Settings → Voice → **Test microphone** (level check + round-trip transcription).
- Avatar rewritten around a damped target-pose rig: smooth transitions between
  every activity, spring squash-and-stretch, eased steering locomotion,
  transform-only rendering and colour cross-fades. New behaviours: mic-reactive
  listening, thinking pose, typing on a laptop, lip-sync, confetti
  celebration, head shake on error, look-around, stretch/yawn, bow, backflip,
  jetpack flights, naps, cursor-tracking eyes and click-to-play. Honours
  prefers-reduced-motion.
- `docs/USER_GUIDE.md` and `docs/PROJECT_OVERVIEW.md`.
- Avatar redesigned as a JARVIS-style holographic core: rotating segmented and
  tick rings, radar sweep, voice-reactive waveform ring, orbiting data motes,
  bloom core that tracks the pointer, HUD readouts (mode, link, CPU/MEM) and
  scanlines. Three colour layers defined once in `src/lib/avatar.ts`: 13 moods
  (core), 4 conditions (outer halo + banner: backend offline, high load,
  blocked by policy, connection issue) driven by status polling and
  `AppError.kind`, and 4 signal ripples for agent tool/notice events. In-app
  colour key (`AvatarKey.svelte`, "KEY" button) and a key in the user guide.
- Avatar extras: boot sequence (core ignites, rings power on inside-out),
  decoding status text, cursor target-lock reticle with coordinates/range,
  orbiting labelled satellites for live tool calls (paired call → result via
  the tool-call id; amber → green/red, then fade), drag-to-spin rings with
  flick inertia, and neural links between data motes while thinking.
  `Signal` gained optional `label`/`ref`.

### Phase 1: Security lockdown (`fix/phase-1-security`)

**Changed**
- Split the 934-line `main.rs` into a library crate: `error`, `state`,
  `settings`, `commands/*`, `security/*` (policy, confirm, executor, elevation,
  files, audit, secrets), `ai/*`, `system/*`. `main.rs` now only calls
  `omnix_lib::run()`. Global `Lazy<Mutex<..>>` statics replaced by Tauri managed
  state; one shared `reqwest::Client`.
- Commands return a structured `AppError` (`{kind, message}`) instead of strings.
- Settings wire/disk format is snake_case end-to-end (the UI previously sent
  camelCase, so **saving settings never worked**), every section is
  `#[serde(default)]` (older files missing `integrations` failed to load), and
  writes are atomic with mode 0600.
- `security.enable_sudo` now defaults to **false**; existing v1 files are reset
  once (schema v2) because elevation is now real.
- New `security.local_only` (default **true**) hard-blocks cloud providers and
  non-local endpoints in Rust.
- `.env.example`: `ENABLE_SUDO=false`, `ENABLE_AUTONOMOUS_MODE=false`, and a
  note that the app does not read `.env`.
- README rewritten to describe only implemented features; performance table
  removed; voice, memory, cloud chat and integrations marked planned.

**Security**
- Removed `execute_system_command`, `read_file`, `write_file`,
  `list_directory` from the IPC surface. The only execution entry point is
  `request_execution` → policy engine → native confirmation → hardened spawn →
  audit.
- Policy engine with ReadOnly / Mutating / Privileged / Denied tiers, a
  quote-aware lexer, recursive unwrapping of `sudo`/`env`/`sh -c`/`eval`/
  `xargs`/`find -exec`/substitutions, built-in non-overridable deny rules, user
  blocklist and allowlist mode. 32 unit tests including bypass attempts.
- Native Rust-side confirmation dialogs (default deny, timeout = deny,
  serialized). Security-weakening settings changes also require confirmation.
- Executor: `tokio::process`, `kill_on_drop`, cleared environment, timeout with
  process-group kill, 1 MiB output cap, structured results. Elevation via
  pkexec / osascript / UAC behind `enable_sudo`.
- Hash-chained, redacted JSONL audit log in the app log dir with
  `verify_audit_log`.
- API keys moved to the OS keychain (`keyring`); `set_secret` /
  `delete_secret` / `has_secret` only; automatic migration of plaintext keys;
  write-only key fields in Settings.
- Strict CSP; capabilities reduced to core defaults; shell, fs, opener,
  notification and clipboard plugins removed (Rust and npm).
- `tracing` replaces all `println!`/`eprintln!`.

**Honest stubs**
- Implemented: `kill_process` (confirmed + audited), `select_file`,
  `export_settings` / `import_settings` (native dialogs; never include keys),
  `test_ai_model` (real Ollama check).
- Now return `not_implemented` (UI controls disabled with a "Not yet
  available" badge): `test_integration`, `save_memory`, `delete_memory`,
  `semantic_search`, `index_document`, `create_knowledge_base`,
  `export_knowledge`, `import_knowledge`, `optimize_vector_db`,
  `toggle_service`, `create_automation`, `toggle_automation`,
  `delete_automation`, `create_scheduled_task`, `toggle_scheduled_task`,
  `create_alert`, `toggle_alert`, `run_system_cleanup`, `optimize_system`,
  `/search`.
- The canned "AI response" shown when Ollama was unreachable was removed; the
  user now sees the real error. Metrics that were hardcoded (temperature 45 °C,
  disk 0 %) are reported as `null` / "n/a" until measured.

**Verified:** `cargo fmt --check`, `cargo clippy --all-targets --all-features -D warnings`,
`cargo test` (72 passed), `npm run check` (0 errors), `npm run build`, `cargo build`,
and a dev launch (clean startup, settings migration applied).

**Residual risk:** Windows elevation/classification paths are compiled only on
Windows and were not exercised here; audit-log tail truncation is not detectable
locally; DNS rebinding against `local_only` host checks.

### Phase 2: Hardening & hygiene (`fix/phase-2-hardening`)

**Correctness**
- Process/metric sorting uses `f32::total_cmp`; no `unwrap()`/`expect()`
  remains on runtime paths (only the setup-time `run().expect` and a static,
  tested regex).
- Real metrics via a `Monitor` that refreshes only what each call needs:
  disk usage (`sysinfo::Disks`, pseudo-filesystems and duplicate mounts
  excluded), network rx/tx bytes/s (`Networks` deltas, loopback excluded),
  hottest sensor temperature (`Components`), each `null` when unavailable.
  CPU sampling honours `MINIMUM_CPU_UPDATE_INTERVAL`. The hardcoded 45 °C is gone.

**Dependencies**
- Rust: `reqwest` 0.11 → 0.13 (rustls), `sysinfo` 0.30 → 0.39, `dirs` 5 → 7;
  `Cargo.lock` is now committed (removed from `.gitignore`).
- npm: removed unused `axios`; `marked` 11 → 18 (not yet used); updates within
  current majors; added `@types/node`; `cookie` pinned to `^0.7.2` via
  `overrides` (GHSA-pxg6-pf52-xh8x, transitive through SvelteKit).
- `npm audit`: 0 vulnerabilities. `cargo audit`: 0 vulnerabilities; 7
  informational warnings (unmaintained `unic-*`, `proc-macro-error`; unsound
  `glib::VariantStrIter`), all transitive through Tauri's GTK3 stack and not
  fixable in this repo.

**Desktop / config**
- Deleted `omnix-autostart.desktop` (hardcoded `/home/paul/...` path). Replaced
  by `tauri-plugin-autostart`, driven from Rust by `general.auto_start` on
  startup and on save.
- `.env.example` is developer-only (`RUST_LOG`, `TAURI_DEV_HOST`); the app
  never reads `.env`.
- No default Ollama model. New `list_ollama_models` command reads `/api/tags`,
  and Settings shows a dropdown of installed models.
- Setup scripts: Node 22+ required, `npm ci`, removed redundant
  `cargo install tauri-cli || true`.

**CI**
- `ci.yml` rewritten: no `|| echo` / `|| true` masks; `dtolnay/rust-toolchain`
  (replaces archived `actions-rs`), `Swatinem/rust-cache`, Node 24 LTS; jobs
  for svelte-check, Vitest, build, `npm audit`, `cargo fmt --check`,
  `cargo clippy -D warnings`, `cargo test`, `cargo audit`, TruffleHog secret
  scan, and a Linux/macOS/Windows compile matrix. Every third-party action is
  pinned to a commit SHA.
- New `release.yml`: tag-triggered `tauri-apps/tauri-action` draft releases
  with documented (commented-out) signing secrets.

**Tests**
- Vitest + @testing-library/svelte + jsdom; `npm test`. 9 tests: API/error
  helpers, write-only `SecretField`, KnowledgeView disabled states.
- Rust: 75 tests (adds metrics tests).

**Svelte**
- All timers are cleaned up on unmount (`+page` status poller, notification
  and avatar-emotion timeouts, Avatar animation timeouts/intervals,
  SettingsView banner); System Control polling skips ticks while a request is
  in flight. Every `invoke` goes through the typed `call<T>()` wrapper with
  types mirroring the Rust structs. Accessibility: all form labels are
  associated with their controls (svelte-check: 0 errors, 0 warnings).

**Docs**
- README: repository URLs and clone instructions fixed, Architecture section
  with a Mermaid diagram. New `docs/ARCHITECTURE.md` (module map, request
  sequence, decisions, testing, release signing).

**Residual risk:** transitive GTK3/glib advisories (upstream Tauri); the
three-OS build job and release workflow have not run yet (first run happens
on push).

### Phase 3: Intelligence layer (`feat/phase-3-intelligence`)

**Added**
- `ai::provider::LlmProvider` trait (`list_models`, `chat_stream`,
  `health_check`) with provider-neutral messages, tool calls and stream events.
- Providers:
  - **Ollama**: streaming `/api/chat` (NDJSON) with native tool calling;
    falls back to answering without tools (with a visible notice) when the
    model doesn't support tools.
  - **Anthropic**: raw-HTTP Messages API over SSE, `eager_input_streaming`
    tools, defensive JSON parsing, `refusal`/`max_tokens` handling, verbatim
    replay of thinking blocks, paginated `GET /v1/models`, no sampling params.
  - **OpenAI-compatible** (OpenAI, xAI, Gemini): SSE with fragmented
    `tool_calls`, `max_completion_tokens` for OpenAI, retry without
    `temperature` for models that reject it.
- Cloud providers are blocked in Rust while `local_only` is on; keys come only
  from the keychain. No model id is hardcoded anywhere: `list_models` /
  `list_ollama_models` discover them, and Settings shows dropdowns.
- Streaming chat: `chat_send(message, Channel<UiEvent>)`, `chat_cancel`,
  `chat_reset`; conversation kept in Rust state with context-window-aware
  truncation (honours `ai.context_window`, `ai.max_tokens`,
  `ai.temperature`).
- Agent loop (`ai::agent`): tools `run_command`, `read_file`,
  `list_directory`, `search_memory`. Every call goes through the Phase 1
  policy engine / file guard as `source: llm_tool` (Mutating/Privileged always
  natively confirmed), output is wrapped in escaped
  `<tool_result untrusted="true">` blocks, the system prompt forbids following
  instructions from tool output, one tool round per message unless
  `autonomous_mode` (then `max_autonomous_steps`).
- `test_ai_model` runs a real provider health check and model-availability
  check.
- Chat UI renders assistant Markdown via `marked` + DOMPurify (scripts,
  iframes, forms, media, styles, `on*` handlers removed; links neutralised),
  shows tool activity, and has Stop / Clear.
- Memory: `MemoryStore` trait + `KbCoreStore` REST adapter configured by
  `memory.backend_url` (empty = disabled, clear UI state) and an optional
  keychain token (`kb_core`). `save_memory`, `delete_memory`,
  `semantic_search`, `index_document` are now real. Contract documented in
  `docs/kb-core-contract.md` (**needs owner confirmation**).
- Removed remaining Chroma references.

**Tests:** Rust 100 unit tests (+1 opt-in live Ollama test, passing against
`granite-code:8b`), Vitest 14 (adds sanitizer tests).

**Residual risk:** end-to-end tool calling was verified with unit tests
(stream parsers, agent helpers), not a live tool-capable model; the kb-core
contract is assumed; cloud providers were not exercised against live APIs
(no keys configured on this machine).

### Phase 4: Integrations & desktop polish (`feat/phase-4-integrations`)

**Added**
- **MCP client** (official `rmcp` 3.4, client-only features): register stdio
  or streamable-HTTP servers in Settings → MCP Servers; tools are exposed to
  the agent as `mcp__<server>__<tool>`. Starting a stdio server is
  policy-classified, natively confirmed and audited; HTTP servers obey
  `local_only`; secrets (env vars, bearer tokens) live in the keychain as
  `mcp.<server>.<NAME>`; every tool call is Mutating (native dialog) unless
  marked read-only, time-limited and audited; outputs are untrusted data.
  Verified end-to-end against a stdio fixture server (`tests/fixtures`).
- Replaced the placeholder GitHub/Drive/Slack/Discord/Jira/Notion settings
  (and `test_integration`) with MCP server entries.
- **AIORC**: `aiorc` cargo feature with an `AiorcProvider` scaffold that
  returns `NotImplemented`. No `.proto` was provided, so no schema was invented.
- **Voice** (push-to-talk only): STT via a configurable faster-whisper
  server (OpenAI-compatible `/v1/audio/transcriptions`, `local_only`),
  raw-bytes IPC; TTS via Piper through the executor's hardened spawn (fixed
  argv, stdin, audited); mic button and Ctrl+Space are wired; "Read aloud" on
  replies. On Linux, WebKitGTK media streams are enabled and **only
  audio-only** permission requests are granted, only while voice is configured.
- **Desktop plugins**: `single-instance`, `window-state`, `global-shortcut`
  (Ctrl+Space PTT), tray icon (Show/Quit, close-to-tray honouring
  `minimize_to_tray`), `tauri-plugin-log` (rotating `omnix.log`, separate from
  the audit log; `tracing` bridged via its `log` feature). The updater is
  intentionally not registered; setup documented in `docs/ARCHITECTURE.md`.
- **Observability**: optional Loki shipping of redacted audit lines
  (`observability.loki_url`, off by default, bounded/async, `local_only`).
- Settings UI: MCP server editor, Voice tab, autonomous-mode and Loki
  controls. `SecretField` checks the keychain itself when `has` is omitted.
- CSP: `media-src 'self' blob:` for TTS playback.

**Tests:** Rust 108 (+1 opt-in live Ollama; +1 `aiorc` test under
`--all-features`), Vitest 16.

**Residual risk:** voice capture and Piper were not exercised end-to-end here
(no STT server or Piper installed); the global shortcut may be unavailable
where Ctrl+Space is bound by the input-method switcher (logged, non-fatal);
MCP servers run with user permissions once approved.

## [1.0.0] - 2026-06-03

### Added
- 🎉 Initial release of OMNIX
- 🤖 AI-powered desktop assistant with God Mode capabilities
- 🎨 Beautiful JARVIS-inspired UI with animated avatar
- 🔐 Sudo-level system access with secure privilege handling
- 💻 Full system command execution
- 📁 Comprehensive file operations
- 🔍 Advanced search capabilities
- 📊 Real-time system monitoring
- 🎤 Voice input support (foundation)
- 🔊 Text-to-speech output (foundation)
- 🧠 Persistent memory system (foundation)
- 🌐 Multi-modal interaction support
- ⚡ Lightning-fast Tauri 2.0 + SvelteKit architecture
- 🎨 Glassmorphic UI with cosmic theme
- 📝 Comprehensive documentation
- 🛠️ Cross-platform support (macOS, Linux, Windows)
- 🔧 Setup scripts for all platforms
- 📦 Production-ready build system

### Features
- Command execution with `/execute` prefix
- File operations with `/file` prefix
- Search functionality with `/search` prefix
- System monitoring with `/monitor` command
- Natural language processing for general queries
- Animated avatar with lip-sync capability
- Real-time system status display
- Command history tracking
- Multiple view modes (Home, Commands, History, Settings, Knowledge, System Control)
- Responsive glassmorphic design
- Particle animation background
- Smooth transitions and animations

### Technical
- Built with Tauri 2.0 for native performance
- SvelteKit 2.0 with Svelte 5 runes
- TailwindCSS for styling
- Rust backend for system integration
- TypeScript for type safety
- Cross-platform compatibility

### Documentation
- Comprehensive README with installation guide
- Contributing guidelines
- MIT License
- Setup scripts for automated installation
- Environment configuration template
- Architecture documentation

## Roadmap (as written for 1.0.0, status updated)

- ✅ Ollama integration (streaming, tools, runtime model discovery)
- ✅ Cloud LLM providers (Anthropic, OpenAI, Gemini, xAI via OpenAI-compatible APIs), opt-in outside local-only mode
- ✅ Voice recognition with Whisper (faster-whisper / Speaches)
- ✅ TTS with Piper
- ✅ Long-term memory via kb-core (PostgreSQL/pgvector)
- ✅ Agent loop (policy-gated tools, bounded autonomy)
- ✅ Plugin architecture: MCP client
- Notification system (tray icon exists; desktop notifications not yet)
- Proactive assistance
- Browser automation
- Code analysis and modification
- Document processing (PDF, DOCX)
- Screenshot and screen understanding
- Clipboard integration (deliberately not granted to the webview today)
- Mobile companion app
- Cloud sync
- Multi-agent collaboration

---

For more details, see the [README](README.md) and [GitHub Releases](https://github.com/paulmmoore3416/omnix-ai-desktop/releases).
