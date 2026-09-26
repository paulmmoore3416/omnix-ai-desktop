"""Service configuration, read once from environment variables."""

from __future__ import annotations

import ipaddress
import os
from dataclasses import dataclass, field
from pathlib import Path


def _env(name: str, default: str) -> str:
    return os.environ.get(name, default).strip() or default


def _float(name: str, default: float) -> float:
    try:
        return float(os.environ.get(name, default))
    except ValueError:
        return default


def parse_watch(spec: str) -> list[tuple[str | None, Path]]:
    """``/a:/b`` or ``notes=/a:work=/b`` → [(collection, path), …]."""
    out: list[tuple[str | None, Path]] = []
    for entry in spec.split(os.pathsep):
        entry = entry.strip()
        if not entry:
            continue
        coll, _, path = entry.partition("=") if "=" in entry and not entry.startswith(("/", "~")) else ("", "", entry)
        out.append((coll.strip().lower() or None, Path(path or entry).expanduser().resolve()))
    return out


def default_data_dir() -> Path:
    base = os.environ.get("XDG_DATA_HOME") or str(Path.home() / ".local" / "share")
    return Path(base) / "omnix" / "kb-core"


@dataclass
class Config:
    """All tunables. Environment variable names are given in ``from_env``."""

    host: str = "127.0.0.1"
    port: int = 8100
    db_path: Path = field(default_factory=lambda: default_data_dir() / "kb.sqlite3")
    ollama: str = "http://127.0.0.1:11434"
    embed_model: str = "nomic-embed-text"
    # Chat model used for fact extraction and contradiction checks. Empty
    # disables both (the endpoints answer 501). The installer fills this in
    # from OMNIX's configured model; nothing here hardcodes one.
    llm_model: str = ""
    # The owner's name ("Paul"). Questions are first person, memories third
    # person; knowing the name lets retrieval bridge the two, and extracted
    # memories use it. Empty = "the user".
    user_name: str = ""
    token: str | None = None
    allowed_hosts: frozenset[str] = frozenset()
    # (collection or None, folder) pairs; `name=/path` files a folder into a collection.
    watch: tuple[tuple[str | None, Path], ...] = ()
    watch_interval: float = 120.0
    # Cosine thresholds for memory consolidation (model dependent; the
    # defaults suit nomic-embed-text). Embeddings cannot tell agreement from
    # contradiction ("prefers morning meetings" vs "prefers afternoon
    # meetings" scores 0.95, as high as a paraphrase), so vectors alone only
    # merge near-verbatim restatements; everything in between is linked and
    # judged by the LLM (duplicate / obsolete / compatible).
    duplicate_threshold: float = 0.985
    related_threshold: float = 0.75
    # Recency half-life for memory activation, in days.
    half_life_days: float = 45.0
    # In-memory vector precision: float32, or float16 for half the RAM.
    vector_dtype: str = "float32"
    # Also index source code and config files in watched folders.
    index_code: bool = True
    # Folder with an ONNX NLI cross-encoder (second duplicate/contradiction
    # judge next to the LLM). None = off.
    nli_model: Path | None = None
    # SQLCipher key (64 hex chars) when the database is encrypted at rest.
    # Never read from the environment: KB_CORE_ENCRYPTION=keyring makes
    # from_env fetch it from the OS keyring (see crypto.py).
    db_key: str | None = field(default=None, repr=False)

    @classmethod
    def from_env(cls) -> "Config":
        token = None
        token_file = os.environ.get("KB_CORE_TOKEN_FILE", "").strip()
        if token_file:
            token = Path(token_file).expanduser().read_text(encoding="utf-8").strip() or None
        watch = tuple(parse_watch(os.environ.get("KB_CORE_WATCH", "")))
        nli = os.environ.get("KB_CORE_NLI_MODEL", "").strip()
        enc = os.environ.get("KB_CORE_ENCRYPTION", "").strip().lower()
        db_key = None
        if enc == "keyring":
            from .crypto import load_key

            db_key = load_key()
        elif enc not in {"", "off", "none"}:
            raise SystemExit(f"KB_CORE_ENCRYPTION must be 'keyring' or empty, not {enc!r}")
        hosts = frozenset(
            h.strip().lower() for h in os.environ.get("KB_CORE_ALLOWED_HOSTS", "").split(",") if h.strip()
        )
        return cls(
            host=_env("KB_CORE_HOST", cls.host),
            port=int(_env("KB_CORE_PORT", str(cls.port))),
            db_path=Path(_env("KB_CORE_DB", str(default_data_dir() / "kb.sqlite3"))).expanduser(),
            ollama=_env("KB_CORE_OLLAMA", cls.ollama).rstrip("/"),
            embed_model=_env("KB_CORE_EMBED_MODEL", cls.embed_model),
            llm_model=os.environ.get("KB_CORE_LLM_MODEL", "").strip(),
            user_name=os.environ.get("KB_CORE_USER_NAME", "").strip()[:64],
            token=token,
            allowed_hosts=hosts,
            watch=watch,
            watch_interval=max(10.0, _float("KB_CORE_WATCH_INTERVAL", cls.watch_interval)),
            duplicate_threshold=_float("KB_CORE_DUPLICATE_THRESHOLD", cls.duplicate_threshold),
            related_threshold=_float("KB_CORE_RELATED_THRESHOLD", cls.related_threshold),
            half_life_days=max(1.0, _float("KB_CORE_HALF_LIFE_DAYS", cls.half_life_days)),
            vector_dtype=_env("KB_CORE_VECTOR_DTYPE", cls.vector_dtype),
            index_code=_env("KB_CORE_INDEX_CODE", "1").lower() not in {"0", "false", "no", "off"},
            nli_model=Path(nli).expanduser() if nli else None,
            db_key=db_key,
        )

    def is_loopback(self) -> bool:
        if self.host == "localhost":
            return True
        try:
            return ipaddress.ip_address(self.host).is_loopback
        except ValueError:
            return False

    def validate(self) -> None:
        # The API has no user accounts: anyone who can reach the port can read
        # every memory. Off-loopback binds therefore require a bearer token.
        if not self.is_loopback() and not self.token:
            raise SystemExit(
                f"refusing to bind {self.host}: set KB_CORE_TOKEN_FILE to require a bearer token "
                "when listening on a non-loopback address"
            )
