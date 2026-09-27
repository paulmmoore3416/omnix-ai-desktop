# CLAUDE.md: OMNIX runbook for Claude Code

You are working in **omnix-ai-desktop**: a local-first AI desktop assistant (Tauri 2 + SvelteKit 5 + Rust). This file
tells you how to install, verify, run, fix, and change it. Read it fully before acting. Deeper references:

- `docs/SERVER_DEPLOYMENT.md`: install layouts, GPU layout, operations, troubleshooting table
- `docs/ARCHITECTURE.md`: module map, sequence diagrams · `docs/SECURITY.md`: threat model (**read before touching `security/`**)
- `docs/REMOTE_ACCESS.md`: proposed phone access design (not built; review it before adding any network listener)
- `docs/USER_GUIDE.md`: what the user sees · `docs/PROJECT_OVERVIEW.md`: capabilities · `CHANGELOG.md`: history

---

## 1. Mission on a fresh machine

Goal: OMNIX fully working. Chat answers from a local model, push-to-talk transcribes, read-aloud speaks, policy
approvals appear, and `./scripts/doctor.sh` exits 0.

Target hardware (the owner's server): i7 12-core, 48 GB RAM, an AMD RX 580 8 GB (runs Ollama
via Vulkan) and an NVIDIA GTX 1060 6 GB (runs Speaches via CUDA), Ubuntu/Debian.

### Procedure

1. **Check the machine first.** Don't change anything yet.
   ```bash
   uname -a; lsb_release -ds; nvidia-smi; echo $XDG_SESSION_TYPE; df -h ~; free -g
   ./scripts/doctor.sh || true
   ```
2. **Pick the layout** (`docs/SERVER_DEPLOYMENT.md` §1):
   - Desktop session present (`$XDG_SESSION_TYPE` is `x11`/`wayland`): **all-in-one**, `./scripts/bootstrap.sh --yes`
   - Headless: **services only**, `./scripts/bootstrap.sh --services-only --lan --yes`. The app runs on another machine.
   - If unsure, ask the user.
3. **Run bootstrap** and wait. It takes 20–40 minutes on the first run. Run it in the background and follow
   `logs/bootstrap-*.log`. It is idempotent: after fixing a problem, re-run the whole script, don't cherry-pick steps.
4. **Verify**: `./scripts/doctor.sh` must print `0 failed`. Warnings are acceptable only if you can explain each one
   (e.g. "omnix not on PATH" in services-only mode).
5. **Hand over the GUI checks** to the user. You cannot click native dialogs: Settings → AI Models → Test;
   Settings → Voice → Test microphone; `/execute touch /tmp/x` shows an approval dialog; Settings → Security →
   Verify audit log.
6. **Report**: what was installed, the doctor output, the GPU split (`ollama ps`, `nvidia-smi`; the AMD card only shows in `journalctl -u ollama`), and anything left for
   the user.

### Things only the human can do: stop and ask

- **Installing or changing the NVIDIA driver** (`sudo ubuntu-drivers install`) and **rebooting**.
- Typing the **sudo password** if it isn't cached, and unlocking the **keyring**.
- Clicking **approval dialogs**, granting **microphone** permission, testing voice with a real mic.
- **GitHub authentication** (`gh auth login`). Never ask for, write, or echo a personal access token; if one appears
  in chat, don't reuse it and tell the user to revoke it.
- Opening firewall ports or anything that exposes 11434/8000 beyond the LAN/tailnet.
- Turning **off** local-only mode, enabling privileged (`sudo`) commands, or entering cloud API keys.
- Creating a **Twilio** account, buying a number, entering its auth token, and enabling Settings → Phone.

---

## 2. Where things live

