#!/bin/bash

# OMNIX Setup Script
# This script sets up the development environment for OMNIX

set -e

echo "🌌 OMNIX Setup Script"
echo "====================="
echo ""

# Check for Node.js
if ! command -v node &> /dev/null; then
    echo "❌ Node.js is not installed. Please install Node.js 18+ first."
    echo "   Visit: https://nodejs.org/"
    exit 1
fi

NODE_VERSION=$(node -v | cut -d'v' -f2 | cut -d'.' -f1)
if [ "$NODE_VERSION" -lt 18 ]; then
    echo "❌ Node.js version 18+ is required. Current version: $(node -v)"
    exit 1
fi

echo "✅ Node.js $(node -v) detected"

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust is not installed."
    echo "   Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

echo "✅ Rust $(rustc --version) detected"

# Platform-specific dependencies
OS="$(uname -s)"
case "${OS}" in
    Linux*)
        echo "🐧 Detected Linux"
        if command -v apt-get &> /dev/null; then
            echo "Installing Linux dependencies..."
            sudo apt-get update
            sudo apt-get install -y \
                libwebkit2gtk-4.1-dev \
                build-essential \
                curl \
                wget \
                file \
                libssl-dev \
                libayatana-appindicator3-dev \
                librsvg2-dev
        elif command -v dnf &> /dev/null; then
            echo "Installing Fedora dependencies..."
            sudo dnf install -y \
                webkit2gtk4.1-devel \
                openssl-devel \
                curl \
                wget \
                file \
                libappindicator-gtk3-devel \
                librsvg2-devel
        fi
        ;;
    Darwin*)
        echo "🍎 Detected macOS"
        if ! command -v xcode-select &> /dev/null; then
            echo "Installing Xcode Command Line Tools..."
            xcode-select --install
        fi
        ;;
    MINGW*|MSYS*|CYGWIN*)
        echo "🪟 Detected Windows"
        echo "Please ensure you have:"
        echo "  - Visual Studio Build Tools"
        echo "  - WebView2 Runtime"
        ;;
    *)
        echo "⚠️  Unknown operating system: ${OS}"
        ;;
esac

# Install npm dependencies
echo ""
echo "📦 Installing npm dependencies..."
npm install

# Install Tauri CLI
echo ""
echo "🦀 Installing Tauri CLI..."
cargo install tauri-cli --version "^2.0.0" || true

echo ""
echo "✅ Setup complete!"
echo ""
echo "🚀 To start OMNIX in development mode:"
echo "   npm run tauri dev"
echo ""
echo "📦 To build for production:"
echo "   npm run tauri build"
echo ""
echo "📚 For more information, see README.md"

# Made with Bob
