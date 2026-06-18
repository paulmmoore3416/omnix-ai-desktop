# 🚀 OMNIX Deployment Guide

Complete guide for building and deploying OMNIX across different platforms.

---

## Table of Contents

- [Prerequisites](#prerequisites)
- [Building for Production](#building-for-production)
- [Platform-Specific Builds](#platform-specific-builds)
- [Distribution](#distribution)
- [Auto-Updates](#auto-updates)
- [CI/CD Integration](#cicd-integration)
- [Troubleshooting](#troubleshooting)

---

## Prerequisites

### Required Tools

- **Node.js** 18+ and npm
- **Rust** 1.70+
- **Git**
- Platform-specific build tools (see below)

### Environment Setup

```bash
# Clone repository
git clone https://github.com/Paulmmoore3416/omnix.git
cd omnix

# Install dependencies
npm install

# Configure environment
cp .env.example .env
# Edit .env with production values
```

---

## Building for Production

### Basic Build

```bash
# Build for current platform
npm run tauri build
```

This creates optimized binaries in `src-tauri/target/release/`.

### Build Configuration

Edit `src-tauri/tauri.conf.json`:

```json
{
  "build": {
    "beforeBuildCommand": "npm run build",
    "beforeDevCommand": "npm run dev",
    "devPath": "http://localhost:5173",
    "distDir": "../build"
  },
  "package": {
    "productName": "OMNIX",
    "version": "1.0.0"
  }
}
```

### Optimization Options

**Rust Optimization** (`src-tauri/Cargo.toml`):
```toml
[profile.release]
opt-level = "z"     # Optimize for size
lto = true          # Link-time optimization
codegen-units = 1   # Better optimization
strip = true        # Strip symbols
panic = "abort"     # Smaller binary
```

**Frontend Optimization** (`vite.config.js`):
```javascript
export default {
  build: {
    minify: 'terser',
    terserOptions: {
      compress: {
        drop_console: true,
        drop_debugger: true
      }
    },
    rollupOptions: {
      output: {
        manualChunks: {
          vendor: ['svelte']
        }
      }
    }
  }
}
```

---

## Platform-Specific Builds

### Linux

#### Ubuntu/Debian (.deb)

```bash
# Install dependencies
sudo apt install libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev

# Build
npm run tauri build

# Output: src-tauri/target/release/bundle/deb/omnix_1.0.0_amd64.deb
```

**Install:**
```bash
sudo dpkg -i omnix_1.0.0_amd64.deb
```

#### Fedora/RHEL (.rpm)

```bash
# Install dependencies
sudo dnf install webkit2gtk4.1-devel \
  openssl-devel \
  curl \
  wget \
  file \
  libappindicator-gtk3-devel \
  librsvg2-devel

# Build
npm run tauri build -- --bundles rpm

# Output: src-tauri/target/release/bundle/rpm/omnix-1.0.0-1.x86_64.rpm
```

**Install:**
```bash
sudo rpm -i omnix-1.0.0-1.x86_64.rpm
```

#### AppImage (Universal)

```bash
# Build
npm run tauri build -- --bundles appimage

# Output: src-tauri/target/release/bundle/appimage/omnix_1.0.0_amd64.AppImage
```

**Run:**
```bash
chmod +x omnix_1.0.0_amd64.AppImage
./omnix_1.0.0_amd64.AppImage
```

#### Arch Linux (AUR)

Create `PKGBUILD`:
```bash
pkgname=omnix
pkgver=1.0.0
pkgrel=1
pkgdesc="Your God Mode AI Desktop Assistant"
arch=('x86_64')
url="https://github.com/Paulmmoore3416/omnix"
license=('MIT')
depends=('webkit2gtk' 'gtk3')
makedepends=('rust' 'npm')

build() {
  cd "$srcdir/$pkgname-$pkgver"
  npm install
  npm run tauri build
}

package() {
  cd "$srcdir/$pkgname-$pkgver"
  install -Dm755 "src-tauri/target/release/omnix" "$pkgdir/usr/bin/omnix"
}
```

### macOS

#### Requirements

```bash
# Install Xcode Command Line Tools
xcode-select --install

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### Build .app Bundle

```bash
# Build
npm run tauri build

# Output: src-tauri/target/release/bundle/macos/OMNIX.app
```

#### Build .dmg Installer

```bash
# Build with DMG
npm run tauri build -- --bundles dmg

# Output: src-tauri/target/release/bundle/dmg/OMNIX_1.0.0_x64.dmg
```

#### Code Signing (for distribution)

```bash
# Sign the app
codesign --force --deep --sign "Developer ID Application: Your Name" \
  src-tauri/target/release/bundle/macos/OMNIX.app

# Verify signature
codesign --verify --verbose src-tauri/target/release/bundle/macos/OMNIX.app

# Notarize (required for macOS 10.15+)
xcrun notarytool submit OMNIX_1.0.0_x64.dmg \
  --apple-id "your@email.com" \
  --password "app-specific-password" \
  --team-id "TEAM_ID"
```

#### Universal Binary (Intel + Apple Silicon)

```bash
# Add targets
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# Build universal binary
npm run tauri build -- --target universal-apple-darwin
```

### Windows

#### Requirements

```powershell
# Install Rust
# Download from: https://rustup.rs/

# Install WebView2
# Download from: https://developer.microsoft.com/microsoft-edge/webview2/
```

#### Build .exe Installer

```powershell
# Build
npm run tauri build

# Output: src-tauri\target\release\bundle\msi\OMNIX_1.0.0_x64_en-US.msi
```

#### Build .msi Installer

```powershell
# Install WiX Toolset
# Download from: https://wixtoolset.org/

# Build MSI
npm run tauri build -- --bundles msi

# Output: src-tauri\target\release\bundle\msi\OMNIX_1.0.0_x64_en-US.msi
```

#### Code Signing

```powershell
# Sign the executable
signtool sign /f certificate.pfx /p password /t http://timestamp.digicert.com `
  src-tauri\target\release\omnix.exe

# Verify signature
signtool verify /pa src-tauri\target\release\omnix.exe
```

---

## Distribution

### GitHub Releases

#### Manual Release

1. **Build for all platforms**
2. **Create release on GitHub**
3. **Upload binaries**:
   - `omnix_1.0.0_amd64.deb` (Linux Debian/Ubuntu)
   - `omnix-1.0.0-1.x86_64.rpm` (Linux Fedora/RHEL)
   - `omnix_1.0.0_amd64.AppImage` (Linux Universal)
   - `OMNIX_1.0.0_x64.dmg` (macOS)
   - `OMNIX_1.0.0_x64_en-US.msi` (Windows)

#### Automated Release (GitHub Actions)

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    strategy:
      matrix:
        platform: [ubuntu-latest, macos-latest, windows-latest]
    
    runs-on: ${{ matrix.platform }}
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Node
        uses: actions/setup-node@v3
        with:
          node-version: 18
      
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Install dependencies (Ubuntu)
        if: matrix.platform == 'ubuntu-latest'
        run: |
          sudo apt update
          sudo apt install -y libwebkit2gtk-4.1-dev \
            build-essential curl wget file \
            libssl-dev libayatana-appindicator3-dev librsvg2-dev
      
      - name: Install dependencies
        run: npm install
      
      - name: Build
        run: npm run tauri build
      
      - name: Upload Release Assets
        uses: softprops/action-gh-release@v1
        with:
          files: |
            src-tauri/target/release/bundle/**/*
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

### Package Managers

#### Homebrew (macOS)

Create `omnix.rb`:
```ruby
class Omnix < Formula
  desc "Your God Mode AI Desktop Assistant"
  homepage "https://github.com/Paulmmoore3416/omnix"
  url "https://github.com/Paulmmoore3416/omnix/archive/v1.0.0.tar.gz"
  sha256 "..."
  license "MIT"

  depends_on "node"
  depends_on "rust"

  def install
    system "npm", "install"
    system "npm", "run", "tauri", "build"
    bin.install "src-tauri/target/release/omnix"
  end

  test do
    system "#{bin}/omnix", "--version"
  end
end
```

**Install:**
```bash
brew tap Paulmmoore3416/omnix
brew install omnix
```

#### Chocolatey (Windows)

Create `omnix.nuspec`:
```xml
<?xml version="1.0"?>
<package>
  <metadata>
    <id>omnix</id>
    <version>1.0.0</version>
    <title>OMNIX</title>
    <authors>Paul Moore</authors>
    <description>Your God Mode AI Desktop Assistant</description>
    <projectUrl>https://github.com/Paulmmoore3416/omnix</projectUrl>
    <licenseUrl>https://github.com/Paulmmoore3416/omnix/blob/main/LICENSE</licenseUrl>
    <tags>ai assistant automation</tags>
  </metadata>
</package>
```

**Install:**
```powershell
choco install omnix
```

#### Snap (Linux)

Create `snapcraft.yaml`:
```yaml
name: omnix
version: '1.0.0'
summary: Your God Mode AI Desktop Assistant
description: |
  OMNIX is an AI-powered desktop assistant with system-level access.

base: core22
confinement: classic
grade: stable

apps:
  omnix:
    command: bin/omnix

parts:
  omnix:
    plugin: rust
    source: .
    build-packages:
      - libwebkit2gtk-4.1-dev
      - libssl-dev
```

**Build and publish:**
```bash
snapcraft
snapcraft upload --release=stable omnix_1.0.0_amd64.snap
```

---

## Auto-Updates

### Tauri Updater

Configure in `tauri.conf.json`:

```json
{
  "tauri": {
    "updater": {
      "active": true,
      "endpoints": [
        "https://github.com/Paulmmoore3416/omnix/releases/latest/download/latest.json"
      ],
      "dialog": true,
      "pubkey": "YOUR_PUBLIC_KEY"
    }
  }
}
```

### Generate Update Manifest

Create `latest.json`:
```json
{
  "version": "1.0.0",
  "notes": "New features and bug fixes",
  "pub_date": "2026-06-18T00:00:00Z",
  "platforms": {
    "linux-x86_64": {
      "signature": "...",
      "url": "https://github.com/Paulmmoore3416/omnix/releases/download/v1.0.0/omnix_1.0.0_amd64.AppImage"
    },
    "darwin-x86_64": {
      "signature": "...",
      "url": "https://github.com/Paulmmoore3416/omnix/releases/download/v1.0.0/OMNIX_1.0.0_x64.dmg"
    },
    "windows-x86_64": {
      "signature": "...",
      "url": "https://github.com/Paulmmoore3416/omnix/releases/download/v1.0.0/OMNIX_1.0.0_x64_en-US.msi"
    }
  }
}
```

---

## CI/CD Integration

### GitHub Actions (Complete Pipeline)

`.github/workflows/ci.yml`:

```yaml
name: CI/CD

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: 18
      - run: npm install
      - run: npm test

  build:
    needs: test
    strategy:
      matrix:
        platform: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: 18
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Install dependencies (Ubuntu)
        if: matrix.platform == 'ubuntu-latest'
        run: |
          sudo apt update
          sudo apt install -y libwebkit2gtk-4.1-dev \
            build-essential curl wget file \
            libssl-dev libayatana-appindicator3-dev librsvg2-dev
      - run: npm install
      - run: npm run tauri build
      - uses: actions/upload-artifact@v3
        with:
          name: ${{ matrix.platform }}-build
          path: src-tauri/target/release/bundle/
```

---

## Troubleshooting

### Build Fails

**Check:**
- All dependencies installed?
- Rust version 1.70+?
- Node version 18+?
- Sufficient disk space?

**Try:**
```bash
# Clean build
cargo clean
rm -rf node_modules
npm install
npm run tauri build
```

### Bundle Size Too Large

**Optimize:**
1. Enable LTO in `Cargo.toml`
2. Strip symbols
3. Use `opt-level = "z"`
4. Minify frontend assets
5. Remove unused dependencies

### Code Signing Issues

**macOS:**
- Verify Developer ID certificate
- Check Keychain Access
- Use correct team ID

**Windows:**
- Verify certificate validity
- Use correct timestamp server
- Check certificate chain

---

## Post-Deployment

### Monitoring

- Track download statistics
- Monitor crash reports
- Collect user feedback
- Analyze usage patterns

### Updates

- Follow semantic versioning
- Maintain changelog
- Test updates thoroughly
- Provide rollback mechanism

---

## Support

For deployment issues:
- 📖 [Documentation](https://github.com/Paulmmoore3416/omnix/wiki)
- 💬 [Discussions](https://github.com/Paulmmoore3416/omnix/discussions)
- 🐛 [Issues](https://github.com/Paulmmoore3416/omnix/issues)

---

**Last Updated:** June 2026  
**Version:** 1.0.0