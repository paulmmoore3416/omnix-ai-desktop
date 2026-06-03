<div align="center">

# 🌌 OMNIX

### Your God Mode AI Desktop Assistant

[![Version](https://img.shields.io/badge/version-1.0.0-blue.svg)](https://github.com/Paulmmoore3416/omnix-ai-desktop)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-orange.svg)](https://tauri.app)
[![Svelte](https://img.shields.io/badge/Svelte-5.0-red.svg)](https://svelte.dev)

**OMNIX** is a revolutionary AI-powered desktop assistant with sudo-level privileges, full system autonomy, and God Mode capabilities. Built with Tauri 2.0 and SvelteKit for blazing-fast native performance.

[Features](#-features) • [Installation](#-installation) • [Usage](#-usage) • [Architecture](#-architecture) • [Contributing](#-contributing)

![OMNIX Interface](docs/screenshot.png)

</div>

---

## ✨ Features

### 🎯 Core Capabilities

- **🔐 God Mode Access** - Sudo-level privileges with secure elevation handling
- **🤖 Autonomous AI Agent** - Self-directed task execution with goal planning
- **🎤 Voice-First Interface** - Natural speech input/output with Whisper + TTS
- **💾 Persistent Memory** - Long-term context retention with vector database
- **🌐 Multi-Modal** - Text, voice, vision, and file analysis
- **⚡ Lightning Fast** - Native performance with Tauri (no Electron bloat)
- **🎨 Beautiful UI** - JARVIS-inspired cosmic theme with animated avatar

### 🛠️ System Integration

- **Command Execution** - Run any system command with full shell access
- **File Operations** - Read, write, move, delete files across the system
- **Process Management** - Monitor and control running applications
- **Network Control** - Manage connections and network settings
- **Automation Scripts** - Create and execute complex workflows
- **Screen Understanding** - Visual analysis of desktop content

### 🧠 AI Capabilities

- **Local LLM Support** - Ollama, LM Studio integration for privacy
- **Cloud Fallback** - Grok, Claude, OpenAI, Gemini support
- **Code Analysis** - Understand and modify codebases
- **Document Processing** - Parse PDFs, DOCX, and more
- **Proactive Assistance** - Anticipates needs and suggests actions
- **Learning System** - Improves from interactions over time

---

## 🚀 Installation

### Prerequisites

- **Node.js** 18+ and npm
- **Rust** 1.70+ (for Tauri)
- **Git**

### Quick Start

```bash
# Clone the repository
git clone https://github.com/Paulmmoore3416/omnix-ai-desktop.git
cd omnix-ai-desktop

# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

### Platform-Specific Setup

#### macOS
```bash
# Install Xcode Command Line Tools
xcode-select --install

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### Linux
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
```

#### Windows
```powershell
# Install Rust
# Download from: https://rustup.rs/

# Install WebView2 (usually pre-installed on Windows 11)
# Download from: https://developer.microsoft.com/microsoft-edge/webview2/
```

---

## 📖 Usage

### Basic Commands

```bash
# Start OMNIX
npm run tauri dev

# Execute system command
/execute ls -la

# File operations
/file read ~/Documents/notes.txt
/file write ~/test.txt "Hello OMNIX"

# Search files
/search "TODO" --path ~/Projects

# System monitoring
/monitor
```

### Voice Commands

Simply click the microphone icon or press `Ctrl+Space` to activate voice input:

- "OMNIX, show me system status"
- "Execute git status in my projects folder"
- "Search for all Python files containing 'import pandas'"
- "Create a backup of my documents folder"

### Advanced Features

#### Autonomous Task Execution
```
"OMNIX, I need to deploy my website. Handle everything."
```
OMNIX will:
1. Analyze the project structure
2. Run tests
3. Build the production bundle
4. Deploy to the server
5. Verify deployment
6. Report results

#### Persistent Memory
```
"Remember that I prefer Python over JavaScript for data analysis"
"What did I tell you about my coding preferences?"
```

---

## 🏗️ Architecture

### Technology Stack

```
┌─────────────────────────────────────────┐
│           Frontend (SvelteKit)          │
│  • Svelte 5.0 with Runes               │
│  • TailwindCSS for styling             │
│  • Animated UI components              │
└─────────────────────────────────────────┘
                    ↕
┌─────────────────────────────────────────┐
│         Tauri 2.0 (Rust Core)          │
│  • Native system APIs                   │
│  • Secure IPC bridge                    │
│  • Plugin system                        │
└─────────────────────────────────────────┘
                    ↕
┌─────────────────────────────────────────┐
│          Backend Services               │
│  • Local LLM (Ollama/LM Studio)        │
│  • Vector DB (Chroma/LanceDB)          │
│  • Voice (Whisper + Piper TTS)         │
└─────────────────────────────────────────┘
```

### Project Structure

```
omnix-ai-desktop/
├── src/                    # Frontend source
│   ├── routes/            # SvelteKit routes
│   ├── lib/               # Shared components
│   └── app.css            # Global styles
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── main.rs       # Entry point
│   │   └── lib.rs        # Core logic
│   ├── Cargo.toml        # Rust dependencies
│   └── tauri.conf.json   # Tauri config
├── static/                # Static assets
├── docs/                  # Documentation
└── scripts/               # Build/deploy scripts
```

---

## 🎨 UI Components

### Animated Avatar

The OMNIX avatar features:
- Real-time lip-sync during speech
- Expressive eye animations
- Glowing neural network patterns
- Smooth floating animation
- Responsive to system state

### Glassmorphic Panels

All UI elements use a consistent glassmorphic design:
- Frosted glass effect with backdrop blur
- Subtle border glow
- Smooth transitions
- Dark cosmic theme

---

## 🔒 Security

OMNIX takes security seriously:

- **Privilege Escalation** - User consent required for sudo operations
- **Sandboxed Execution** - Commands run in controlled environment
- **Encrypted Storage** - Sensitive data encrypted at rest
- **API Key Management** - Secure credential storage
- **Audit Logging** - All system operations logged

---

## 🛣️ Roadmap

### Version 1.1 (Q3 2026)
- [ ] Full Ollama integration
- [ ] Voice cloning for personalized TTS
- [ ] Browser automation
- [ ] Mobile companion app

### Version 1.2 (Q4 2026)
- [ ] Multi-agent collaboration
- [ ] Plugin marketplace
- [ ] Cloud sync for memory
- [ ] Advanced vision capabilities

### Version 2.0 (Q1 2027)
- [ ] Self-improvement system
- [ ] Distributed computing support
- [ ] AR/VR interface
- [ ] Quantum-ready architecture

---

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup

```bash
# Fork and clone the repo
git clone https://github.com/YOUR_USERNAME/omnix-ai-desktop.git

# Create a feature branch
git checkout -b feature/amazing-feature

# Make your changes and commit
git commit -m "Add amazing feature"

# Push and create a pull request
git push origin feature/amazing-feature
```

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- **Tauri Team** - For the amazing framework
- **Svelte Team** - For the reactive UI library
- **Ollama** - For local LLM support
- **OpenAI** - For Whisper speech recognition
- **Community** - For feedback and contributions

---

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/Paulmmoore3416/omnix-ai-desktop/issues)
- **Discussions**: [GitHub Discussions](https://github.com/Paulmmoore3416/omnix-ai-desktop/discussions)
- **Email**: paulmmoore3416@gmail.com

---

<div align="center">

**Built with ❤️ by Paul Moore**

⭐ Star this repo if you find it useful!

[Report Bug](https://github.com/Paulmmoore3416/omnix-ai-desktop/issues) • [Request Feature](https://github.com/Paulmmoore3416/omnix-ai-desktop/issues)

</div>
