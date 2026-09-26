#!/usr/bin/env bash
# =============================================================================
# OMNIX doctor: read-only health check of everything bootstrap.sh sets up.
# Prints one line per check and exits non-zero if any required check fails.
#
#   ./scripts/doctor.sh           # full check
#   ./scripts/doctor.sh --quiet   # only failures and warnings
#
# Endpoints are taken from ~/.config/omnix/settings.json when present, so this
# also validates a machine that points OMNIX at a remote Ollama/Speaches server.
# =============================================================================
set -uo pipefail

QUIET=0; [[ "${1:-}" == "--quiet" ]] && QUIET=1
SETTINGS_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/omnix/settings.json"
FAILS=0; WARNS=0
g=$'\e[32m'; y=$'\e[33m'; r=$'\e[31m'; o=$'\e[0m'
pass() { [[ $QUIET == 1 ]] || echo "${g}  PASS${o}  $*"; }
warn() { echo "${y}  WARN${o}  $*"; WARNS=$((WARNS+1)); }
fail() { echo "${r}  FAIL${o}  $*"; FAILS=$((FAILS+1)); }
have() { command -v "$1" >/dev/null 2>&1; }
# Read a settings value with jq, falling back to a default.
cfg() { [[ -s "$SETTINGS_FILE" ]] && have jq && jq -r "$1 // empty" "$SETTINGS_FILE" 2>/dev/null || true; }

echo "OMNIX doctor — $(date '+%F %T')"

# Toolchains (only needed to build from source).
if have rustc; then pass "rust $(rustc --version | awk '{print $2}')"; else warn "rust not installed (only needed to build)"; fi
if have node; then
  nv=$(node -v | sed 's/^v//' | cut -d. -f1)
  if [[ $nv -ge 22 ]]; then pass "node $(node -v)"; else fail "node $(node -v) is too old (need 22+)"; fi
else warn "node not installed (only needed to build)"; fi

# App binary.
if have omnix; then pass "omnix installed at $(command -v omnix)"
else warn "omnix binary not on PATH (dev mode: npm run tauri dev)"; fi

# GPUs.
if have nvidia-smi && nvidia-smi >/dev/null 2>&1; then
  while IFS=, read -r idx name total used; do
    pass "GPU$idx$name —${used} /${total} MiB used"
  done < <(nvidia-smi --query-gpu=index,name,memory.total,memory.used --format=csv,noheader,nounits)
else warn "no NVIDIA GPU/driver visible (models will run on CPU)"; fi

# Settings file.
if [[ -s "$SETTINGS_FILE" ]]; then
  if jq empty "$SETTINGS_FILE" 2>/dev/null; then pass "settings.json valid ($SETTINGS_FILE)"
  else fail "settings.json is not valid JSON"; fi
  perms=$(stat -c %a "$SETTINGS_FILE")
  [[ "$perms" == 600 ]] && pass "settings.json mode 600" || warn "settings.json mode $perms (expected 600)"
else warn "no settings.json yet (OMNIX will use defaults on first launch)"; fi

# Ollama.
OLLAMA="$(cfg .ai.ollama_host)"; OLLAMA="${OLLAMA:-http://localhost:11434}"
MODEL="$(cfg .ai.ollama_model)"
if ver=$(curl -fs --max-time 5 "$OLLAMA/api/version" | jq -r .version 2>/dev/null) && [[ -n "$ver" ]]; then
  pass "ollama $ver at $OLLAMA"
  models=$(curl -fs --max-time 5 "$OLLAMA/api/tags" | jq -r '.models[].name')
  if [[ -z "$MODEL" ]]; then warn "no chat model selected (Settings → AI)"
  elif grep -qx "$MODEL" <<<"$models"; then pass "chat model $MODEL installed"
  else fail "chat model $MODEL is not installed (ollama pull $MODEL)"; fi
  # Real generation round-trip (short, bounded).
  if [[ -n "$MODEL" ]] && grep -qx "$MODEL" <<<"$models"; then
    if curl -fs --max-time 120 "$OLLAMA/api/generate" \
        -d "{\"model\":\"$MODEL\",\"prompt\":\"Reply with OK\",\"stream\":false,\"options\":{\"num_predict\":8}}" \
        | jq -e '.response' >/dev/null; then pass "chat model answers"
    else fail "chat model did not answer within 120 s"; fi
  fi
else fail "ollama not reachable at $OLLAMA"; fi

