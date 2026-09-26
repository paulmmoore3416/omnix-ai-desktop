#!/usr/bin/env bash
# =============================================================================
# OMNIX long-term memory: install/update the kb-core service, point OMNIX at
# it, and optionally add folders of notes that it keeps indexed live.
#
#   ./scripts/setup-memory.sh                        # install/update + configure
#   ./scripts/setup-memory.sh ~/notes ~/kb-repo      # … and watch these folders
#   ./scripts/setup-memory.sh --lan                  # also serve other machines (token)
#
# What it does (idempotent; no sudo):
#   1. pulls the embedding model into Ollama (default nomic-embed-text)
#   2. installs kb-core/ to ~/.local/share/omnix/kb-core/app with a private
#      venv (numpy for fast vector search; falls back to pure Python)
#   3. writes ~/.config/omnix/kb-core.env (kept on re-runs; folders appended)
#   4. installs + starts the systemd *user* unit omnix-kb-core.service
#   5. migrates memories from the kb-core 1.x database once, if present
#   6. sets memory.backend_url in ~/.config/omnix/settings.json (backup first)
#   7. installs a `kb-core` command in ~/.local/bin
#
# Options:
#   --lan        bind 0.0.0.0 and require a bearer token (generated once into
#                ~/.config/omnix/kb-core.token, mode 600). Enter the token in
#                OMNIX → Settings → Memory on the other machines.
#   --no-llm     don't use a chat model (disables fact capture and the
#                duplicate/contradiction judge)
#
# Environment overrides:
#   OMNIX_KB_PORT         kb-core port             (default 8100)
#   OMNIX_EMBED_MODEL     Ollama embedding model   (default nomic-embed-text)
#   OMNIX_KB_LLM_MODEL    chat model for kb-core   (default: OMNIX's ai.ollama_model)
#   OMNIX_DATA_DIR        data location            (default ~/.local/share/omnix)
# =============================================================================
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PORT="${OMNIX_KB_PORT:-8100}"
EMBED_MODEL="${OMNIX_EMBED_MODEL:-nomic-embed-text}"
DATA_DIR="${OMNIX_DATA_DIR:-$HOME/.local/share/omnix}"
KB_DIR="$DATA_DIR/kb-core"
CONF_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/omnix"
SETTINGS_FILE="$CONF_DIR/settings.json"
ENV_FILE="$CONF_DIR/kb-core.env"
TOKEN_FILE="$CONF_DIR/kb-core.token"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
URL="http://127.0.0.1:$PORT"

LAN=0
USE_LLM=1
FOLDERS=()
for arg in "$@"; do
  case "$arg" in
    --lan) LAN=1 ;;
    --no-llm) USE_LLM=0 ;;
    -h|--help) sed -n '2,36p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "Unknown option: $arg (see --help)"; exit 2 ;;
    *) FOLDERS+=("$arg") ;;
  esac
done

