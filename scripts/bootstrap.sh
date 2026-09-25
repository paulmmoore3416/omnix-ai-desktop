#!/usr/bin/env bash
# =============================================================================
# OMNIX one-shot bootstrap for a fresh Linux machine (Ubuntu/Debian first-class).
#
#   git clone https://github.com/paulmmoore3416/omnix-ai-desktop.git
#   cd omnix-ai-desktop && ./scripts/bootstrap.sh
#
# Installs and configures everything OMNIX needs, then builds and installs the
# app. Safe to re-run: every step checks before it acts, and existing user
# settings are only filled in, never overwritten.
#
# What it sets up:
#   1. System packages (Tauri/WebKitGTK build deps, audio, jq, python venv)
#   2. Rust (rustup, stable >= 1.95) and Node.js 24
#   3. npm dependencies (npm ci) and a Rust dependency prefetch
#   4. Ollama (systemd service) + chat models sized to the detected GPUs
#   5. Speaches (faster-whisper STT) in Docker with NVIDIA GPU access
#   6. Piper TTS in a private venv + a downloaded voice
#   7. ~/.config/omnix/settings.json seeded with the above
#   8. Release build of OMNIX installed as a .deb (desktop entry + `omnix` binary)
#   9. scripts/doctor.sh health check
#
# NVIDIA drivers are NOT installed automatically (a driver install needs a
# reboot and can break a working desktop). If `nvidia-smi` is missing the
# script stops and says what to run.
#
# Options (or set the matching environment variable):
#   --services-only   Set up Ollama/Speaches/Piper only; skip the app build.
#   --no-services     Build/install the app only; skip Ollama/Speaches/Piper.
#   --lan             Expose Ollama (11434) and Speaches (8000) on the LAN so
#                     OMNIX on other machines can use this server. Default is
#                     127.0.0.1 only.
#   --cpu             Allow running without an NVIDIA GPU (CPU Whisper image).
#   --yes             Non-interactive (assume yes for apt prompts).
#   -h | --help       Show this help.
#
# Tunables (environment variables):
#   OMNIX_CHAT_MODEL     default chat model            (default: qwen3:8b)
#   OMNIX_EXTRA_MODELS   extra Ollama models to pull   (default: "qwen3:14b")
#   OMNIX_STT_MODEL      Speaches/faster-whisper model (default: Systran/faster-distil-whisper-large-v3)
#   OMNIX_STT_GPU        GPU index for Speaches        (default: the GPU with the least VRAM)
#   OMNIX_PIPER_VOICE    Piper voice id                (default: en_US-lessac-medium)
#   OMNIX_DATA_DIR       voices/venvs location         (default: ~/.local/share/omnix)
# =============================================================================
set -Eeuo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="$REPO_DIR/logs"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/bootstrap-$(date +%Y%m%d-%H%M%S).log"
# Mirror everything to a log file so a failed unattended run can be diagnosed.
exec > >(tee -a "$LOG_FILE") 2>&1

SERVICES=1
APP=1
LAN=0
ALLOW_CPU=0
APT_YES=""
for arg in "$@"; do
  case "$arg" in
    --services-only) APP=0 ;;
    --no-services) SERVICES=0 ;;
    --lan) LAN=1 ;;
    --cpu) ALLOW_CPU=1 ;;
    --yes|-y) APT_YES="-y" ;;
    -h|--help) sed -n '2,50p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "Unknown option: $arg (see --help)"; exit 2 ;;
  esac
done
[[ -z "$APT_YES" && ! -t 0 ]] && APT_YES="-y"   # no TTY → non-interactive

CHAT_MODEL="${OMNIX_CHAT_MODEL:-qwen3:8b}"
EXTRA_MODELS="${OMNIX_EXTRA_MODELS-qwen3:14b}"
STT_MODEL="${OMNIX_STT_MODEL:-Systran/faster-distil-whisper-large-v3}"
PIPER_VOICE="${OMNIX_PIPER_VOICE:-en_US-lessac-medium}"
DATA_DIR="${OMNIX_DATA_DIR:-$HOME/.local/share/omnix}"
SETTINGS_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/omnix/settings.json"
BIND_ADDR=127.0.0.1; [[ $LAN == 1 ]] && BIND_ADDR=0.0.0.0
MIN_RUST="1.95"
NODE_MAJOR=24

