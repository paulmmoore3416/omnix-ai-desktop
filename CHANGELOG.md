# Changelog

All notable changes to OMNIX will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
