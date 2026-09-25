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