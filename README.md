<div align="center">

# 🌌 OMNIX

### Your God Mode AI Desktop Assistant

[![Version](https://img.shields.io/badge/version-1.0.0-blue.svg)](https://github.com/Paulmmoore3416/omnix)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-orange.svg)](https://tauri.app)
[![Svelte](https://img.shields.io/badge/Svelte-5.0-red.svg)](https://svelte.dev)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)
[![Code of Conduct](https://img.shields.io/badge/code%20of-conduct-ff69b4.svg)](CODE_OF_CONDUCT.md)

**OMNIX** is a revolutionary AI-powered desktop assistant with sudo-level privileges, full system autonomy, and God Mode capabilities. Built with Tauri 2.0 and SvelteKit for blazing-fast native performance.

[Features](#-features) • [Installation](#-installation) • [Usage](#-usage) • [Architecture](#-architecture) • [Contributing](#-contributing) • [Security](#-security)

![OMNIX Interface](uiexample.jpg)

</div>

---

## 🎯 What is OMNIX?

OMNIX is not just another AI assistant—it's a **complete system control interface** that gives you unprecedented power over your computer through natural language. Think of it as having a highly skilled system administrator at your fingertips, ready to execute any command, automate any task, and manage your entire digital workspace.

### Why OMNIX?

- 🚀 **Native Performance**: Built with Tauri, not Electron—10x smaller, 3x faster
- 🔐 **True System Access**: Sudo privileges with intelligent safety controls
- 🧠 **Persistent Memory**: Remembers context across sessions using vector databases
- 🎤 **Voice-First**: Natural speech interface with Whisper + TTS
- 🤖 **Autonomous Agent**: Can plan and execute multi-step tasks independently
- 🌐 **Privacy-Focused**: Local LLM support—your data never leaves your machine
- 🎨 **Beautiful UI**: JARVIS-inspired cosmic interface with animated avatar

---

## ✨ Features

### 🎯 Core Capabilities

#### 🔐 God Mode Access
- **Sudo-level privileges** with secure elevation handling
- User consent required for all privileged operations
- Comprehensive audit logging of all system commands
- Configurable safety controls and confirmation prompts

#### 🤖 Autonomous AI Agent
- Self-directed task execution with intelligent goal planning
- Multi-step workflow automation
- Proactive problem-solving and error recovery
- Learning from past interactions

#### 🎤 Voice-First Interface
- Natural speech input with OpenAI Whisper
- High-quality text-to-speech with Piper TTS
- Hands-free operation mode
- Multiple language support

#### 💾 Persistent Memory
- Long-term context retention using vector databases
- Semantic search across conversation history
- User preference learning
- Project-specific memory contexts

#### 🌐 Multi-Modal Capabilities
- Text, voice, and vision processing
- File analysis (PDFs, DOCX, code, images)
- Screen understanding and OCR
- Web scraping and data extraction

#### ⚡ Lightning Fast
- Native performance with Tauri (Rust + WebView)
- 10x smaller than Electron apps
- Instant startup time
- Minimal resource usage

### 🛠️ System Integration

#### Command Execution
```bash
# Execute any system command
"Run git status in my projects folder"
"Install npm packages and start the dev server"
"Find all Python files modified in the last week"
```

#### File Operations
```bash
# Comprehensive file management
"Create a backup of my documents folder"
"Search for TODO comments in all JavaScript files"
"Organize my downloads by file type"
```

#### Process Management
```bash
# Monitor and control applications
"Show me what's using the most CPU"
"Kill the process on port 3000"
"Start my development environment"
```

#### Network Control
```bash
# Manage network settings
"Check my internet speed"
"Show active network connections"
"Configure firewall rules for port 8080"
```

#### Automation Scripts
```bash
# Create complex workflows
"Deploy my website: run tests, build, and push to production"
"Set up a new React project with TypeScript and Tailwind"
"Backup my code, compress it, and upload to cloud storage"
```

### 🧠 AI Capabilities

#### Local LLM Support
- **Ollama** integration for complete privacy
- **LM Studio** compatibility
- Run powerful models locally (Llama, Mistral, CodeLlama)
- No internet required for basic operations

#### Cloud Fallback
- **Grok (X.AI)** - Fast and capable
- **Claude (Anthropic)** - Best reasoning
- **GPT-4 (OpenAI)** - Most versatile
- **Gemini (Google)** - Multimodal excellence

#### Code Analysis
- Understand entire codebases
- Generate and refactor code
- Debug and optimize
- Documentation generation

#### Document Processing
- Parse PDFs, DOCX, spreadsheets
- Extract structured data
- Summarize long documents
- Convert between formats

#### Proactive Assistance
- Anticipates your needs
- Suggests optimizations
- Monitors system health
- Alerts on issues

---

## 🚀 Installation

### Prerequisites

- **Node.js** 18+ and npm
- **Rust** 1.70+ (for Tauri)
- **Git**

### Quick Start

```bash
# Clone the repository
git clone https://github.com/Paulmmoore3416/omnix.git
cd omnix

# Install dependencies
npm install

# Configure environment
cp .env.example .env
# Edit .env with your API keys (optional for cloud LLMs)

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
sudo dnf install webkit2gtk4.1-devel \
  openssl-devel \
  curl \
  wget \
  file \
  libappindicator-gtk3-devel \
  librsvg2-devel
```

**Arch Linux:**
```bash
sudo pacman -S webkit2gtk-4.1 \
  base-devel \
  curl \
  wget \
  file \
  openssl \
  libappindicator-gtk3 \
  librsvg
```

</details>

<details>
<summary><b>🪟 Windows</b></summary>

```powershell
# Install Rust
# Download from: https://rustup.rs/

# Install WebView2 (usually pre-installed on Windows 11)
# Download from: https://developer.microsoft.com/microsoft-edge/webview2/

# Install Node dependencies
npm install

# Run OMNIX
npm run tauri dev
```

</details>

### Configuration

1. **Copy environment template:**
   ```bash
   cp .env.example .env
   ```

2. **Configure AI models** (choose one or more):
   ```env
   # Local LLM (recommended for privacy)
   OLLAMA_HOST=http://localhost:11434
   OLLAMA_MODEL=llama2
   
   # Cloud LLMs (optional)
   OPENAI_API_KEY=your_key_here
   ANTHROPIC_API_KEY=your_key_here
   GROK_API_KEY=your_key_here
   ```

3. **Security settings:**
   ```env
   ENABLE_SUDO=true
   REQUIRE_CONFIRMATION=true
   LOG_ALL_COMMANDS=true
   ENCRYPTION_KEY=generate_secure_key_here
   ```

---

## 📖 Usage

### Basic Commands

```bash
# System commands
/execute ls -la
/execute git status

# File operations
/file read ~/Documents/notes.txt
/file write ~/test.txt "Hello OMNIX"
/file search "TODO" --path ~/Projects

# System monitoring
/monitor cpu
/monitor memory
/monitor processes
```

### Voice Commands

Activate voice input with the microphone icon or `Ctrl+Space`:

- **"OMNIX, show me system status"**
- **"Execute git status in my projects folder"**
- **"Search for all Python files containing 'import pandas'"**
- **"Create a backup of my documents folder"**
- **"What's using the most memory right now?"**

### Natural Language Examples

#### Development Workflow
```
"Set up a new React project called 'my-app' with TypeScript, 
Tailwind CSS, and ESLint. Initialize git and make the first commit."
```

#### System Maintenance
```
"Check for system updates, clean up old log files, 
and show me disk space usage."
```

#### Data Analysis
```
"Find all CSV files in my Downloads folder, analyze them, 
and create a summary report."
```

#### Automation
```
"Every day at 9 AM, pull the latest changes from my repositories, 
run tests, and notify me of any failures."
```

### Advanced Features

#### Autonomous Task Execution
```
"OMNIX, I need to deploy my website. Handle everything."
```

OMNIX will:
1. ✅ Analyze the project structure
2. ✅ Run tests and linting
3. ✅ Build the production bundle
4. ✅ Optimize assets
5. ✅ Deploy to the server
6. ✅ Verify deployment
7. ✅ Report results

#### Persistent Memory
```
User: "Remember that I prefer Python over JavaScript for data analysis"
OMNIX: "Noted. I'll prioritize Python for your data analysis tasks."

[Later...]
User: "What did I tell you about my coding preferences?"
OMNIX: "You prefer Python over JavaScript for data analysis tasks."
```

#### Context-Aware Assistance
```
User: "I'm working on a machine learning project"
OMNIX: "I see you're in a Python project with scikit-learn. 
       Would you like me to help with model training or data preprocessing?"
```

---

## 🏗️ Architecture

### Technology Stack

```
┌─────────────────────────────────────────┐
│        Frontend (SvelteKit 5.0)         │
│  • Reactive UI with Svelte Runes       │
│  • TailwindCSS for styling             │
│  • Animated components                  │
│  • Real-time updates                    │
└─────────────────────────────────────────┘
                    ↕ IPC Bridge
┌─────────────────────────────────────────┐
│         Tauri 2.0 (Rust Core)          │
│  • Native system APIs                   │
│  • Secure command execution             │
│  • File system access                   │
│  • Process management                   │
│  • Plugin system                        │
└─────────────────────────────────────────┘
                    ↕ API Calls
┌─────────────────────────────────────────┐
│          Backend Services               │
│  • Local LLM (Ollama/LM Studio)        │
│  • Vector DB (Chroma/LanceDB)          │
│  • Voice (Whisper + Piper TTS)         │
│  • Cloud APIs (optional)                │
└─────────────────────────────────────────┘
```

### Project Structure

```
omnix/
├── src/                      # Frontend source
│   ├── routes/              # SvelteKit routes
│   │   ├── +page.svelte    # Main interface
│   │   ├── +layout.svelte  # App layout
│   │   └── +layout.ts      # Layout config
│   ├── lib/                 # Shared components
│   │   └── components/
│   │       ├── Avatar.svelte
│   │       ├── KnowledgeView.svelte
│   │       ├── SettingsView.svelte
│   │       └── SystemControlView.svelte
│   ├── app.css             # Global styles
│   └── app.html            # HTML template
│
├── src-tauri/               # Rust backend
│   ├── src/
│   │   ├── main.rs         # Entry point
│   │   └── lib.rs          # Core logic
│   ├── Cargo.toml          # Rust dependencies
│   ├── tauri.conf.json     # Tauri configuration
│   └── capabilities/       # Permission definitions
│
├── static/                  # Static assets
├── scripts/                 # Build/setup scripts
│   ├── setup.sh            # Linux/macOS setup
│   └── setup.ps1           # Windows setup
│
├── docs/                    # Documentation
├── .env.example            # Environment template
├── .gitignore              # Git ignore rules
├── package.json            # Node dependencies
├── tailwind.config.js      # Tailwind config
├── vite.config.js          # Vite config
├── README.md               # This file
├── LICENSE                 # MIT License
├── CONTRIBUTING.md         # Contribution guide
├── CODE_OF_CONDUCT.md      # Code of conduct
├── SECURITY.md             # Security policy
└── CHANGELOG.md            # Version history
```

### Data Flow

```
User Input (Voice/Text)
        ↓
    Frontend (Svelte)
        ↓
    Tauri IPC Bridge
        ↓
    Rust Backend
        ↓
    ┌─────────────┬─────────────┬─────────────┐
    ↓             ↓             ↓             ↓
System APIs   LLM Service   Vector DB    File System
    ↓             ↓             ↓             ↓
    └─────────────┴─────────────┴─────────────┘
                    ↓
            Response Processing
                    ↓
            Frontend Update
                    ↓
            User Output
```

---

## 🎨 UI Components

### Animated Avatar

The OMNIX avatar is a sophisticated visual representation featuring:

- **Real-time lip-sync** during speech output
- **Expressive eye animations** that respond to system state
- **Glowing neural network patterns** with particle effects
- **Smooth floating animation** with subtle breathing motion
- **State indicators** (thinking, speaking, listening, idle)

### Glassmorphic Design

All UI elements use a consistent glassmorphic design language:

- **Frosted glass effect** with backdrop blur
- **Subtle border glow** with gradient accents
- **Smooth transitions** and micro-interactions
- **Dark cosmic theme** with purple/blue gradients
- **Responsive layout** that adapts to window size

### Command Interface

- **Auto-complete** for common commands
- **Syntax highlighting** for code blocks
- **Markdown rendering** for formatted responses
- **Code copy buttons** for easy snippet extraction
- **Command history** with search

---

## 🔒 Security

OMNIX takes security seriously. See [SECURITY.md](SECURITY.md) for comprehensive details.

### Key Security Features

- ✅ **Privilege Escalation Control** - User consent required for sudo operations
- ✅ **Sandboxed Execution** - Commands run in controlled environment
- ✅ **Encrypted Storage** - Sensitive data encrypted at rest
- ✅ **API Key Management** - Secure credential storage
- ✅ **Audit Logging** - All system operations logged
- ✅ **No Telemetry** - Your data stays on your machine

### Security Best Practices

1. **Never commit `.env` files** - Already in `.gitignore`
2. **Use strong encryption keys** - Generate with `openssl rand -base64 32`
3. **Rotate API keys regularly** - Every 90 days recommended
4. **Enable confirmation prompts** - Set `REQUIRE_CONFIRMATION=true`
5. **Review audit logs** - Check `./logs/audit.log` regularly

---

## 🛣️ Roadmap

### Version 1.1 (Q3 2026)
- [ ] Full Ollama integration with model management
- [ ] Voice cloning for personalized TTS
- [ ] Browser automation with Playwright
- [ ] Mobile companion app (iOS/Android)
- [ ] Plugin marketplace

### Version 1.2 (Q4 2026)
- [ ] Multi-agent collaboration system
- [ ] Cloud sync for memory and settings
- [ ] Advanced vision capabilities (OCR, object detection)
- [ ] Natural language to SQL queries
- [ ] Integration with popular dev tools

### Version 2.0 (Q1 2027)
- [ ] Self-improvement system (learns from corrections)
- [ ] Distributed computing support
- [ ] AR/VR interface
- [ ] Quantum-ready architecture
- [ ] Enterprise features (team collaboration, admin controls)

---

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

### Quick Contribution Guide

1. **Fork the repository**
2. **Create a feature branch**: `git checkout -b feature/amazing-feature`
3. **Make your changes** and commit: `git commit -m "Add amazing feature"`
4. **Push to your fork**: `git push origin feature/amazing-feature`
5. **Open a Pull Request**

### Development Setup

```bash
# Fork and clone
git clone https://github.com/YOUR_USERNAME/omnix.git
cd omnix

# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Run tests
npm test

# Build for production
npm run tauri build
```

### Areas for Contribution

- 🐛 **Bug fixes** - Help us squash bugs
- ✨ **New features** - Implement items from the roadmap
- 📝 **Documentation** - Improve guides and examples
- 🎨 **UI/UX** - Enhance the interface
- 🧪 **Testing** - Add test coverage
- 🌍 **Translations** - Add language support

---

## 📊 Performance

### Benchmarks

| Metric | OMNIX (Tauri) | Electron Alternative |
|--------|---------------|---------------------|
| **Bundle Size** | ~15 MB | ~150 MB |
| **Memory Usage** | ~50 MB | ~200 MB |
| **Startup Time** | <1s | ~3s |
| **CPU Usage (Idle)** | <1% | ~5% |

### System Requirements

**Minimum:**
- CPU: Dual-core 2.0 GHz
- RAM: 4 GB
- Storage: 500 MB
- OS: Windows 10, macOS 10.15, Linux (kernel 4.4+)

**Recommended:**
- CPU: Quad-core 3.0 GHz+
- RAM: 8 GB+
- Storage: 2 GB (for local LLM models)
- OS: Windows 11, macOS 12+, Linux (kernel 5.10+)

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

### What this means:
- ✅ Commercial use
- ✅ Modification
- ✅ Distribution
- ✅ Private use
- ❌ Liability
- ❌ Warranty

---

## 🙏 Acknowledgments

- **[Tauri Team](https://tauri.app)** - For the amazing framework
- **[Svelte Team](https://svelte.dev)** - For the reactive UI library
- **[Ollama](https://ollama.ai)** - For local LLM support
- **[OpenAI](https://openai.com)** - For Whisper speech recognition
- **[Anthropic](https://anthropic.com)** - For Claude API
- **[X.AI](https://x.ai)** - For Grok API
- **Community Contributors** - For feedback and contributions

---

## 📞 Support & Community

### Get Help

- 📖 **Documentation**: [GitHub Wiki](https://github.com/Paulmmoore3416/omnix/wiki)
- 🐛 **Bug Reports**: [GitHub Issues](https://github.com/Paulmmoore3416/omnix/issues)
- 💬 **Discussions**: [GitHub Discussions](https://github.com/Paulmmoore3416/omnix/discussions)
- 📧 **Email**: paulmmoore3416@gmail.com

### Stay Updated

- ⭐ **Star this repo** to show support
- 👁️ **Watch** for updates and releases
- 🍴 **Fork** to contribute

---

## 🎯 Use Cases

### For Developers
- Automate development workflows
- Manage multiple projects
- Quick code analysis and refactoring
- Git operations and deployment

### For System Administrators
- Server management and monitoring
- Automated maintenance tasks
- Log analysis and troubleshooting
- Security audits

### For Power Users
- File organization and cleanup
- Batch processing tasks
- System optimization
- Custom automation scripts

### For Data Scientists
- Data preprocessing pipelines
- Model training automation
- Result analysis and visualization
- Documentation generation

---

## 🌟 Showcase

### Built With OMNIX

Share your projects and automations! Open a discussion to showcase what you've built.

---

<div align="center">

## 💖 Built with passion by [Paul Moore](https://github.com/Paulmmoore3416)

### ⭐ Star this repo if you find it useful!

[![GitHub stars](https://img.shields.io/github/stars/Paulmmoore3416/omnix?style=social)](https://github.com/Paulmmoore3416/omnix/stargazers)
[![GitHub forks](https://img.shields.io/github/forks/Paulmmoore3416/omnix?style=social)](https://github.com/Paulmmoore3416/omnix/network/members)
[![GitHub watchers](https://img.shields.io/github/watchers/Paulmmoore3416/omnix?style=social)](https://github.com/Paulmmoore3416/omnix/watchers)

[Report Bug](https://github.com/Paulmmoore3416/omnix/issues) • [Request Feature](https://github.com/Paulmmoore3416/omnix/issues) • [Contribute](CONTRIBUTING.md)

**Made with ❤️ using Tauri, Svelte, and Rust**

</div>
