#!/usr/bin/env bash
# One-time setup for deploy/docker-compose.yml: creates .env from the example
# and a random kb-core bearer token (mode 600). Idempotent: existing files are
# kept. The token is never printed; copy it into OMNIX from the file.
set -euo pipefail
cd "$(dirname "$0")"
umask 077
mkdir -p secrets notes
if [[ ! -f .env ]]; then cp .env.example .env; echo "created deploy/.env (review OMNIX_BIND and OMNIX_MODELS)"; fi
if [[ ! -s secrets/kb_core_token ]]; then
  head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n' > secrets/kb_core_token
  echo "created deploy/secrets/kb_core_token"
fi
# The directory keeps other users on this host out (700). The file itself
# must be readable by the container's non-root uid (10001): Compose mounts
# file secrets as-is, so it is 644 inside the private directory.
chmod 700 secrets
chmod 644 secrets/kb_core_token
chmod 600 .env
cat <<MSG

Next:
  docker compose up -d                       # CPU
  docker compose -f docker-compose.yml -f docker-compose.nvidia.yml up -d   # NVIDIA

Then in OMNIX on your workstation:
  Settings → AI Models → Ollama host   http://<this-server>:11434
  Settings → Voice     → STT server    http://<this-server>:8000
  Settings → Memory    → kb-core URL   http://<this-server>:8100 and paste the token
                                        from deploy/secrets/kb_core_token (stored in the OS keychain)
MSG
