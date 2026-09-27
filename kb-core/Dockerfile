# kb-core (OMNIX memory service) for home servers.
#
# Build from the repository root:   docker build -t omnix-kb-core kb-core
# Usually run through deploy/docker-compose.yml, which also supplies the
# bearer-token secret. kb-core refuses to listen on a non-loopback address
# without one, and inside a container it must listen on 0.0.0.0.
FROM python:3.12-slim

# pdftotext for PDF indexing (fixed argv, no shell; see sync.py).
RUN apt-get update \
 && apt-get install -y --no-install-recommends poppler-utils \
 && rm -rf /var/lib/apt/lists/* \
 && pip install --no-cache-dir "numpy>=2" \
 && useradd --system --uid 10001 --home-dir /data --shell /usr/sbin/nologin kb \
 && mkdir -p /data && chown kb:kb /data && chmod 700 /data

WORKDIR /app
COPY --chown=root:root kb_core /app/kb_core

ENV PYTHONPATH=/app \
    PYTHONUNBUFFERED=1 \
    KB_CORE_HOST=0.0.0.0 \
    KB_CORE_PORT=8100 \
    KB_CORE_DB=/data/kb.sqlite3 \
    KB_CORE_TOKEN_FILE=/run/secrets/kb_core_token

USER kb
VOLUME ["/data"]
EXPOSE 8100

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 CMD \
  python -c "import os,urllib.request as u; t=open(os.environ['KB_CORE_TOKEN_FILE']).read().strip(); \
u.urlopen(u.Request('http://127.0.0.1:8100/health', headers={'Authorization': 'Bearer ' + t}), timeout=4)"

CMD ["python", "-m", "kb_core", "serve"]
