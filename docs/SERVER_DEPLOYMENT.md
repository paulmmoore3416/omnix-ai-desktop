# OMNIX Server Deployment

How to put OMNIX on a dedicated Linux machine with NVIDIA GPUs, using `scripts/bootstrap.sh`. Written for the
reference box below; any Ubuntu/Debian machine with an NVIDIA GPU works the same way.

| Reference hardware | |
|---|---|
| CPU | Intel Core i7, 12 cores |
| RAM | 48 GB |
| GPU 0 / GPU 1 | 8 GB + 6 GB NVIDIA |
| OS | Ubuntu 24.04 LTS or newer (Debian 12+ works) |

---

## 1. Pick a topology

OMNIX is a **desktop app** (Tauri/WebKitGTK window, native approval dialogs, OS keychain). The heavy parts, the LLM
and speech-to-text, are plain network services. That allows two layouts:

| Layout | When | Command |
|---|---|---|
| **A. All-in-one**: app + services on the server, used at its own screen (or over a remote-desktop session) | The server has a desktop session | `./scripts/bootstrap.sh` |
| **B. GPU server + thin clients**: services on the server, the OMNIX app on your laptop/desktop | The server is headless | Server: `./scripts/bootstrap.sh --services-only --lan` · Each client: `./scripts/bootstrap.sh --no-services`, then point Settings at the server |

Layout B keeps working in **local-only mode** as long as the server address is private: RFC 1918 (`192.168.x.x`,
`10.x.x.x`), Tailscale (`100.64.0.0/10`), or IPv6 ULA. Public addresses are refused by design.

> The approval dialogs and keychain need a real graphical session with a polkit agent and GNOME Keyring/KWallet.
> Don't run the app under Xvfb or plain SSH. Use layout B, or a remote-desktop session (GNOME Remote Desktop, RDP, VNC).

---

## 2. Before you run it

1. **Ubuntu/Debian with a desktop** (layout A) or server edition (layout B, `--services-only`).
2. **NVIDIA driver installed and working.** This is the only step the script won't do, because a driver install needs a
   reboot and can break a running desktop:
   ```bash
   sudo ubuntu-drivers install && sudo reboot
   nvidia-smi          # must list both GPUs
   ```
3. A normal user with `sudo`. Don't run the script as root.
4. **GitHub access** (the repository is private): `sudo apt install gh && gh auth login`, or an SSH key on your account.
5. About **40 GB free disk** (models ≈ 15 GB, Docker image ≈ 8 GB, Rust build cache ≈ 5 GB).

---

## 3. Install

```bash
gh repo clone paulmmoore3416/omnix-ai-desktop     # or: git clone git@github.com:paulmmoore3416/omnix-ai-desktop.git
cd omnix-ai-desktop
./scripts/bootstrap.sh            # add --yes for unattended, --lan to serve other machines
```

The first run takes 20–40 minutes, most of it model downloads and the Rust release build. Output is also written to
`logs/bootstrap-<timestamp>.log`.

| Step | What happens | Idempotent how |
|---|---|---|
| System packages | WebKitGTK 4.1 + Tauri build deps, `libxdo-dev`, ALSA utils, `jq`, Python venv | Installs only missing packages |
| GPU detection | Reads `nvidia-smi`; picks the **smallest** GPU for speech-to-text | Stops with instructions if no driver (unless `--cpu`) |
| Rust + Node | rustup stable (≥ 1.95), Node.js 24 from NodeSource | Skips if already new enough |
| Dependencies | `npm ci`, `cargo fetch --locked` | Lockfiles are committed |
| Ollama | Official installer, systemd drop-in `/etc/systemd/system/ollama.service.d/omnix.conf` (bind address, flash attention, 30 min keep-alive); pulls `qwen3:8b` + `qwen3:14b` | Skips installed models |
| Docker + NVIDIA toolkit | `docker.io`, `nvidia-container-toolkit`, runtime configured; adds you to the `docker` group | Skips if present |
| Speaches (STT) | Container `omnix-speaches` (`latest-cuda`), port 8000, pinned to the smallest GPU, model `Systran/faster-distil-whisper-large-v3` downloaded and kept loaded | Container is recreated each run so flags match |
| Piper (TTS) | venv at `~/.local/share/omnix/piper-venv`, voice `en_US-lessac-medium` in `~/.local/share/omnix/voices/`, `~/.local/bin/piper` symlink, synth smoke test | Skips if present |
| Settings | Fills empty values in `~/.config/omnix/settings.json` (model, STT URL/model, Piper path, **absolute** voice path); keeps your choices; timestamped backup; mode 600 | Merge, never overwrite |
| App | `npm run tauri build -- --bundles deb`, then `apt install` of the `.deb` → `omnix` command + app launcher | Reinstalls the new build |
| Health check | `scripts/doctor.sh` | Read-only |

