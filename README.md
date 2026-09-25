<div align="center">

# 🌌 OMNIX

### A local-first AI desktop assistant

[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2-orange.svg)](https://tauri.app)
[![Svelte](https://img.shields.io/badge/Svelte-5-red.svg)](https://svelte.dev)
[![Rust](https://img.shields.io/badge/Rust-1.80+-orange.svg)](https://www.rust-lang.org)

**OMNIX** is a desktop assistant built with Tauri 2, SvelteKit and Rust. It talks to a local LLM (Ollama) and can run commands on your machine, but only through a policy engine, native approval dialogs and a tamper-evident audit log.

![OMNIX Interface](uiexample.jpg)

</div>

---

## What works today

| Area | Status |
|------|--------|
| Chat with a local Ollama model | ✅ |
| `/execute` shell commands, risk-classified, with native approval for anything that changes the system | ✅ |
| `/file read`, `/file list`, `/file write` (writes need approval, credential files are off-limits) | ✅ |
| `/monitor` and the System Control view (CPU, memory, processes, ending a process) | ✅ |
| API keys stored in the OS keychain | ✅ |
| Hash-chained, redacted audit log with a **Verify** button | ✅ |
| Local-only mode (blocks cloud providers and non-local endpoints) | ✅ on by default |
| Settings export/import (never includes secrets) | ✅ |
| Cloud providers (OpenAI, Anthropic, Gemini, xAI) | 🚧 planned: keys can be stored; chat is not wired yet |
| Voice input/output (push-to-talk) | 🚧 planned |
| Persistent memory / knowledge base | 🚧 planned |
| Services, automations, scheduler, alerts, cleanup | 🚧 planned: controls are disabled in the UI |
| Integrations (GitHub, Drive, …) | 🚧 planned: will be MCP servers |

Anything marked planned is visibly disabled in the app and returns a `not_implemented` error from the backend. Nothing pretends to succeed.

---

## Quick start

### Prerequisites

- Node.js 22 LTS and npm
- Rust (stable) via [rustup](https://rustup.rs)
- Platform dependencies for Tauri: see the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
  - Linux: `libwebkit2gtk-4.1-dev build-essential curl wget file libssl-dev libayatana-appindicator3-dev librsvg2-dev`, a polkit agent (for `pkexec`), and a Secret Service provider (GNOME Keyring or KWallet) for key storage
- [Ollama](https://ollama.com) with at least one model pulled

```bash
git clone https://github.com/paulmmoore3416/omnix-ai-desktop.git
cd omnix-ai-desktop
npm install
npm run tauri dev        # development
npm run tauri build      # production bundle
```

### Configure

All configuration is done in the app under **Settings** and stored in:

- `settings.json` in your config directory (`~/.config/omnix/` on Linux). It contains **no secrets**.
- The **OS keychain** for API keys (write-only from the UI: "Key saved ✓ / Replace / Remove").

`.env` files are **not** read by the app. `.env.example` only documents developer tooling variables.

To use a local model:

```bash
ollama pull llama3.1        # or any model you prefer
```

Then open **Settings → AI Models**, click **Test Connection** to list installed models, type the model name, and save.

---

## Usage

```text
/execute git status                 # read-only: runs immediately
/execute npm install                # mutating: native approval dialog first
/file read ~/notes/todo.md
/file list ~/projects
/file write ~/notes/new.md Hello    # approval dialog
/monitor
```

Anything that doesn't start with `/` is sent to your Ollama model.

---

## 🔒 Security

OMNIX is designed on the assumption that the webview **and** the LLM may be compromised or manipulated. The Rust backend is the trust boundary. See [`docs/SECURITY.md`](docs/SECURITY.md) for the full threat model.

**Command execution**

- The frontend cannot run shell commands directly. The single entry point (`request_execution`, also used by `/execute`) runs every command through a policy engine that parses it with shell-quoting awareness and classifies it:

  | Tier | Examples | Behaviour |
  |------|----------|-----------|
  | **Read-only** | `ls`, `cat`, `git status`, `ps`, `df` | Runs (optionally confirmed) |
  | **Mutating** | writes, installs, `git commit`, service changes, anything using `;` `&&` `\|` `$(…)` redirects or globs, unknown programs | Native approval dialog |
  | **Privileged** | `sudo …`, `pkexec …` | Requires **Settings → Security → Allow privileged commands** (off by default) plus approval; elevation goes through the OS password prompt (pkexec / macOS admin prompt / UAC), and OMNIX never handles passwords |
  | **Denied** | `rm -rf /` (and quoting, path-prefix, variable and Unicode variants), `mkfs`, raw disk writes, fork bombs, `chmod -R 777 /`, `curl … \| sh`, reading `~/.ssh`, `/etc/shadow`, cloud credentials | Never runs; cannot be overridden |

- Approval dialogs are **native** (raised from Rust), show the exact command, working directory and risk tier, default to **deny**, and time out as deny.
- Child processes get a cleared environment (`PATH`, `HOME`, `LANG`, `TERM` only), a timeout (default 60 s), a 1 MiB output cap, and are killed with their whole process group on timeout.
- You can add your own blocked prefixes, or switch to allowlist mode.

**Audit log.** Every execution, file access, process kill, secret change and security-setting change is appended to `audit.jsonl` in the app log directory (`~/.local/share/com.paulmmoore.omnix/logs/` on Linux). Each line carries the SHA-256 of the previous line, so edits and deletions are detectable (**Settings → Security → Verify audit log**). API keys and bearer tokens are redacted before writing.

**Secrets.** Provider keys live only in the OS keychain. No IPC command returns a secret to the UI. Keys found in older plaintext `settings.json` files are migrated to the keychain automatically and removed from the file.

**Local-only mode (default: on).** Hard-blocks cloud AI providers and any endpoint that doesn't resolve to loopback, RFC 1918, link-local, CGNAT/Tailscale (`100.64.0.0/10`) or IPv6 ULA addresses. This is enforced in Rust, not the UI. **Keep it on for machines on healthcare networks**: it prevents prompts that may contain PHI from leaving the local network. Turning it off requires a native confirmation.

**Webview hardening.** Strict Content-Security-Policy (no remote origins; all network access goes through Rust). No shell, filesystem, opener, dialog, notification or clipboard plugin permissions are granted to the webview. Devtools only open in debug builds.

**No telemetry.** OMNIX sends nothing anywhere except to the model endpoint you configure.

---

## Technology

- **Frontend:** SvelteKit 5, TypeScript, Tailwind CSS
- **Backend:** Tauri 2, Rust (tokio, reqwest, sysinfo, keyring, tracing)
- **AI:** Ollama (local)

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

```bash
git clone https://github.com/<your-username>/omnix-ai-desktop.git
git checkout -b feature/my-change
```

## License

MIT, see [LICENSE](LICENSE).

## Support

- Bugs and feature requests: [GitHub Issues](https://github.com/paulmmoore3416/omnix-ai-desktop/issues)
- Security issues: see [`docs/SECURITY.md`](docs/SECURITY.md) (please report privately)