| What | Path |
|---|---|
| Rust backend (trust boundary) | `src-tauri/src/`: `lib.rs` (builder), `state.rs`, `error.rs`, `settings.rs`, `commands/*` (thin IPC wrappers) |
| Security | `src-tauri/src/security/`: `policy.rs` (risk tiers), `confirm.rs` (native dialogs), `executor.rs`, `elevation.rs`, `files.rs`, `audit.rs` (hash chain), `secrets.rs` (keychain) |
| AI | `src-tauri/src/ai/`: `provider.rs` (trait), `ollama.rs`, `anthropic.rs`, `openai_compat.rs`, `agent.rs` (tool loop), `endpoint.rs` (local-only guard), `aiorc.rs` (scaffold) |
| Phone | `src-tauri/src/phone.rs` (Twilio texts/calls, outbound only; token in keychain as `twilio`) |
| Google | `src-tauri/src/google.rs` (Gmail/Drive via OAuth + PKCE loopback, Developer Knowledge API; off by default, blocked by `local_only`; refresh token `internal.google_refresh`) |
| Licensing / usage | `src-tauri/src/license.rs` (offline Ed25519 keys, informational tiers; issuer `scripts/omnix-license.py`, signing key in the owner's keyring) · `src-tauri/src/usage.rs` (content-free daily ledger) |
| Deliverables | `src-tauri/src/workbook.rs` (`.xlsx` from a JSON spec for the `create_workbook` tool); writes go through `security/files.rs::write_bytes` |
| Integrations | `mcp.rs`, `voice.rs` (Speaches STT client, Piper TTS), `memory/kb_core.rs`, `observability.rs` (Loki), `desktop.rs` (tray, shortcuts), `ai/ollama_admin.rs` (model manager), `ai/metrics.rs` (agent/model metrics) |
| Host control | `src-tauri/src/system/`: `metrics.rs`, `gpu.rs` (NVIDIA + AMD), `history.rs`, `probe.rs` (fixed-argv reads), `services.rs`, `docker.rs`, `cleanup.rs`, `advisor.rs`, `snapshot.rs` · `src-tauri/src/ops/`: alerts/automations/scheduler (`engine.rs`, `rules.rs`, `cron.rs`, `approval.rs` HMAC) |
| Frontend | `src/routes/+page.svelte` (main UI, push-to-talk state machine), `src/lib/components/*`, `src/lib/voice.ts` (WAV capture), `src/lib/avatar.ts` (avatar colour system) |
| Webview permissions | `src-tauri/capabilities/default.json`, CSP in `src-tauri/tauri.conf.json` |
| Setup / health | `scripts/bootstrap.sh`, `scripts/doctor.sh` (Linux) · `scripts/setup.sh`, `scripts/setup.ps1` (macOS/Windows dev) |
| CI | `.github/workflows/ci.yml`, `release.yml` |
| User settings | `~/.config/omnix/settings.json` (no secrets, mode 600) · rules in `~/.config/omnix/ops.json` (mode 600, protected from commands) |
| App + audit logs | `~/.local/share/com.paulmmoore.omnix/logs/` |
| Piper venv + voices | `~/.local/share/omnix/` |
| Memory service (kb-core) | source `kb-core/` (see its README) · installed to `~/.local/share/omnix/kb-core/` (app, venv, `memory.db`) · config `~/.config/omnix/kb-core.env` · CLI `kb-core` |
| Docker (home server) | `deploy/docker-compose.yml` (+ `docker-compose.nvidia.yml`, `init.sh`), `kb-core/Dockerfile` · connectors: `kb-core/kb_core/connectors.py`, `docs/CONNECTORS.md` |
| Services | `ollama.service` (+ drop-in `/etc/systemd/system/ollama.service.d/omnix.conf`), Docker container `omnix-speaches` on :8000, user unit `omnix-kb-core` on :8100 |

---

## 3. Diagnosing problems

Start with `./scripts/doctor.sh`, then work down this list. Full table: `docs/SERVER_DEPLOYMENT.md` §8.

| Doctor says | Look at | Usual fix |
|---|---|---|
| ollama not reachable | `systemctl status ollama`, `journalctl -u ollama -n 100` | `sudo systemctl restart ollama`; CUDA errors mean a driver problem, so ask the user |
| chat model not installed / not answering | `ollama list`, `ollama ps` | `ollama pull <model>`; if it spills to CPU, use `qwen3:8b` |
| chat model only N% on GPU / AMD GPU warnings | `ollama ps`, `journalctl -u ollama \| grep "using device"` | Smaller model; AMD needs the `amdgpu` kernel driver and `mesa-vulkan-drivers` |
| speaches not reachable | `docker ps -a`, `docker logs omnix-speaches` | Re-run bootstrap. GPU passthrough: `docker run --rm --gpus all ubuntu nvidia-smi` |
| STT model not downloaded | `curl -s localhost:8000/v1/models \| jq` | `curl -X POST localhost:8000/v1/models/<model-id>` |
| tts_voice not absolute / files missing | `jq .voice ~/.config/omnix/settings.json` | Re-run bootstrap (it fixes relative voice paths); both `.onnx` and `.onnx.json` must exist |
| settings.json invalid | the file | Restore the newest `settings.json.bak-*` next to it |
| no Secret Service | `gnome-keyring` running? | Only matters for cloud API keys |
| kb-core not reachable / degraded | `kb-core status`, `journalctl --user -u omnix-kb-core -n 50` | `./scripts/setup-memory.sh` (idempotent); degraded = embedding model unreachable, so check `nomic-embed-text` in `ollama list` |

App-level errors reach the UI as structured `AppError` values (`kind` field). The same events are in the app log and
`audit.jsonl`. To debug the app itself, run `npm run tauri dev` from a desktop session and watch the terminal.

---

## 4. Changing the code

### Quality gate: all must pass before any commit

```bash
cd src-tauri && cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all && cd ..
npm run check && npm test && npm run build
(cd kb-core && python3 -m unittest discover -s tests -t .)
```

Also `cargo audit` / `npm audit` when dependencies change. Don't silence warnings with `#[allow]` unless a comment
justifies it. Before adding or upgrading a crate or npm package, check the current version (`cargo search`,
`npm view <pkg> version`) and its changelog.

### Security invariants: never break these

1. **The webview is untrusted.** Don't add Tauri plugin permissions to `capabilities/default.json` (no shell, fs,
   opener, dialog, clipboard, notification; the dialog and notification plugins are driven from Rust only), don't
   loosen the CSP, and don't add a command that runs a shell string from the frontend. All execution goes through
   `security/policy.rs` → native confirm → `executor.rs` → audit. Unattended runs (scheduler/automations) use
   `executor::execute_preapproved` only after verifying the rule's HMAC approval (`ops/approval.rs`); never add
   another path that skips confirmation. Host probes (`system/probe.rs`) take hardcoded argv only.