### Options

| Flag | Effect |
|---|---|
| `--services-only` | Ollama, Speaches, Piper and settings only; no app build |
| `--no-services` | App only (use on thin clients in layout B) |
| `--lan` | Bind Ollama (11434) and Speaches (8000) to `0.0.0.0` instead of `127.0.0.1` |
| `--cpu` | Allow running without an NVIDIA GPU (CPU Speaches image) |
| `--yes` | Non-interactive apt (automatic when there is no TTY) |

### Tunables (environment variables)

| Variable | Default | Notes |
|---|---|---|
| `OMNIX_CHAT_MODEL` | `qwen3:8b` | Default chat model (set only if settings have none) |
| `OMNIX_EXTRA_MODELS` | `qwen3:14b` | Space-separated; `""` to skip |
| `OMNIX_STT_MODEL` | `Systran/faster-distil-whisper-large-v3` | Any faster-whisper model id Speaches can download |
| `OMNIX_STT_GPU` | smallest GPU index | Force Speaches onto a specific GPU |
| `OMNIX_PIPER_VOICE` | `en_US-lessac-medium` | Any Piper voice id |
| `OMNIX_DATA_DIR` | `~/.local/share/omnix` | Piper venv and voices |

Example: `OMNIX_EXTRA_MODELS="qwen3:14b qwen2.5-coder:7b" ./scripts/bootstrap.sh --yes`

---

## 4. GPU and memory layout (8 GB + 6 GB)

| Workload | Where | VRAM (approx.) |
|---|---|---|
| Speaches, `faster-distil-whisper-large-v3` (fp16) | 6 GB GPU (pinned with `--gpus device=N`) | ~1.5–2 GB, kept resident |
| `qwen3:8b` (Q4_K_M, default) | Fits entirely on the 8 GB GPU | ~5–6 GB with 4–8k context |
| `qwen3:14b` (Q4_K_M) | Ollama splits it across both GPUs | ~9–10 GB total |

- Ollama sees both GPUs and places layers itself; flash attention (`OLLAMA_FLASH_ATTENTION=1`) reduces KV-cache
  memory. If a large model spills to CPU (`ollama ps` shows a CPU %), use the 8B model or a smaller context.
- The 48 GB of system RAM leaves plenty of headroom for CPU offload, the Rust build and the app.
- Want the whole 8 GB card for the LLM only? Add `Environment="CUDA_VISIBLE_DEVICES=<8GB index>"` to the Ollama
  drop-in. Then 14B models no longer fit on the GPU.

Check live usage with `nvidia-smi` and `ollama ps`.

---

## 5. Verify

```bash
./scripts/doctor.sh
```

`doctor.sh` is read-only and exits non-zero on any failure. It checks the toolchains, the `omnix` binary, GPUs,
`settings.json` (valid JSON, mode 600), the Ollama version, that the selected model is installed **and answers a
prompt**, Speaches health and that the STT model is downloaded, the Piper binary and voice files, the
`omnix-speaches` container, and the Secret Service. Endpoints are read from your settings, so on a layout B client it
checks the remote server.

Then in the app:

1. **Settings → AI Models**: the model list is filled; **Test** answers.
2. **Settings → Voice → Test microphone**: say a sentence; expect `✓ Heard: "…"`.
3. Ask something in chat, then click **🔊 Read aloud** under the reply.
4. `/execute git status` runs immediately; `/execute touch /tmp/x` opens a native approval dialog.
5. **Settings → Security → Verify audit log** reports the chain is intact.

