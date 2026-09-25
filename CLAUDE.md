# CLAUDE.md: OMNIX runbook for Claude Code

You are working in **omnix-ai-desktop**: a local-first AI desktop assistant (Tauri 2 + SvelteKit 5 + Rust). This file
tells you how to install, verify, run, fix, and change it. Read it fully before acting. Deeper references:

- `docs/SERVER_DEPLOYMENT.md`: install layouts, GPU layout, operations, troubleshooting table
- `docs/ARCHITECTURE.md`: module map, sequence diagrams · `docs/SECURITY.md`: threat model (**read before touching `security/`**)
- `docs/USER_GUIDE.md`: what the user sees · `docs/PROJECT_OVERVIEW.md`: capabilities · `CHANGELOG.md`: history

---

## 1. Mission on a fresh machine

Goal: OMNIX fully working. Chat answers from a local model, push-to-talk transcribes, read-aloud speaks, policy
approvals appear, and `./scripts/doctor.sh` exits 0.

Target hardware (the owner's server): i7 12-core, 48 GB RAM, two NVIDIA GPUs (8 GB + 6 GB), Ubuntu/Debian.

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
6. **Report**: what was installed, the doctor output, the GPU split (`nvidia-smi`, `ollama ps`), and anything left for
   the user.

### Things only the human can do: stop and ask

- **Installing or changing the NVIDIA driver** (`sudo ubuntu-drivers install`) and **rebooting**.
- Typing the **sudo password** if it isn't cached, and unlocking the **keyring**.
- Clicking **approval dialogs**, granting **microphone** permission, testing voice with a real mic.
- **GitHub authentication** (`gh auth login`). Never ask for, write, or echo a personal access token; if one appears
  in chat, don't reuse it and tell the user to revoke it.
- Opening firewall ports or anything that exposes 11434/8000 beyond the LAN/tailnet.
- Turning **off** local-only mode, enabling privileged (`sudo`) commands, or entering cloud API keys.

---

## 2. Where things live

| What | Path |
|---|---|
| Rust backend (trust boundary) | `src-tauri/src/`: `lib.rs` (builder), `state.rs`, `error.rs`, `settings.rs`, `commands/*` (thin IPC wrappers) |
| Security | `src-tauri/src/security/`: `policy.rs` (risk tiers), `confirm.rs` (native dialogs), `executor.rs`, `elevation.rs`, `files.rs`, `audit.rs` (hash chain), `secrets.rs` (keychain) |
| AI | `src-tauri/src/ai/`: `provider.rs` (trait), `ollama.rs`, `anthropic.rs`, `openai_compat.rs`, `agent.rs` (tool loop), `endpoint.rs` (local-only guard), `aiorc.rs` (scaffold) |
| Integrations | `mcp.rs`, `voice.rs` (Speaches STT client, Piper TTS), `memory/kb_core.rs`, `observability.rs` (Loki), `desktop.rs` (tray, shortcuts) |
| Frontend | `src/routes/+page.svelte` (main UI, push-to-talk state machine), `src/lib/components/*`, `src/lib/voice.ts` (WAV capture), `src/lib/avatar.ts` (avatar colour system) |
| Webview permissions | `src-tauri/capabilities/default.json`, CSP in `src-tauri/tauri.conf.json` |
| Setup / health | `scripts/bootstrap.sh`, `scripts/doctor.sh` (Linux) · `scripts/setup.sh`, `scripts/setup.ps1` (macOS/Windows dev) |
| CI | `.github/workflows/ci.yml`, `release.yml` |
| User settings | `~/.config/omnix/settings.json` (no secrets, mode 600) |
| App + audit logs | `~/.local/share/com.paulmmoore.omnix/logs/` |
| Piper venv + voices | `~/.local/share/omnix/` |
| Services | `ollama.service` (+ drop-in `/etc/systemd/system/ollama.service.d/omnix.conf`), Docker container `omnix-speaches` on :8000 |

---

## 3. Diagnosing problems

Start with `./scripts/doctor.sh`, then work down this list. Full table: `docs/SERVER_DEPLOYMENT.md` §8.

| Doctor says | Look at | Usual fix |
|---|---|---|
| ollama not reachable | `systemctl status ollama`, `journalctl -u ollama -n 100` | `sudo systemctl restart ollama`; CUDA errors mean a driver problem, so ask the user |
| chat model not installed / not answering | `ollama list`, `ollama ps` | `ollama pull <model>`; if it spills to CPU, use `qwen3:8b` |
| speaches not reachable | `docker ps -a`, `docker logs omnix-speaches` | Re-run bootstrap. GPU passthrough: `docker run --rm --gpus all ubuntu nvidia-smi` |
| STT model not downloaded | `curl -s localhost:8000/v1/models \| jq` | `curl -X POST localhost:8000/v1/models/<model-id>` |
| tts_voice not absolute / files missing | `jq .voice ~/.config/omnix/settings.json` | Re-run bootstrap (it fixes relative voice paths); both `.onnx` and `.onnx.json` must exist |
| settings.json invalid | the file | Restore the newest `settings.json.bak-*` next to it |
| no Secret Service | `gnome-keyring` running? | Only matters for cloud API keys |

App-level errors reach the UI as structured `AppError` values (`kind` field). The same events are in the app log and
`audit.jsonl`. To debug the app itself, run `npm run tauri dev` from a desktop session and watch the terminal.

---

## 4. Changing the code

### Quality gate: all must pass before any commit

```bash
cd src-tauri && cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all && cd ..
npm run check && npm test && npm run build
```

Also `cargo audit` / `npm audit` when dependencies change. Don't silence warnings with `#[allow]` unless a comment
justifies it. Before adding or upgrading a crate or npm package, check the current version (`cargo search`,
`npm view <pkg> version`) and its changelog.

### Security invariants: never break these

1. **The webview is untrusted.** Don't add Tauri plugin permissions to `capabilities/default.json` (no shell, fs,
   opener, dialog, clipboard, notification), don't loosen the CSP, and don't add a command that runs a shell string
   from the frontend. All execution goes through `request_execution` → `security/policy.rs` → native confirm →
   `executor.rs` → audit.
2. **Built-in `Denied` rules can't be overridden** from settings. Add a test in `policy.rs` for every new rule or bypass.
3. **Approval dialogs are native**, built from the parsed request, and default to deny.
4. **Secrets live only in the OS keychain.** No IPC command returns a secret. Never write keys to `settings.json`,
   logs, the repo, or chat.
5. **`local_only` defaults on** and is enforced in Rust (`ai/endpoint.rs`). New network endpoints must go through that guard.
6. **`enable_sudo` defaults off.** Elevation only via pkexec with a confirmation dialog; OMNIX never handles passwords.
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

- Services, automations, scheduler, alerts, cleanup: **planned**, and the UI controls are disabled on purpose.
- AIORC backend: scaffold only (`--features aiorc`), waiting on its `.proto`.
- Auto-update: off until release signing keys exist.
- Voice needs a Speaches/faster-whisper server; read-aloud needs Piper with an absolute voice path.
- No authentication on Ollama/Speaches: keep them on `127.0.0.1` or a firewalled LAN/tailnet.