step() { echo; echo "==> $*"; }
ok() { echo "  ok  $*"; }
die() { echo "ERROR: $*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

have python3 || die "python3 is required"
have jq || die "jq is required (sudo apt install jq)"
have curl || die "curl is required"
have systemctl || die "systemd is required"
python3 -c 'import sys; sys.exit(sys.version_info < (3, 10))' || die "python 3.10+ is required"
python3 -c 'import sqlite3; c = sqlite3.connect(":memory:"); c.execute("create virtual table t using fts5(a)")' 2>/dev/null \
  || die "this python's sqlite3 lacks FTS5"

cfg() { [[ -s "$SETTINGS_FILE" ]] && jq -r "$1 // empty" "$SETTINGS_FILE" 2>/dev/null || true; }
OLLAMA="$(cfg .ai.ollama_host)"
OLLAMA="${OLLAMA:-http://127.0.0.1:11434}"
OLLAMA="${OLLAMA/localhost/127.0.0.1}"
LLM_MODEL=""
if [[ $USE_LLM == 1 ]]; then LLM_MODEL="${OMNIX_KB_LLM_MODEL:-$(cfg .ai.ollama_model)}"; fi

# --- 1. models --------------------------------------------------------------
step "Ollama models"
curl -fs --max-time 5 "$OLLAMA/api/version" >/dev/null || die "Ollama is not reachable at $OLLAMA (run bootstrap.sh first)"
installed() { curl -fs "$OLLAMA/api/tags" | jq -e --arg m "$1" '.models[].name | select(. == $m or . == ($m + ":latest"))' >/dev/null; }
if installed "$EMBED_MODEL"; then ok "embedding model $EMBED_MODEL"
else
  have ollama || die "ollama CLI not found; pull $EMBED_MODEL on $OLLAMA manually"
  ollama pull "$EMBED_MODEL"; ok "pulled $EMBED_MODEL"
fi
if [[ -n "$LLM_MODEL" ]]; then
  if installed "$LLM_MODEL"; then ok "chat model $LLM_MODEL (fact capture + consolidation)"
  else echo "  warn chat model $LLM_MODEL is not installed; fact capture disabled"; LLM_MODEL=""; fi
fi

# --- 2. code + venv ---------------------------------------------------------
step "kb-core service"
install -d -m 700 "$KB_DIR"
rm -rf "$KB_DIR/app.new"
install -d "$KB_DIR/app.new"
cp -r "$REPO/kb-core/kb_core" "$KB_DIR/app.new/"
find "$KB_DIR/app.new" -name __pycache__ -prune -exec rm -rf {} +
rm -rf "$KB_DIR/app" && mv "$KB_DIR/app.new" "$KB_DIR/app"
if [[ ! -x "$KB_DIR/venv/bin/python" ]]; then
  python3 -m venv "$KB_DIR/venv" || die "python3 -m venv failed (sudo apt install python3-venv)"
fi
if "$KB_DIR/venv/bin/python" -c 'import numpy' 2>/dev/null; then ok "numpy present"
elif "$KB_DIR/venv/bin/pip" install -q --disable-pip-version-check numpy; then ok "numpy installed (fast vector search)"
else echo "  warn numpy could not be installed; using the pure-Python vector search"; fi
ok "$(PYTHONPATH="$KB_DIR/app" "$KB_DIR/venv/bin/python" -m kb_core --version)"

# --- 3. configuration -------------------------------------------------------
install -d -m 700 "$CONF_DIR"
if [[ ! -s "$ENV_FILE" ]]; then
  cat >"$ENV_FILE" <<EOF
# kb-core settings (read by omnix-kb-core.service). Edit, then:
#   systemctl --user restart omnix-kb-core
KB_CORE_HOST=127.0.0.1
KB_CORE_PORT=$PORT
KB_CORE_DB=$KB_DIR/memory.db
KB_CORE_OLLAMA=$OLLAMA
KB_CORE_EMBED_MODEL=$EMBED_MODEL
# Local chat model for fact capture and the duplicate/contradiction judge (empty = off)
KB_CORE_LLM_MODEL=$LLM_MODEL
# Folders kept indexed live, separated by ':' (re-scanned every KB_CORE_WATCH_INTERVAL seconds)
KB_CORE_WATCH=
KB_CORE_WATCH_INTERVAL=120
EOF
  ok "wrote $ENV_FILE"
fi
chmod 600 "$ENV_FILE"
setenv() {  # setenv KEY VALUE: replace or append a line in the env file
  local tmp; tmp="$(mktemp "$ENV_FILE.XXXX")"
  grep -v "^$1=" "$ENV_FILE" >"$tmp" || true
  echo "$1=$2" >>"$tmp"
  install -m 600 "$tmp" "$ENV_FILE"; rm -f "$tmp"
}
getenv() { sed -n "s/^$1=//p" "$ENV_FILE" | tail -1; }
if [[ -n "$LLM_MODEL" && -z "$(getenv KB_CORE_LLM_MODEL)" ]]; then setenv KB_CORE_LLM_MODEL "$LLM_MODEL"; fi
if [[ $USE_LLM == 0 ]]; then setenv KB_CORE_LLM_MODEL ""; fi

if ! grep -q '^KB_CORE_USER_NAME=' "$ENV_FILE"; then
  # First name from the account's full name (or git), so first-person
  # questions ("my skills") match third-person memories ("Paul's skills").
  OWNER="$(getent passwd "$USER" 2>/dev/null | cut -d: -f5 | cut -d, -f1 | awk '{print $1}')"
  [[ -z "$OWNER" ]] && OWNER="$(git config --global user.name 2>/dev/null | awk '{print $1}')"
  OWNER="$(printf '%s' "$OWNER" | tr -cd '[:alnum:] .-' | cut -c1-40)"
  setenv KB_CORE_USER_NAME "$OWNER"
  ok "owner name for memories: ${OWNER:-(unknown; edit KB_CORE_USER_NAME in $ENV_FILE)}"
fi

if ((${#FOLDERS[@]})); then
  watch="$(getenv KB_CORE_WATCH)"
  for f in "${FOLDERS[@]}"; do
    d="$(cd "$f" 2>/dev/null && pwd)" || die "not a directory: $f"
    case ":$watch:" in *":$d:"*) ok "already watching $d" ;; *) watch="${watch:+$watch:}$d"; ok "watching $d" ;; esac
  done
  setenv KB_CORE_WATCH "$watch"
fi

if [[ $LAN == 1 ]]; then
  if [[ ! -s "$TOKEN_FILE" ]]; then
    (umask 077; python3 -c 'import secrets; print(secrets.token_urlsafe(32))' >"$TOKEN_FILE")
    ok "generated a bearer token in $TOKEN_FILE"
  fi
  chmod 600 "$TOKEN_FILE"
  setenv KB_CORE_HOST 0.0.0.0
  setenv KB_CORE_TOKEN_FILE "$TOKEN_FILE"
fi

# --- 4. systemd user unit ---------------------------------------------------
install -d "$UNIT_DIR"
cat >"$UNIT_DIR/omnix-kb-core.service" <<EOF
# Managed by omnix-ai-desktop/scripts/setup-memory.sh
[Unit]
Description=OMNIX kb-core long-term memory (local)
After=network-online.target

[Service]
Type=simple
EnvironmentFile=$ENV_FILE
Environment=PYTHONPATH=$KB_DIR/app
Environment=PYTHONUNBUFFERED=1
ExecStart=$KB_DIR/venv/bin/python -m kb_core serve
# The database holds personal data: everything it creates stays private.
UMask=0077
NoNewPrivileges=yes
PrivateTmp=yes
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF
systemctl --user daemon-reload
systemctl --user enable omnix-kb-core.service >/dev/null
systemctl --user restart omnix-kb-core.service
AUTH=()
if [[ -n "$(getenv KB_CORE_TOKEN_FILE)" ]]; then AUTH=(-H "Authorization: Bearer $(cat "$(getenv KB_CORE_TOKEN_FILE)")"); fi
for _ in $(seq 1 40); do
  curl -fs --max-time 2 "${AUTH[@]}" "$URL/health" >/dev/null && break
  sleep 0.5
done
curl -fs "${AUTH[@]}" "$URL/health" >/dev/null || die "kb-core did not start: journalctl --user -u omnix-kb-core -n 50"
ok "omnix-kb-core running on $(getenv KB_CORE_HOST):$PORT"

# --- 5. CLI -------------------------------------------------------------------
install -d "$HOME/.local/bin"
cat >"$HOME/.local/bin/kb-core" <<EOF
#!/bin/sh
# kb-core command line (talks to the running omnix-kb-core service)
set -a; . "$ENV_FILE"; set +a
export PYTHONPATH="$KB_DIR/app"
export KB_CORE_URL="$URL"
exec "$KB_DIR/venv/bin/python" -m kb_core "\$@"
EOF
chmod 755 "$HOME/.local/bin/kb-core"
ok "installed ~/.local/bin/kb-core (try: kb-core status)"

# --- 6. migrate kb-core 1.x memories (once) ------------------------------------
LEGACY="$KB_DIR/kb.sqlite3"
if [[ -s "$LEGACY" ]]; then
  step "Migrating kb-core 1.x memories"
  "$HOME/.local/bin/kb-core" migrate-legacy "$LEGACY"
  stamp="$(date +%Y%m%d%H%M%S)"
  for f in "$LEGACY" "$LEGACY-wal" "$LEGACY-shm"; do [[ -e "$f" ]] && mv "$f" "$f.migrated-$stamp"; done
  rm -f "$KB_DIR/kb_core.py"
  ok "old database kept as $LEGACY.migrated-$stamp"
fi

# --- 7. OMNIX settings ----------------------------------------------------------
step "OMNIX settings"
if [[ -s "$SETTINGS_FILE" ]]; then
  jq empty "$SETTINGS_FILE" 2>/dev/null || die "$SETTINGS_FILE is not valid JSON; restore a settings.json.bak-* first"
  current="$(jq -r '.memory.backend_url // empty' "$SETTINGS_FILE")"
else
  echo '{}' >"$SETTINGS_FILE"; current=""
fi
if [[ "$current" == "$URL" ]]; then
  ok "memory.backend_url already $URL"
else
  [[ -n "$current" ]] && echo "  replacing memory.backend_url $current"
  cp -p "$SETTINGS_FILE" "$SETTINGS_FILE.bak-$(date +%Y%m%d%H%M%S)"
  tmp="$(mktemp "$SETTINGS_FILE.XXXX")"
  jq --arg u "$URL" '.memory = ((.memory // {}) + {backend_url: $u})' "$SETTINGS_FILE" >"$tmp"
  mv "$tmp" "$SETTINGS_FILE"
  ok "memory.backend_url = $URL"
  if pgrep -x omnix >/dev/null; then echo "  NOTE: OMNIX is running; quit and reopen it so it picks up the new setting."; fi
fi
chmod 600 "$SETTINGS_FILE"

if [[ -n "$(getenv KB_CORE_WATCH)" ]]; then
  step "Syncing watched folders (first run embeds everything; later runs only changes)"
  "$HOME/.local/bin/kb-core" sync >/dev/null && ok "synced"
fi

step "Done"
"$HOME/.local/bin/kb-core" status
if [[ $LAN == 1 ]]; then
  echo "LAN mode: kb-core listens on 0.0.0.0:$PORT. Firewall it to trusted hosts; the token is in $TOKEN_FILE."
fi
echo "Try it in OMNIX: Knowledge view → Search, or ask the assistant about something in your notes."
