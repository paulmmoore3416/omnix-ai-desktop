# OMNIX: Project Showcase

OMNIX is a local-first AI desktop assistant (Tauri 2, Rust, SvelteKit 5) that
lets a language model inspect and operate a computer, **safely**. The
interesting engineering is the trust boundary: the webview and the model are
both treated as untrusted, and every privileged action passes through a Rust
policy engine, a native approval dialog and a tamper-evident audit log.

Everything below describes what is implemented and tested in this repository.

---

## Highlights

- **Command policy engine** (Rust): quote-aware shell lexer + `shlex` argv
  parsing that classifies commands into ReadOnly / Mutating / Privileged /
  Denied. It recursively unwraps `sudo`, `env`, `sh -c`, `eval`, `xargs`,
  `find -exec` and command substitution, and blocks bypasses such as quoting
  tricks, `/bin/rm`, variable expansion, chained commands and Unicode
  look-alikes. Covered by 32 unit tests.
- **Native (Rust-side) confirmation dialogs**: default-deny, time out as deny,
  serialized; they also guard security-weakening settings changes.
- **Hardened execution**: `tokio::process`, cleared environment, timeouts with
  process-group kill, output caps, and elevation via pkexec / macOS admin
  prompt / UAC (off by default).
- **Hash-chained JSONL audit log** with secret redaction and a verifier;
  optional shipping to Grafana Loki.
- **Secrets in the OS keychain** with automatic migration from legacy
  plaintext settings and no IPC path that returns a secret.
- **Provider-neutral LLM layer**: streaming Ollama (NDJSON), Anthropic
  Messages API (SSE, raw HTTP), and OpenAI-compatible providers, all with tool
  calling and runtime model discovery (no hardcoded model ids).
- **Custom agent loop** (no framework): every tool call goes through the same
  policy engine as the user; tool output is wrapped as untrusted data
  (prompt-injection mitigation); bounded autonomy.