2. **Built-in `Denied` rules can't be overridden** from settings. Add a test in `policy.rs` for every new rule or bypass.
3. **Approval dialogs are native**, built from the parsed request, and default to deny.
4. **Secrets live only in the OS keychain.** No IPC command returns a secret. Never write keys to `settings.json`,
   logs, the repo, or chat.
5. **`local_only` defaults on** and is enforced in Rust (`ai/endpoint.rs`). New network endpoints must go through that guard.
   The only exception is the user-enabled phone (`ensure_phone_allowed`, fixed Twilio host, fixed recipient); don't add others.
6. **`enable_sudo` defaults off.** Elevation only via pkexec (or, for service control, systemd's own polkit prompt)
   after an OMNIX confirmation dialog; never unattended; OMNIX never handles passwords.
7. **Model output is data.** Tool results are wrapped as untrusted; rendered Markdown goes through DOMPurify (`src/lib/markdown.ts`).
8. **No fake success.** Unbuilt features return `AppError::NotImplemented` and their UI controls are disabled.
9. **No hardcoded model ids in app code.** Models are discovered from the provider at runtime. (The bootstrap
   defaults in `scripts/` are only install-time suggestions.)
10. **The audit log is append-only and hash-chained.** Don't rewrite it; every new sensitive action should be audited.

### Conventions

- Rust: `thiserror` errors, doc comments on public items, comments explaining every security decision, `tracing` for logs.
- Frontend: Svelte 5 runes, TypeScript, Tailwind. Avatar colours only in `src/lib/avatar.ts`. If you add a mood or
  condition, update `AvatarKey.svelte` and the key table in `docs/USER_GUIDE.md` §7.
- Conventional commits (`fix(security): …`, `feat(voice): …`, `docs: …`). Add a `CHANGELOG.md` entry under `[Unreleased]`.
- When behaviour changes, update the docs in the same commit: README status table, `USER_GUIDE.md`, and
  `SERVER_DEPLOYMENT.md` if setup changes.

---

## 5. Known limitations (don't "fix" these by faking them)

- Memory encryption at rest is opt-in (`setup-memory.sh --encrypt`) and
  needs an unlocked keyring for kb-core to start.
- Privileged (sudo) commands never run unattended by design; approve them interactively.
- AIORC backend: scaffold only (`--features aiorc`), waiting on its `.proto`.
- Auto-update: off until release signing keys exist.
- Voice needs a Speaches/faster-whisper server; read-aloud needs Piper with an absolute voice path.
- No authentication on Ollama/Speaches: keep them on `127.0.0.1` or a firewalled LAN/tailnet.
