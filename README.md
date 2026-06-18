<div align="center">

# 🌌 OMNIX

### Your God Mode AI Desktop Assistant

[![Version](https://img.shields.io/badge/version-1.0.0-blue.svg)](https://github.com/Paulmmoore3416/omnix)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-orange.svg)](https://tauri.app)
[![Svelte](https://img.shields.io/badge/Svelte-5.0-red.svg)](https://svelte.dev)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org)

**OMNIX** is an AI-powered desktop assistant with system-level access, built with Tauri 2.0 and SvelteKit for native performance.

![OMNIX Interface](uiexample.jpg)

</div>

---

## 🎯 What is OMNIX?

OMNIX is a desktop AI assistant that gives you unprecedented control over your computer through natural language. Execute commands, manage files, automate tasks, and interact with AI—all from a beautiful, native interface.

### Key Features

- 🔐 **System-Level Access** - Execute commands with sudo privileges
- 🤖 **AI Integration** - Local LLMs (Ollama) and cloud APIs (OpenAI, Claude, Grok)
- 🎤 **Voice Interface** - Natural speech input/output
- 💾 **Persistent Memory** - Remembers context across sessions
- ⚡ **Native Performance** - 10x smaller and 3x faster than Electron
- 🎨 **Beautiful UI** - JARVIS-inspired cosmic theme

---

## 🚀 Quick Start

### Prerequisites

- **Node.js** 18+ and npm
- **Rust** 1.70+
- **Git**

### Installation

```bash
# Clone the repository
git clone https://github.com/Paulmmoore3416/omnix.git
cd omnix

# Install dependencies
npm install

# Configure environment (optional for cloud AI)
cp .env.example .env
# Edit .env with your API keys if using cloud services

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

### Platform-Specific Setup

<details>
<summary><b>🍎 macOS</b></summary>

```bash
# Install Xcode Command Line Tools
xcode-select --install

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install dependencies
npm install

# Run OMNIX
npm run tauri dev
```

</details>

<details>
<summary><b>🐧 Linux</b></summary>

```bash
# Install dependencies (Ubuntu/Debian)
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Node dependencies
npm install

# Run OMNIX
npm run tauri dev
```

**Fedora/RHEL:**
```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel
```

**Arch Linux:**
```bash
sudo pacman -S webkit2gtk-4.1 base-devel curl wget file openssl libappindicator-gtk3 librsvg
```

</details>

<details>
<summary><b>🪟 Windows</b></summary>

```powershell
# Install Rust from: https://rustup.rs/

# Install WebView2 (usually pre-installed on Windows 11)
# Download from: https://developer.microsoft.com/microsoft-edge/webview2/

# Install Node dependencies
npm install

# Run OMNIX
npm run tauri dev
```

</details>

---

## 🛠️ Configuration

### Using Local AI (Recommended)

For complete privacy, use Ollama for local AI:

```bash
# Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Pull a model
ollama pull llama2

# Configure OMNIX
# Edit .env:
OLLAMA_HOST=http://localhost:11434
OLLAMA_MODEL=llama2
```

### Using Cloud AI (Optional)

To use cloud AI services, add API keys to `.env`:

```env
# OpenAI
OPENAI_API_KEY=your_key_here

# Anthropic Claude
ANTHROPIC_API_KEY=your_key_here

# Grok (X.AI)
GROK_API_KEY=your_key_here

# Google Gemini
GEMINI_API_KEY=your_key_here
```

---

## 📖 Usage

### Basic Commands

```bash
# Execute system commands
/execute ls -la

# File operations
/file read ~/Documents/notes.txt
/file write ~/test.txt "Hello OMNIX"

# System monitoring
/monitor
```

### Voice Commands

Press `Ctrl+Space` or click the microphone icon:

- "OMNIX, show me system status"
- "Execute git status in my projects folder"
- "Search for all Python files containing 'import pandas'"

### Natural Language

Just talk to OMNIX naturally:

```
"Set up a new React project with TypeScript and Tailwind"
"Find all TODO comments in my code"
"What's using the most CPU right now?"
```

---

## 🏗️ Technology Stack

### Frontend
- **SvelteKit 5.0** - Reactive UI framework
- **TypeScript** - Type-safe development
- **TailwindCSS** - Utility-first styling

### Backend
- **Tauri 2.0** - Rust-based desktop framework
- **Rust** - Systems programming language

### AI/ML
- **Ollama** - Local LLM support
- **OpenAI, Anthropic, X.AI, Google** - Cloud AI APIs
- **Whisper** - Speech-to-text
- **Piper TTS** - Text-to-speech

---

## 📊 Performance

| Metric | OMNIX (Tauri) | Electron Alternative |
|--------|---------------|---------------------|
| **Bundle Size** | ~15 MB | ~150 MB |
| **Memory Usage** | ~50 MB | ~200 MB |
| **Startup Time** | <1s | ~3s |
| **CPU Usage (Idle)** | <1% | ~5% |

---

## 🔒 Security

OMNIX takes security seriously:

- ✅ User confirmation required for sudo operations
- ✅ All commands logged for audit trail
- ✅ Encrypted credential storage
- ✅ No telemetry or data collection
- ✅ Local-first architecture

**Important:** Never commit your `.env` file. It's already in `.gitignore`.

---

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

```bash
# Fork and clone
git clone https://github.com/YOUR_USERNAME/omnix.git

# Create a feature branch
git checkout -b feature/amazing-feature

# Make changes and commit
git commit -m "Add amazing feature"

# Push and create PR
git push origin feature/amazing-feature
```

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- **[Tauri Team](https://tauri.app)** - Amazing desktop framework
- **[Svelte Team](https://svelte.dev)** - Reactive UI library
- **[Ollama](https://ollama.ai)** - Local LLM support
- **[OpenAI](https://openai.com)** - Whisper speech recognition

---

## 📞 Support

- 🐛 **Bug Reports**: [GitHub Issues](https://github.com/Paulmmoore3416/omnix/issues)
- 💬 **Discussions**: [GitHub Discussions](https://github.com/Paulmmoore3416/omnix/discussions)
- 📧 **Email**: paulmmoore3416@gmail.com

---

<div align="center">

**Built with ❤️ using Tauri, Svelte, and Rust**

⭐ Star this repo if you find it useful!

[Report Bug](https://github.com/Paulmmoore3416/omnix/issues) • [Request Feature](https://github.com/Paulmmoore3416/omnix/issues)

</div>
