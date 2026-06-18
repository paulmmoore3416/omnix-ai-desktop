# ❓ Frequently Asked Questions (FAQ)

Common questions and answers about OMNIX.

---

## Table of Contents

- [General Questions](#general-questions)
- [Installation & Setup](#installation--setup)
- [Usage & Features](#usage--features)
- [Security & Privacy](#security--privacy)
- [Performance & Optimization](#performance--optimization)
- [Troubleshooting](#troubleshooting)
- [Development & Contributing](#development--contributing)

---

## General Questions

### What is OMNIX?

OMNIX is an AI-powered desktop assistant with system-level access that can execute commands, manage files, automate tasks, and interact with you through voice or text. Think of it as having a highly skilled system administrator at your fingertips.

### How is OMNIX different from other AI assistants?

**Key Differences:**
- ✅ **True System Access** - Can execute any command with sudo privileges
- ✅ **Native Performance** - Built with Tauri (Rust), not Electron
- ✅ **Privacy-First** - Supports local LLMs, data stays on your machine
- ✅ **Persistent Memory** - Remembers context across sessions
- ✅ **Autonomous Agent** - Can plan and execute multi-step tasks
- ✅ **Open Source** - Fully transparent and customizable

### Is OMNIX free?

Yes! OMNIX is completely free and open-source under the MIT License. You can use it for personal or commercial purposes without any restrictions.

### What platforms does OMNIX support?

- ✅ **Linux** (Ubuntu, Fedora, Arch, Debian, etc.)
- ✅ **macOS** (10.15+)
- ✅ **Windows** (10/11)

### Do I need an internet connection?

**No!** OMNIX can run completely offline using local LLMs like Ollama. However, cloud AI features (GPT-4, Claude, etc.) require internet access.

---

## Installation & Setup

### What are the system requirements?

**Minimum:**
- CPU: Dual-core 2.0 GHz
- RAM: 4 GB
- Storage: 500 MB
- OS: Windows 10, macOS 10.15, Linux (kernel 4.4+)

**Recommended:**
- CPU: Quad-core 3.0 GHz+
- RAM: 8 GB+
- Storage: 2 GB (for local LLM models)
- GPU: Optional, for faster local AI inference

### How do I install OMNIX?

```bash
# Clone the repository
git clone https://github.com/Paulmmoore3416/omnix.git
cd omnix

# Install dependencies
npm install

# Configure environment
cp .env.example .env

# Run OMNIX
npm run tauri dev
```

See [README.md](README.md#installation) for detailed platform-specific instructions.

### Do I need to install Rust?

Yes, Rust is required to build Tauri applications. Install it from [rustup.rs](https://rustup.rs/).

### Can I use OMNIX without API keys?

Yes! Configure a local LLM (Ollama) and you won't need any API keys:

```env
OLLAMA_HOST=http://localhost:11434
OLLAMA_MODEL=llama2
```

### How do I get API keys for cloud services?

- **OpenAI**: https://platform.openai.com/api-keys
- **Anthropic**: https://console.anthropic.com/
- **Grok (X.AI)**: https://x.ai/api
- **Google Gemini**: https://makersuite.google.com/app/apikey

---

## Usage & Features

### How do I activate voice input?

- **Keyboard**: Press `Ctrl+Space`
- **Mouse**: Click the microphone icon
- **Command**: Type `/voice start`

### Can OMNIX execute sudo commands?

Yes, but with safety controls:
1. User confirmation required (configurable)
2. All commands logged to audit trail
3. Can be disabled entirely in settings

```env
ENABLE_SUDO=true
REQUIRE_CONFIRMATION=true
LOG_ALL_COMMANDS=true
```

### How does persistent memory work?

OMNIX uses vector databases to store and retrieve context:

```bash
# Save information
"Remember that I prefer Python for data analysis"

# Recall later
"What did I tell you about my preferences?"
```

Memory is stored locally in `./data/memory/`.

### Can OMNIX automate multi-step tasks?

Yes! Enable autonomous mode:

```env
ENABLE_AUTONOMOUS_MODE=true
MAX_AUTONOMOUS_STEPS=10
```

Example:
```
"Deploy my website: run tests, build, and push to production"
```

OMNIX will plan and execute all steps automatically.

### How do I use OMNIX with my IDE?

OMNIX can integrate with your development workflow:

```bash
# Open file in VS Code
"Open app.ts in VS Code"

# Run tests
"Run npm test in my project"

# Git operations
"Commit all changes with message 'Fix bug'"
```

### Can OMNIX access the internet?

Yes, OMNIX can:
- Make HTTP requests
- Download files
- Scrape websites
- Check connectivity
- Monitor network traffic

### Does OMNIX support plugins?

Plugin system is planned for v1.2. Currently, you can extend OMNIX by:
- Modifying the source code
- Adding custom commands
- Creating shell scripts

---

## Security & Privacy

### Is OMNIX safe to use?

Yes, with proper configuration:

✅ **Built-in Safety Features:**
- User confirmation for sudo operations
- Command audit logging
- Sandboxed execution
- Encrypted credential storage

⚠️ **User Responsibility:**
- Review commands before confirming
- Keep API keys secure
- Monitor audit logs
- Use strong encryption keys

### Does OMNIX collect telemetry?

**No!** Telemetry is disabled by default:

```env
ENABLE_TELEMETRY=false
```

Even if enabled, telemetry is:
- Opt-in only
- Anonymized
- Stored locally
- Never transmitted without consent

### Where is my data stored?

All data is stored locally:
- **Memory**: `./data/memory/`
- **Logs**: `./logs/`
- **Config**: `.env` file
- **Vector DB**: `./data/vector_db/`

Nothing is sent to external servers unless you use cloud AI APIs.

### Can I use OMNIX in a corporate environment?

Yes, but consider:
- Disable sudo access if not needed
- Use local LLMs for sensitive data
- Enable comprehensive logging
- Review security policies
- Conduct security audit

### How do I secure my API keys?

1. **Never commit `.env` to git** (already in `.gitignore`)
2. **Use environment-specific keys**
3. **Rotate keys every 90 days**
4. **Use strong encryption**:
   ```bash
   openssl rand -base64 32
   ```
5. **Monitor key usage**

### What permissions does OMNIX need?

**Linux/macOS:**
- File system access
- Command execution
- Network access
- Microphone (for voice input)

**Windows:**
- Same as above
- WebView2 runtime

All permissions are requested at runtime.

---

## Performance & Optimization

### Why is OMNIX so fast?

OMNIX uses Tauri (Rust + WebView) instead of Electron:
- **10x smaller** bundle size
- **3x faster** startup time
- **4x less** memory usage
- Native system integration

### How much RAM does OMNIX use?

- **Idle**: ~50 MB
- **Active**: ~100-200 MB
- **With Local LLM**: +2-8 GB (depends on model)

### Can I run OMNIX on a low-end machine?

Yes, but:
- Use cloud APIs instead of local LLMs
- Disable autonomous mode
- Reduce memory cache size
- Close other applications

### How do I optimize performance?

```env
# Reduce memory usage
MAX_MEMORY_SIZE=500
VECTOR_DB_PATH=./data/vector_db

# Disable features
ENABLE_AUTONOMOUS_MODE=false
ENABLE_PROACTIVE_SUGGESTIONS=false

# Use faster models
OLLAMA_MODEL=llama2:7b  # Instead of 13b or 70b
```

### Does OMNIX support GPU acceleration?

Yes, for local LLMs:
- **NVIDIA**: CUDA support via Ollama
- **AMD**: ROCm support (Linux)
- **Apple Silicon**: Metal acceleration (macOS)

---

## Troubleshooting

### OMNIX won't start

**Check:**
1. Node.js and Rust installed?
2. Dependencies installed? (`npm install`)
3. `.env` file configured?
4. Port conflicts? (check if another app uses the same port)

**Try:**
```bash
# Clean install
rm -rf node_modules package-lock.json
npm install

# Clear cache
rm -rf .svelte-kit

# Rebuild
npm run tauri build
```

### Voice input not working

**Check:**
1. Microphone permissions granted?
2. Whisper model installed?
3. Audio device selected correctly?

**Try:**
```bash
# Test microphone
arecord -l  # Linux
system_profiler SPAudioDataType  # macOS

# Reinstall Whisper
pip install openai-whisper --upgrade
```

### Commands fail with "Permission Denied"

**Solutions:**
1. Enable sudo: `ENABLE_SUDO=true`
2. Run with elevated privileges
3. Check file permissions
4. Add user to required groups (Linux)

### Local LLM not responding

**Check:**
1. Ollama running? (`ollama serve`)
2. Model downloaded? (`ollama pull llama2`)
3. Correct endpoint? (`OLLAMA_HOST=http://localhost:11434`)

**Try:**
```bash
# Test Ollama
curl http://localhost:11434/api/generate -d '{
  "model": "llama2",
  "prompt": "Hello"
}'
```

### High memory usage

**Solutions:**
1. Reduce memory cache: `MAX_MEMORY_SIZE=500`
2. Use smaller LLM model
3. Clear vector database: `rm -rf ./data/vector_db`
4. Restart OMNIX

### API errors

**Check:**
1. API keys valid?
2. Internet connection?
3. Rate limits exceeded?
4. Service status (check provider's status page)

**Try:**
```bash
# Test API key
curl https://api.openai.com/v1/models \
  -H "Authorization: Bearer $OPENAI_API_KEY"
```

### Build errors

**Common issues:**

**Rust not found:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

**WebKit missing (Linux):**
```bash
sudo apt install libwebkit2gtk-4.1-dev
```

**Node version:**
```bash
# Use Node 18+
nvm install 18
nvm use 18
```

---

## Development & Contributing

### How can I contribute?

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

**Quick start:**
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Submit a pull request

### How do I add a new command?

1. **Define command in Rust** (`src-tauri/src/lib.rs`):
```rust
#[tauri::command]
fn my_command(arg: String) -> Result<String, String> {
    // Implementation
    Ok(format!("Result: {}", arg))
}
```

2. **Register command** (`src-tauri/src/main.rs`):
```rust
.invoke_handler(tauri::generate_handler![my_command])
```

3. **Call from frontend**:
```typescript
import { invoke } from '@tauri-apps/api/tauri';
const result = await invoke('my_command', { arg: 'value' });
```

### How do I add a new AI model?

Edit `.env`:
```env
# Add your model
MY_AI_API_KEY=your_key_here
MY_AI_ENDPOINT=https://api.example.com
```

Implement in code (see [API.md](API.md) for details).

### Where can I get help?

- 📖 [Documentation](https://github.com/Paulmmoore3416/omnix/wiki)
- 💬 [Discussions](https://github.com/Paulmmoore3416/omnix/discussions)
- 🐛 [Issues](https://github.com/Paulmmoore3416/omnix/issues)
- 📧 [Email](mailto:paulmmoore3416@gmail.com)

### How do I report a bug?

1. Check [existing issues](https://github.com/Paulmmoore3416/omnix/issues)
2. Create a new issue with:
   - Clear description
   - Steps to reproduce
   - Expected vs actual behavior
   - System information
   - Logs (if applicable)

### How do I request a feature?

Open a [feature request](https://github.com/Paulmmoore3416/omnix/issues/new) with:
- Clear description of the feature
- Use cases and benefits
- Proposed implementation (optional)

---

## Still Have Questions?

- 💬 [Ask in Discussions](https://github.com/Paulmmoore3416/omnix/discussions)
- 📧 [Email Support](mailto:paulmmoore3416@gmail.com)
- 📖 [Read the Docs](https://github.com/Paulmmoore3416/omnix/wiki)

---

**Last Updated:** June 2026  
**Version:** 1.0.0