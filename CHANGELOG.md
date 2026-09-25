# Changelog

All notable changes to OMNIX will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

## [Unreleased]

### Planned Features
- Full Ollama/LM Studio integration
- Cloud LLM fallback (OpenAI, Claude, Grok, Gemini)
- Complete voice recognition with Whisper
- Natural TTS with Piper/Coqui
- Vector database for long-term memory
- Autonomous agent loop
- Proactive assistance
- Browser automation
- Code analysis and modification
- Document processing (PDF, DOCX)
- Screenshot and screen understanding
- Clipboard integration
- Notification system
- Plugin architecture
- Mobile companion app
- Cloud sync
- Multi-agent collaboration

---

For more details, see the [README](README.md) and [GitHub Releases](https://github.com/Paulmmoore3416/omnix-ai-desktop/releases).