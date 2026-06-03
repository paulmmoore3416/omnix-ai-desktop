# OMNIX Setup Script for Windows
# Run this script in PowerShell as Administrator

Write-Host "🌌 OMNIX Setup Script for Windows" -ForegroundColor Cyan
Write-Host "===================================" -ForegroundColor Cyan
Write-Host ""

# Check for Node.js
try {
    $nodeVersion = node -v
    Write-Host "✅ Node.js $nodeVersion detected" -ForegroundColor Green
} catch {
    Write-Host "❌ Node.js is not installed." -ForegroundColor Red
    Write-Host "   Please install Node.js 18+ from: https://nodejs.org/" -ForegroundColor Yellow
    exit 1
}

# Check for Rust
try {
    $rustVersion = rustc --version
    Write-Host "✅ Rust detected: $rustVersion" -ForegroundColor Green
} catch {
    Write-Host "❌ Rust is not installed." -ForegroundColor Red
    Write-Host "   Installing Rust..." -ForegroundColor Yellow
    
    # Download and run rustup installer
    Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile "$env:TEMP\rustup-init.exe"
    Start-Process -FilePath "$env:TEMP\rustup-init.exe" -ArgumentList "-y" -Wait
    
    # Refresh environment variables
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
    
    Write-Host "✅ Rust installed successfully" -ForegroundColor Green
}

# Check for WebView2
Write-Host ""
Write-Host "Checking for WebView2 Runtime..." -ForegroundColor Yellow
$webview2Path = "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
if (Test-Path $webview2Path) {
    Write-Host "✅ WebView2 Runtime is installed" -ForegroundColor Green
} else {
    Write-Host "⚠️  WebView2 Runtime not detected" -ForegroundColor Yellow
    Write-Host "   Downloading WebView2 Runtime..." -ForegroundColor Yellow
    
    $webview2Url = "https://go.microsoft.com/fwlink/p/?LinkId=2124703"
    $webview2Installer = "$env:TEMP\MicrosoftEdgeWebview2Setup.exe"
    
    Invoke-WebRequest -Uri $webview2Url -OutFile $webview2Installer
    Start-Process -FilePath $webview2Installer -ArgumentList "/silent /install" -Wait
    
    Write-Host "✅ WebView2 Runtime installed" -ForegroundColor Green
}

# Install npm dependencies
Write-Host ""
Write-Host "📦 Installing npm dependencies..." -ForegroundColor Yellow
npm install

if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Failed to install npm dependencies" -ForegroundColor Red
    exit 1
}

Write-Host "✅ npm dependencies installed" -ForegroundColor Green

# Install Tauri CLI
Write-Host ""
Write-Host "🦀 Installing Tauri CLI..." -ForegroundColor Yellow
cargo install tauri-cli --version "^2.0.0"

Write-Host ""
Write-Host "✅ Setup complete!" -ForegroundColor Green
Write-Host ""
Write-Host "🚀 To start OMNIX in development mode:" -ForegroundColor Cyan
Write-Host "   npm run tauri dev" -ForegroundColor White
Write-Host ""
Write-Host "📦 To build for production:" -ForegroundColor Cyan
Write-Host "   npm run tauri build" -ForegroundColor White
Write-Host ""
Write-Host "📚 For more information, see README.md" -ForegroundColor Cyan

# Made with Bob