# Speaches STT.
STT="$(cfg .voice.stt_url)"
if [[ -z "$STT" ]]; then warn "voice.stt_url empty — the mic button is disabled"
elif curl -fs --max-time 5 "$STT/health" >/dev/null; then
  pass "speaches healthy at $STT"
  SM="$(cfg .voice.whisper_model)"
  if curl -fs --max-time 5 "$STT/v1/models" | jq -e --arg m "$SM" '.data[] | select(.id == $m)' >/dev/null; then
    pass "STT model $SM downloaded"
  else fail "STT model '$SM' not downloaded (curl -X POST $STT/v1/models/$SM)"; fi
else fail "speaches not reachable at $STT"; fi

# Piper TTS.
PIPER="$(cfg .voice.piper_path)"; PIPER="${PIPER:-piper}"
VOICE="$(cfg .voice.tts_voice)"
if have "$PIPER" || [[ -x "$PIPER" ]]; then
  pass "piper at $PIPER"
  if [[ "$VOICE" == /* && -s "$VOICE" && -s "$VOICE.json" ]]; then pass "piper voice $(basename "$VOICE")"
  elif [[ "$VOICE" != /* ]]; then warn "tts_voice '$VOICE' is not an absolute .onnx path (Piper will not find it)"
  else fail "piper voice files missing: $VOICE(.json)"; fi
else warn "piper not found — voice output disabled"; fi

# Docker container state.
if have docker; then
  D=docker; docker info >/dev/null 2>&1 || D="sudo -n docker"
  st=$($D inspect -f '{{.State.Status}}' omnix-speaches 2>/dev/null || true)
  [[ -n "$st" ]] && { [[ "$st" == running ]] && pass "container omnix-speaches running" || fail "container omnix-speaches is $st"; }
fi

# Long-term memory (kb-core), only when configured.
KB="$(cfg .memory.backend_url)"
if [[ -n "$KB" ]]; then
  KB_AUTH=()
  KB_TOKEN_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/omnix/kb-core.token"
  [[ -r "$KB_TOKEN_FILE" ]] && KB_AUTH=(-H "Authorization: Bearer $(cat "$KB_TOKEN_FILE")")
  if kh=$(curl -fs --max-time 5 "${KB_AUTH[@]}" "$KB/health") && [[ -n "$kh" ]]; then
    pass "kb-core at $KB — $(jq -r '"\(.memories // "?") memories, \(.documents // "?") documents, \(.chunks // "?") chunks"' <<<"$kh" 2>/dev/null)"
    [[ "$(jq -r '.status // "ok"' <<<"$kh")" == degraded ]] \
      && warn "kb-core embedding model unavailable ($(jq -r '.embed_error' <<<"$kh")): search is keyword-only"
    pend=$(jq -r '.pending_embeddings // 0' <<<"$kh")
    [[ "$pend" =~ ^[0-9]+$ ]] && ((pend > 0)) && warn "kb-core has $pend items waiting for embeddings (normal right after indexing)"
    if st=$(curl -fs --max-time 5 "${KB_AUTH[@]}" "$KB/stats" 2>/dev/null); then
      llm=$(jq -r '.llm_model // empty' <<<"$st")
      [[ -n "$llm" ]] && pass "kb-core learning model $llm" || warn "kb-core has no chat model: fact capture and contradiction checks are off"
      nli=$(jq -r '.nli_model // empty' <<<"$st")
      [[ -n "$nli" ]] && pass "kb-core NLI judge $nli (merges need $(jq -r '.judge' <<<"$st"))"
      [[ "$(jq -r '.encrypted // false' <<<"$st")" == true ]] && pass "kb-core database encrypted at rest (SQLCipher)"
      nrev=$(jq -r '.needs_review // 0' <<<"$st")
      [[ "$nrev" =~ ^[0-9]+$ ]] && ((nrev > 0)) && echo "  info  kb-core: $nrev merged/replaced memories to review (Knowledge view)"
      jq -r '(.watch.errors // {}) | to_entries[] | "\(.key): \(.value)"' <<<"$st" | while read -r e; do echo "${y}  WARN${o}  kb-core watch folder $e"; done
    fi
  else fail "kb-core not reachable at $KB (systemctl --user status omnix-kb-core; journalctl --user -u omnix-kb-core -n 50)"; fi
fi

# Keychain (API keys + MCP secrets live here).
if have gnome-keyring-daemon || pgrep -x kwalletd6 >/dev/null || pgrep -x kwalletd5 >/dev/null; then pass "secret service available"
else warn "no Secret Service (gnome-keyring/kwallet): cloud API keys cannot be stored"; fi

echo
echo "Summary: $FAILS failed, $WARNS warnings"
exit $(( FAILS > 0 ))