---

## 6. Layout B: connecting clients to the server

On the server: `./scripts/bootstrap.sh --services-only --lan`, then firewall the ports to your LAN or tailnet:

```bash
sudo ufw allow from 192.168.1.0/24 to any port 11434,8000 proto tcp
sudo ufw allow in on tailscale0 to any port 11434,8000 proto tcp   # if using Tailscale
sudo ufw enable
```

Neither Ollama nor Speaches has authentication. **Never forward these ports to the internet.**

On each client: `./scripts/bootstrap.sh --no-services`, then in **Settings**:

- **AI Models → Ollama host:** `http://<server-ip>:11434`
- **Voice → Speech-to-text server URL:** `http://<server-ip>:8000`, **Whisper model:** `Systran/faster-distil-whisper-large-v3`
- **Voice → Piper**: Piper runs on the client. Either run `./scripts/bootstrap.sh --services-only` on the client too
  (installs Piper; also a local Ollama/Speaches, which you can ignore or stop), or install Piper by hand as in the
  [user guide](USER_GUIDE.md#setting-up-voice-one-time).

---

## 7. Operate

| Task | Command |
|---|---|
| Update OMNIX | `git pull && ./scripts/bootstrap.sh --yes` (keeps your settings) |
| Add a model | `ollama pull <model>`, then pick it in Settings |
| Restart services | `sudo systemctl restart ollama` · `docker restart omnix-speaches` |
| Service logs | `journalctl -u ollama -f` · `docker logs -f omnix-speaches` |
| Bootstrap logs | `logs/bootstrap-*.log` in the repo |
| App logs + audit log | `~/.local/share/com.paulmmoore.omnix/logs/` (`audit.jsonl` is hash-chained) |
| Settings | `~/.config/omnix/settings.json` (no secrets; keys are in the OS keychain) |
| Launch at login | Settings → General |

### Long-term memory (kb-core)

`bootstrap.sh` installs it (skip with `--no-memory`). It gives the assistant automatic recall, the `search_memory` and
`remember` tools, optional fact capture, and the **Knowledge** view. Details: [`kb-core/README.md`](../kb-core/README.md).

```bash
./scripts/setup-memory.sh                        # install/update the service and point OMNIX at it
./scripts/setup-memory.sh ~/notes ~/some-kb-repo # … and keep these folders indexed live
./scripts/setup-memory.sh --lan                  # headless server: serve other machines (bearer token)
```

The script pulls `nomic-embed-text` into Ollama and installs the code with a private venv under
`~/.local/share/omnix/kb-core/`. It writes `~/.config/omnix/kb-core.env` (kept on re-runs; new folders are
appended) and starts the systemd **user** service `omnix-kb-core` on `127.0.0.1:8100`. It uses OMNIX's chat model
for fact capture and the duplicate/contradiction judge, and your account's first name for personalised retrieval.
It migrates memories from kb-core 1.x once, sets `memory.backend_url`, and installs a `kb-core` CLI in
`~/.local/bin`. Restart OMNIX afterwards if it was running. Data is one SQLite file,
`~/.local/share/omnix/kb-core/memory.db` (mode 600).

| Task | Command |
|---|---|
| Status, counts, models, watched folders | `kb-core status` |
| Logs | `journalctl --user -u omnix-kb-core -f` |
| See what the assistant would recall | `kb-core search "your question"` |
| Settings (models, folders, name) | edit `~/.config/omnix/kb-core.env`, then `systemctl --user restart omnix-kb-core` |
| Back up / restore | `kb-core export ~/memory.jsonl` · `kb-core import ~/memory.jsonl` (or Knowledge → Export/Import) |
| Tidy up (merge duplicates, compact) | `kb-core maintenance` (or Knowledge → Optimize) |
| Run without being logged in (headless) | `sudo loginctl enable-linger $USER` |

**GPU/VRAM:** `nomic-embed-text` is about 0.3 GB and stays resident alongside the chat model. Fact capture and the
contradiction judge reuse the chat model already loaded by OMNIX, so they add no VRAM (only a few seconds of GPU
time after a reply, in the background).

**More kb-core options** (`~/.config/omnix/kb-core.env`): `KB_CORE_WATCH=notes=/home/you/notes:work=/srv/docs`
files folders into knowledge bases; `KB_CORE_INDEX_CODE=0` limits indexing to notes and PDFs (PDFs need
`poppler-utils` for `pdftotext`); `KB_CORE_VECTOR_DTYPE=float16` halves the index's RAM for very large stores.
Prometheus can scrape `http://127.0.0.1:8100/metrics`.

**Changing the embedding model** (`KB_CORE_EMBED_MODEL`) is safe: the service re-embeds everything in the
background from the stored text, and keyword search keeps working meanwhile.

**Services-only / LAN layout:** run `./scripts/setup-memory.sh --lan` on the server. It binds `0.0.0.0:8100` and
creates a bearer token in `~/.config/omnix/kb-core.token` (mode 600). On each client, set **Settings → Memory →
kb-core URL** to `http://<server>:8100` and paste the token into the bearer-token field (it's stored in the OS
keychain). Firewall 8100 to trusted hosts like 11434/8000.

### Uninstall

```bash
sudo apt remove omnix
docker rm -f omnix-speaches && docker volume rm omnix-hf-cache
systemctl --user disable --now omnix-kb-core && rm ~/.config/systemd/user/omnix-kb-core.service
rm -rf ~/.local/share/omnix ~/.local/bin/piper   # includes the memory database
sudo rm /etc/systemd/system/ollama.service.d/omnix.conf && sudo systemctl daemon-reload && sudo systemctl restart ollama
# Optional: ~/.config/omnix (settings) and ~/.local/share/com.paulmmoore.omnix (logs, audit)
```

---

## 8. Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `nvidia-smi not found/working` | Install the driver and reboot (§2). Or `--cpu` to continue without the GPU |
| `docker: permission denied` after install | Group membership applies at next login. The script uses `sudo docker` meanwhile; log out and back in |
| Speaches never becomes healthy | `docker logs omnix-speaches`. On a GPU host, check `docker run --rm --gpus all ubuntu nvidia-smi` works; if not, `sudo nvidia-ctk runtime configure --runtime=docker && sudo systemctl restart docker` |
| `could not download <model> (HTTP 4xx)` | Wrong STT model id, or no internet. Verify the id on Hugging Face |
| `chat model did not answer within 120 s` | First load of a big model can be slow; retry. `ollama ps` shows GPU/CPU split; `journalctl -u ollama` for CUDA errors |
| Mic button has a yellow dot | `voice.stt_url` is empty: re-run bootstrap or set it in Settings → Voice |
| Read-aloud silent | `tts_voice` must be an **absolute path** to the `.onnx` file with its `.onnx.json` next to it; `doctor.sh` checks this |
| Tauri build fails on `webkit2gtk` / `xdo` | Missing dev packages; re-run bootstrap (it installs `libwebkit2gtk-4.1-dev libxdo-dev …`) |
| Cloud keys won't save | No Secret Service. Install/unlock GNOME Keyring (`gnome-keyring`, `libsecret-1-0`) |
| Approval dialogs never appear / `pkexec` fails | No polkit agent in the session. Use a full desktop session |
| Knowledge view says memory isn't configured | Run `./scripts/setup-memory.sh`, then restart OMNIX. `doctor.sh` checks kb-core when `memory.backend_url` is set |
| Knowledge view shows "keyword-only" / doctor warns kb-core is degraded | The embedding model isn't reachable: `ollama list \| grep nomic-embed-text`, `journalctl --user -u omnix-kb-core -n 50`. Pending items embed automatically once it's back |
| Fact capture never saves anything | `kb-core status` shows `llm off`: set `KB_CORE_LLM_MODEL` in `~/.config/omnix/kb-core.env` to an installed chat model and restart the service; also turn on Settings → Memory → Learn from conversations |
| Remote Ollama rejected with a local-only error | The address isn't private. Use the LAN or Tailscale IP, not a public one |

Still stuck: run `./scripts/doctor.sh` and read the newest `logs/bootstrap-*.log`. If you're using Claude Code on the
server, [`CLAUDE.md`](../CLAUDE.md) has the diagnosis procedure.
