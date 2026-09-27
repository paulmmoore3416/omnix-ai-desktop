# OMNIX Help: setting up local models

OMNIX runs its AI on your own hardware. A few local services do the work. OMNIX finds each one at a fixed address, and by default nothing leaves this machine.

| Service | What it does | Default address |
|---|---|---|
| **Ollama** | Chat model (answers, tools, summaries) and the embedding model for memory | `http://127.0.0.1:11434` |
| **Speaches** (faster-whisper) | Speech-to-text for push-to-talk | `http://127.0.0.1:8000` |
| **Piper** | Read-aloud (text-to-speech), runs as a local program | a `.onnx` voice file on disk |
| **kb-core** | Long-term memory and document search | `http://127.0.0.1:8100` |

On Linux, `./scripts/bootstrap.sh --yes` installs and wires up all of them. Run `./scripts/doctor.sh` at any time to check them. It must end with `0 failed`.

## 1. Install Ollama and a chat model

1. Install Ollama from ollama.com, or let `bootstrap.sh` do it.
2. Pull a chat model that fits your GPU's memory:

   | GPU memory | Good starting model |
   |---|---|
   | 6–8 GB | `qwen3:8b` |
   | 12–16 GB | `qwen3:14b` |
   | CPU only | `qwen3:4b` (slow but works) |

   ```bash
   ollama pull qwen3:8b
   ollama pull nomic-embed-text   # embeddings for memory search
   ```
3. In OMNIX: **Settings → AI Models**. Set the provider to **Ollama**, pick the model from the list, then **Test**. OMNIX lists whatever models Ollama reports. It doesn't assume any particular model.

**Check it runs on the GPU:** `ollama ps` shows a `PROCESSOR` column. `100% GPU` is what you want. If part of the model spills to the CPU, pick a smaller model or a smaller context window in Settings → AI Models.

**AMD cards** run through Vulkan. They need the `amdgpu` kernel driver and `mesa-vulkan-drivers`. `journalctl -u ollama | grep "using device"` shows which card Ollama chose.

## 2. Voice: speech-to-text and read-aloud

- **Push-to-talk** needs Speaches. `bootstrap.sh` starts it in Docker (on an NVIDIA GPU when one is present) and downloads the whisper model. Test it in **Settings → Voice → Test microphone**.
- **Read-aloud** needs Piper plus a voice. The voice path in Settings → Voice must be **absolute**, and both `voice.onnx` and `voice.onnx.json` must exist next to each other.

## 3. Memory (kb-core)

kb-core stores memories and indexes your notes. It needs `nomic-embed-text` in Ollama for meaning-based search. Without it, search falls back to keywords and doctor reports it as *degraded*.

- Install or repair: `./scripts/setup-memory.sh`. Encrypt at rest: `./scripts/setup-memory.sh --encrypt`.
- Index a folder of notes (an Obsidian vault works): add it to `KB_CORE_WATCH` in `~/.config/omnix/kb-core.env`, for example `KB_CORE_WATCH=notes=/home/you/Vault`, then restart with `systemctl --user restart omnix-kb-core`. OMNIX spots Obsidian vaults automatically. It reads their tags and front matter and skips `.obsidian/` and `.trash/`.
- Chat commands: `/remember <fact>`, `/recall <question>`, `/search <words>`.

## 4. Running the services on another machine

Put Ollama, Speaches and kb-core on a home server, either with `bootstrap.sh --services-only --lan` or with the Docker Compose file in `deploy/`. Then point **Settings → AI Models → Ollama host** at `http://<server>:11434`. These services have no login of their own, so keep them on your LAN or tailnet behind a firewall. Never expose them to the internet.

## 5. Cloud models (optional)

Local-only mode is **on** by default and blocks every cloud provider. To use OpenAI, Anthropic, Gemini or xAI with your own API key:

1. Settings → Security: turn off **local-only mode**. You'll be asked to confirm.
2. Settings → AI Models: choose the provider and paste your key. It goes straight into the OS keychain and is never shown again.

## 6. Your usage and your license

- **Settings → Usage** shows how much work your local models did: turns, tokens and tool calls, plus what the same tokens would have cost from a cloud API at a reference rate you choose. Only daily counts are kept, on this machine. No prompts or replies are recorded.
- **Settings → License** shows your tier and takes a license key. Keys are checked offline. In this version the tiers are informational, and every feature works without a license.

## 7. When something is wrong

| Symptom | Try |
|---|---|
| "Ollama not reachable" | `systemctl status ollama`, then `sudo systemctl restart ollama` |
| Model is slow, `ollama ps` shows CPU | Use a smaller model or a shorter context window |
| Microphone test hears nothing | Check the input device and volume in your OS sound settings |
| Read-aloud fails | Make the Piper voice path absolute and check that both voice files exist |
| Memory search says *degraded* | `ollama pull nomic-embed-text`, then `kb-core status` |

For anything else, `./scripts/doctor.sh` names the failing piece and the usual fix.