- **MCP client** (official `rmcp` SDK) with policy-gated, audited tool calls.
- **kb-core long-term memory engine** (Python, SQLite FTS5 + exact vectors): automatic recall before every
  reply with a **calibrated 0–1 relevance** (lift over the query's background similarity), personalised
  first-person → third-person query variants, hybrid keyword + semantic search with MMR diversity, memory
  activation (importance, recency, use, reinforcement), **contradiction-aware consolidation** (the local LLM
  judges duplicate vs. obsolete; supersession is reversible), opt-in fact capture from conversations, live
  incremental folder sync, and graceful degradation when the embedding model is down.
- **Local-only mode** that blocks cloud providers and non-private endpoints in
  Rust, built for machines on healthcare networks (no PHI egress).
- **Voice**: push-to-talk speech-to-text (faster-whisper) and Piper TTS.
  Audio is captured with Web Audio and encoded in-app to 16 kHz WAV (no
  dependency on webview codecs), with a live level meter, hold-or-tap
  controls, Esc to cancel, and a built-in microphone self-test.
- **JARVIS-style holographic avatar** (SVG): 13 mood colours, 4 alert
  conditions and 4 signal ripples wired to real backend state, tool-call
  satellites, voice-reactive waveform, flick-to-spin rings, and an in-app
  colour key.
- **System Control and unattended automation**: telemetry for every GPU
  (NVIDIA via `nvidia-smi`, AMD via sysfs), 1-hour metric history, systemd
  services and Docker containers, a model manager, alerts, automations and
  cron schedules. Unattended commands need one native approval, stored as an
  HMAC-SHA256 over rule id, command and cwd with a keychain key the webview
  can't reach, and are re-classified by the policy engine at every run.
- **One-command deployment**: `scripts/bootstrap.sh` provisions a fresh
  Ubuntu server (Ollama + GPU-sized models, Speaches STT on CUDA, Piper TTS,
  seeded settings, `.deb` install); `scripts/doctor.sh` verifies it.

## Stack

| Layer | Technology |
|-------|-----------|
| Frontend | SvelteKit 5 (runes), TypeScript, Tailwind CSS, `marked` + DOMPurify |
| Backend | Rust, Tauri 2, tokio, reqwest (rustls), sysinfo, keyring, tracing, rmcp |
| AI | Ollama, Anthropic Messages API, OpenAI-compatible APIs |
| Memory | kb-core (Python stdlib + optional numpy, SQLite FTS5, Ollama `nomic-embed-text`) via a documented REST contract |
| Quality | cargo clippy `-D warnings`, cargo test, cargo audit, svelte-check, Vitest, npm audit |
| CI/CD | GitHub Actions (SHA-pinned actions, 3-OS build matrix, secret scan), tag-triggered draft releases |

## Architecture

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the module map and
sequence diagrams, [`docs/TECHNICAL_REFERENCE.md`](docs/TECHNICAL_REFERENCE.md)
for every command, setting and algorithm, and [`docs/SECURITY.md`](docs/SECURITY.md)
for the threat model.

## Engineering decisions worth discussing

1. **The webview is not trusted.** Shell, filesystem and opener plugins were
   removed; the strict CSP forbids remote origins; the frontend only calls
   explicit Rust commands.
2. **Confirmation must be native.** JavaScript `confirm()` can be skipped by
   the code it is supposed to guard, so approvals are raised from Rust with
   text built from the parsed request.
3. **Fail closed.** Unparseable commands, invisible Unicode, dynamic
   executable names and excessive nesting are denied rather than guessed at.
4. **Honest stubs.** Unbuilt features return `not_implemented` and their
   controls are disabled, instead of reporting success.
5. **Model output is data.** Tool results are delimited and escaped, and the
   chat renderer sanitizes Markdown and neutralizes links.
6. **Measure before you threshold.** Calibrating kb-core on real embeddings
   showed "prefers morning meetings" vs "prefers afternoon meetings" at cosine
   0.953, higher than a true paraphrase (0.951). Vector-only de-duplication
   would silently lose changed facts, so ambiguous pairs go to an LLM judge.

## Test & quality snapshot

- Rust: 148 unit tests, including policy bypass attempts, audit-chain
  tampering, secret migration, stream parsers (Ollama/Anthropic/OpenAI), an
  MCP stdio end-to-end test, executor timeouts/env clearing and memory
  recall-block escaping, HMAC approvals and settings validation. Plus opt-in
  live tests (Ollama, host probes).
- Frontend: 29 Vitest tests (API/error helpers, write-only secret field,
  disabled-state UI, Knowledge view, markdown sanitizer, voice helpers, WAV encoder).
- kb-core: 61 offline tests (consolidation, supersession, outage/backfill,
  model change, personalisation, incremental re-index, HTTP guards), run in
  CI with and without numpy.
- `cargo clippy -D warnings`, `svelte-check` with 0 errors and 0 warnings,
  `npm audit` clean, `cargo audit` with 0 vulnerabilities.

## Resume bullets (accurate as of this version)

- Designed and built a **Rust command-policy engine** for an AI desktop
  assistant that classifies shell commands into four risk tiers with a
  quote-aware lexer, blocking recursive root deletes, disk wipes,
  download-and-execute and credential reads, validated by 32 bypass-focused
  unit tests.
- Implemented a **tamper-evident, hash-chained audit log** with secret
  redaction and verification, plus optional Grafana Loki shipping.
- Built a **provider-neutral streaming LLM layer** (Ollama, Anthropic,
  OpenAI-compatible) and a **custom tool-using agent loop** where every model
  action passes the same policy engine and native approval flow as the user.
- Integrated the **Model Context Protocol** (official Rust SDK) with
  policy-gated, audited tool calls and keychain-stored credentials.
- Added a **local-only mode** that enforces private-network egress in Rust to
  prevent PHI leaving healthcare networks.
- Hardened CI with **SHA-pinned GitHub Actions**, clippy `-D warnings`,
  dependency audits and a three-OS build matrix.
- Built **kb-core**, a local long-term memory engine with calibrated hybrid
  retrieval, contradiction-aware consolidation via an LLM judge, memory
  activation modelling and incremental folder sync; lifted first-person recall
  of third-person memories from 0.33 to 0.62 relevance with query
  personalisation.

## Roadmap

- AIORC gRPC routing backend (scaffolded; awaiting its `.proto`)
- Signed releases and auto-update
- Multi-host view (several OMNIX servers in one System Control)

## Links

- Repository: https://github.com/paulmmoore3416/omnix-ai-desktop
- Contact: paulmmoore3416@gmail.com