c_blue=$'\e[34m'; c_green=$'\e[32m'; c_yellow=$'\e[33m'; c_red=$'\e[31m'; c_off=$'\e[0m'
step() { echo; echo "${c_blue}==> $*${c_off}"; }
ok()   { echo "${c_green}  ✓ $*${c_off}"; }
warn() { echo "${c_yellow}  ! $*${c_off}"; }
die()  { echo "${c_red}  ✗ $*${c_off}"; echo "Log: $LOG_FILE"; exit 1; }
trap 'die "bootstrap failed at line $LINENO (exit $?)."' ERR
have() { command -v "$1" >/dev/null 2>&1; }
ver_ge() { [[ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -1)" == "$2" ]]; }

[[ $EUID -eq 0 ]] && die "Run as your normal user, not root (sudo is used only where needed)."
[[ "$(uname -s)" == Linux ]] || die "bootstrap.sh targets Linux. On macOS/Windows use scripts/setup.sh / setup.ps1."
have apt-get || die "Only apt-based distros (Ubuntu/Debian) are automated. See docs/SERVER_DEPLOYMENT.md for manual steps."
sudo -v || die "sudo is required for package installs."

echo "OMNIX bootstrap — repo: $REPO_DIR"
echo "Log: $LOG_FILE"

# --- 1. System packages ------------------------------------------------------
step "System packages"
PKGS=(build-essential curl wget file git jq ca-certificates gnupg pkg-config
      libwebkit2gtk-4.1-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
      libxdo-dev python3 python3-venv python3-pip alsa-utils)
MISSING=()
for p in "${PKGS[@]}"; do dpkg -s "$p" >/dev/null 2>&1 || MISSING+=("$p"); done
if ((${#MISSING[@]})); then
  sudo apt-get update
  sudo apt-get install $APT_YES --no-install-recommends "${MISSING[@]}"
fi
ok "system packages present"

# --- 2. GPU detection --------------------------------------------------------
step "GPU detection"
GPU_COUNT=0
if have nvidia-smi && nvidia-smi >/dev/null 2>&1; then
  mapfile -t GPUS < <(nvidia-smi --query-gpu=index,name,memory.total --format=csv,noheader,nounits)
  GPU_COUNT=${#GPUS[@]}
  for g in "${GPUS[@]}"; do ok "GPU $g MiB"; done
  # Default Speaches to the smallest GPU so the larger one stays free for the LLM.
  STT_GPU="${OMNIX_STT_GPU:-$(printf '%s\n' "${GPUS[@]}" | sort -t, -k3 -n | head -1 | cut -d, -f1 | tr -d ' ')}"
else
  if [[ $ALLOW_CPU == 1 || $SERVICES == 0 ]]; then
    warn "no working NVIDIA driver; continuing on CPU"
  else
    die "nvidia-smi not found/working. Install the driver first:  sudo ubuntu-drivers install && sudo reboot
       then re-run this script (or pass --cpu to run Whisper on the CPU)."
  fi
fi

# --- 3. Rust -----------------------------------------------------------------
if [[ $APP == 1 ]]; then
  step "Rust toolchain"
  # shellcheck disable=SC1091
  [[ -f "$HOME/.cargo/env" ]] && . "$HOME/.cargo/env"
  if ! have rustup; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
  fi
  RUST_VER="$(rustc --version 2>/dev/null | awk '{print $2}' || echo 0)"
  ver_ge "$RUST_VER" "$MIN_RUST" || rustup update stable
  rustup default stable >/dev/null
  ok "$(rustc --version)"

  # --- 4. Node.js ------------------------------------------------------------
  step "Node.js $NODE_MAJOR"
  CUR_NODE="$(node -v 2>/dev/null | sed 's/^v//' | cut -d. -f1 || echo 0)"
  if [[ -z "$CUR_NODE" || "$CUR_NODE" -lt 22 ]]; then
    # NodeSource apt repo (signed); gives a system-wide node/npm.
    curl -fsSL "https://deb.nodesource.com/setup_${NODE_MAJOR}.x" -o /tmp/nodesource_setup.sh
    sudo -E bash /tmp/nodesource_setup.sh
    sudo apt-get install $APT_YES nodejs
    rm -f /tmp/nodesource_setup.sh
  fi
  ok "node $(node -v), npm $(npm -v)"

  step "npm + cargo dependencies"
  (cd "$REPO_DIR" && npm ci --no-audit --no-fund)
  (cd "$REPO_DIR/src-tauri" && cargo fetch --locked)
  ok "dependencies installed"
fi

if [[ $SERVICES == 1 ]]; then
  # --- 5. Ollama -------------------------------------------------------------
  step "Ollama"
  if ! have ollama; then
    curl -fsSL https://ollama.com/install.sh -o /tmp/ollama_install.sh
    sh /tmp/ollama_install.sh
    rm -f /tmp/ollama_install.sh
  fi
  if systemctl list-unit-files ollama.service >/dev/null 2>&1; then
    # Drop-in: bind address, flash attention (cuts KV-cache VRAM), keep the
    # model resident so the first reply after idle is not a cold load.
    sudo mkdir -p /etc/systemd/system/ollama.service.d
    sudo tee /etc/systemd/system/ollama.service.d/omnix.conf >/dev/null <<EOF
# Managed by omnix-ai-desktop/scripts/bootstrap.sh
[Service]
Environment="OLLAMA_HOST=${BIND_ADDR}:11434"
Environment="OLLAMA_FLASH_ATTENTION=1"
Environment="OLLAMA_KEEP_ALIVE=30m"
EOF
    sudo systemctl daemon-reload
    sudo systemctl enable --now ollama
    sudo systemctl restart ollama
  fi
  for _ in $(seq 1 30); do curl -fs http://127.0.0.1:11434/api/version >/dev/null && break; sleep 1; done
  curl -fs http://127.0.0.1:11434/api/version >/dev/null || die "Ollama is not answering on :11434 (journalctl -u ollama)"
  ok "ollama $(curl -fs http://127.0.0.1:11434/api/version | jq -r .version)"
  for m in $CHAT_MODEL $EXTRA_MODELS; do
    if ollama list | awk 'NR>1{print $1}' | grep -qx "$m"; then ok "model $m present"
    else echo "  pulling $m ..."; ollama pull "$m"; ok "model $m pulled"; fi
  done

  # --- 6. Docker + NVIDIA container toolkit + Speaches -----------------------
  step "Docker"
  if ! have docker; then
    sudo apt-get install $APT_YES docker.io docker-compose-v2 || sudo apt-get install $APT_YES docker.io
  fi
  sudo systemctl enable --now docker
  DOCKER="docker"; docker info >/dev/null 2>&1 || DOCKER="sudo docker"
  if ! id -nG "$USER" | grep -qw docker; then
    sudo usermod -aG docker "$USER"
    warn "added $USER to the docker group (takes effect at next login; using sudo for now)"
  fi
  ok "$($DOCKER --version)"

  GPU_ARGS=()
  SPEACHES_IMAGE="ghcr.io/speaches-ai/speaches:latest-cpu"
  if ((GPU_COUNT > 0)); then
    if ! have nvidia-ctk; then
      step "NVIDIA container toolkit"
      curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey \
        | sudo gpg --dearmor --yes -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
      curl -fsSL https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list \
        | sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' \
        | sudo tee /etc/apt/sources.list.d/nvidia-container-toolkit.list >/dev/null
      sudo apt-get update
      sudo apt-get install $APT_YES nvidia-container-toolkit
    fi
    sudo nvidia-ctk runtime configure --runtime=docker >/dev/null
    sudo systemctl restart docker
    ok "nvidia-container-toolkit configured"
    SPEACHES_IMAGE="ghcr.io/speaches-ai/speaches:latest-cuda"
    GPU_ARGS=(--gpus "device=${STT_GPU}")
  fi

  step "Speaches (speech-to-text) on ${BIND_ADDR}:8000 ${GPU_ARGS[*]:-(CPU)}"
  if $DOCKER ps -a --format '{{.Names}}' | grep -qx omnix-speaches; then
    $DOCKER rm -f omnix-speaches >/dev/null   # recreate so flags always match this run
  fi
  $DOCKER pull -q "$SPEACHES_IMAGE" >/dev/null
  $DOCKER run -d --name omnix-speaches --restart unless-stopped \
    -p "${BIND_ADDR}:8000:8000" \
    -v omnix-hf-cache:/home/ubuntu/.cache/huggingface/hub \
    -e STT_MODEL_TTL=-1 \
    "${GPU_ARGS[@]}" "$SPEACHES_IMAGE" >/dev/null
  for _ in $(seq 1 90); do curl -fs http://127.0.0.1:8000/health >/dev/null && break; sleep 2; done
  curl -fs http://127.0.0.1:8000/health >/dev/null || die "Speaches did not become healthy ($DOCKER logs omnix-speaches)"
  echo "  downloading STT model $STT_MODEL (first run only) ..."
  code=$(curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:8000/v1/models/${STT_MODEL}")
  [[ "$code" == 200 || "$code" == 201 ]] || die "Speaches could not download $STT_MODEL (HTTP $code)"
  ok "speaches healthy, model $STT_MODEL ready"

  # --- 7. Piper TTS ----------------------------------------------------------
  step "Piper text-to-speech"
  mkdir -p "$DATA_DIR/voices" "$HOME/.local/bin"
  [[ -x "$DATA_DIR/piper-venv/bin/piper" ]] || {
    python3 -m venv "$DATA_DIR/piper-venv"
    "$DATA_DIR/piper-venv/bin/pip" install -q --upgrade pip piper-tts
  }
  VOICE_ONNX="$DATA_DIR/voices/${PIPER_VOICE}.onnx"
  if [[ ! -s "$VOICE_ONNX" || ! -s "${VOICE_ONNX}.json" ]]; then
    "$DATA_DIR/piper-venv/bin/python" -m piper.download_voices --force-redownload \
      --download-dir "$DATA_DIR/voices" "$PIPER_VOICE"
  fi
  ln -sf "$DATA_DIR/piper-venv/bin/piper" "$HOME/.local/bin/piper"
  # Smoke test: synthesize one line to a temp WAV.
  tmpwav="$(mktemp --suffix .wav)"
  echo "OMNIX voice check." | "$DATA_DIR/piper-venv/bin/piper" --model "$VOICE_ONNX" --output_file "$tmpwav" 2>/dev/null
  [[ -s "$tmpwav" ]] || die "Piper produced no audio"
  rm -f "$tmpwav"
  ok "piper + voice $PIPER_VOICE working"

  # --- 8. Seed settings.json -------------------------------------------------
  step "OMNIX settings ($SETTINGS_FILE)"
  mkdir -p "$(dirname "$SETTINGS_FILE")"
  [[ -s "$SETTINGS_FILE" ]] || echo '{"schema_version": 2}' > "$SETTINGS_FILE"
  cp "$SETTINGS_FILE" "$SETTINGS_FILE.bak-$(date +%Y%m%d%H%M%S)"
  # Fill in only empty/default values: never clobber choices the user made in
  # the app. Paths are absolute because OMNIX runs Piper from its cache dir,
  # where a bare voice name would not resolve.
  tmp="$(mktemp)"
  jq --arg model "$CHAT_MODEL" \
     --arg stt "http://127.0.0.1:8000" \
     --arg sttmodel "$STT_MODEL" \
     --arg piper "$DATA_DIR/piper-venv/bin/piper" \
     --arg voice "$VOICE_ONNX" '
    def unset(v): (v == null) or (v == "");
    .schema_version //= 2
    | .ai.provider //= "ollama"
    | .ai.ollama_host //= "http://localhost:11434"
    | (if unset(.ai.ollama_model) then .ai.ollama_model = $model else . end)
    | (if .voice.enabled == null then .voice.enabled = true else . end)
    | (if unset(.voice.stt_url) then .voice.stt_url = $stt else . end)
    | (if unset(.voice.whisper_model) or .voice.whisper_model == "base" then .voice.whisper_model = $sttmodel else . end)
    | (if unset(.voice.piper_path) or .voice.piper_path == "piper" then .voice.piper_path = $piper else . end)
    | (if unset(.voice.tts_voice) or (.voice.tts_voice | test("/") | not) then .voice.tts_voice = $voice else . end)
  ' "$SETTINGS_FILE" > "$tmp"
  install -m 600 "$tmp" "$SETTINGS_FILE"
  rm -f "$tmp"
  ok "settings seeded (backup kept next to the file)"
fi

# --- 9. Build + install the app ---------------------------------------------
if [[ $APP == 1 ]]; then
  step "Building OMNIX (release .deb) — this takes a few minutes the first time"
  (cd "$REPO_DIR" && npm run tauri build -- --bundles deb)
  DEB="$(ls -t "$REPO_DIR"/src-tauri/target/release/bundle/deb/*.deb | head -1)"
  sudo apt-get install $APT_YES --reinstall "$DEB"
  ok "installed $(basename "$DEB") → run 'omnix' or use the OMNIX app launcher"
fi

# --- 10. Health check --------------------------------------------------------
step "Health check"
"$REPO_DIR/scripts/doctor.sh" || warn "doctor reported problems (see above)"

echo
echo "${c_green}OMNIX bootstrap finished.${c_off}  Log: $LOG_FILE"
[[ $LAN == 1 ]] && echo "Ollama and Speaches are listening on the LAN. Restrict ports 11434/8000 to trusted hosts (ufw)."
echo "Start OMNIX from the app menu or run: omnix"
