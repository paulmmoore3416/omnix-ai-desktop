"""Encryption at rest: SQLCipher, with the key in the OS keyring.

SQLCipher encrypts the whole database file page by page (AES-256), so FTS5,
indexes and the WAL keep working unchanged; encrypting individual columns
would break keyword search. The key is 32 random bytes, stored hex-encoded in
the OS keyring (Secret Service on Linux, Keychain on macOS, Credential
Manager on Windows) under ``KEYRING_SERVICE``/``KEYRING_USER``. It is never
read from the environment or a file and never logged.

Needs ``sqlcipher3`` (``sqlcipher3-binary`` wheel) and ``keyring``; the
installer adds both to the venv with ``setup-memory.sh --encrypt``. Plain
(unencrypted) databases need neither.

What this protects: a copied disk, a stolen backup of ``memory.db``, another
local user. What it does not: a process running as the same user while the
keyring is unlocked (it can ask the keyring, or the running service, for the
data), and text already written to swap or older unencrypted backups.
"""

from __future__ import annotations

import os
import re
import secrets
from pathlib import Path
from typing import Any

KEYRING_SERVICE = "omnix-kb-core"
KEYRING_USER = "database-key"
SQLITE_MAGIC = b"SQLite format 3\x00"
_KEY_RE = re.compile(r"^[0-9a-f]{64}$")


def _keyring() -> Any:
    try:
        import keyring
    except ImportError:
        raise SystemExit("encryption needs the keyring package (re-run ./scripts/setup-memory.sh --encrypt)") from None
    return keyring


def cipher_module() -> Any:
    """The SQLCipher DB-API module (same interface as ``sqlite3``)."""
    try:
        from sqlcipher3 import dbapi2  # type: ignore[import-not-found]
    except ImportError:
        raise SystemExit("encryption needs sqlcipher3 (re-run ./scripts/setup-memory.sh --encrypt)") from None
    return dbapi2


def load_key() -> str:
    """The database key from the OS keyring. Exits if it is missing or
    malformed: starting on an encrypted file without its key would only fail
    later and less clearly."""
    try:
        key = _keyring().get_password(KEYRING_SERVICE, KEYRING_USER)
    except Exception as e:  # noqa: BLE001 - keyring backends raise their own types
        raise SystemExit(f"cannot read the kb-core key from the OS keyring (is it unlocked?): {e}") from None
    if not key:
        raise SystemExit(f"no kb-core key in the OS keyring ({KEYRING_SERVICE}/{KEYRING_USER}): "
                         "run 'kb-core encrypt' first, or remove KB_CORE_ENCRYPTION from kb-core.env")
    if not _KEY_RE.match(key):
        raise SystemExit(f"the kb-core key in the OS keyring ({KEYRING_SERVICE}/{KEYRING_USER}) is malformed")
    return key


def create_key() -> str:
    """Return the keyring's key, creating one first if there is none.

    An existing key is reused, never replaced: it may be the only way to
    open an encrypted copy of the database (a backup, say)."""
    kr = _keyring()
    try:
        key = kr.get_password(KEYRING_SERVICE, KEYRING_USER)
        if key and _KEY_RE.match(key):
            return key
        if key:
            raise SystemExit(f"the kb-core key in the OS keyring ({KEYRING_SERVICE}/{KEYRING_USER}) is malformed; "
                             "remove it by hand if no encrypted database needs it")
        key = secrets.token_hex(32)
        kr.set_password(KEYRING_SERVICE, KEYRING_USER, key)
    except SystemExit:
        raise
    except Exception as e:  # noqa: BLE001
        raise SystemExit(f"cannot store the kb-core key in the OS keyring (is it unlocked?): {e}") from None
    return key


def is_plain_sqlite(path: Path) -> bool:
    """True for an ordinary SQLite file. SQLCipher files start with random
    salt instead of the SQLite header."""
    with open(path, "rb") as f:
        return f.read(len(SQLITE_MAGIC)) == SQLITE_MAGIC


def _key_pragma(key: str | None) -> str:
    # Raw 256-bit key: x'…' skips SQLCipher's PBKDF2 (the key is already
    # random, stretching it adds nothing). Hex only, so safe to interpolate.
    if key is None:
        return "''"
    if not _KEY_RE.match(key):
        raise ValueError("key must be 64 hex characters")
    return f"\"x'{key}'\""


def convert(path: Path, src_key: str | None, dst_key: str | None) -> tuple[int, int, int]:
    """Rewrite ``path`` from ``src_key`` to ``dst_key`` (None = plain SQLite)
    with ``sqlcipher_export``. The service must be stopped.

    Writes a private temporary file next to the database, checks it opens
    with the new key and passes ``integrity_check``, then atomically
    replaces the original. On any failure the original is left as it was.
    Returns ``(items, documents, links)`` counted from the new file.
    """
    dbapi = cipher_module()
    path = Path(path)
    tmp = path.with_name(path.name + ".convert-tmp")
    tmp.unlink(missing_ok=True)
    os.close(os.open(tmp, os.O_RDWR | os.O_CREAT | os.O_EXCL, 0o600))
    try:
        src = dbapi.connect(str(path), isolation_level=None)
        try:
            if src_key is not None:  # a plain source takes no key at all
                src.execute(f"PRAGMA key = {_key_pragma(src_key)}")
            src.execute("SELECT count(*) FROM sqlite_master").fetchone()  # fails fast on a wrong key
            src.execute("PRAGMA wal_checkpoint(TRUNCATE)")
            version = src.execute("PRAGMA user_version").fetchone()[0]
            src.execute(f"ATTACH DATABASE ? AS out KEY {_key_pragma(dst_key)}", (str(tmp),))
            src.execute("SELECT sqlcipher_export('out')")
            src.execute(f"PRAGMA out.user_version = {int(version)}")
            src.execute("DETACH DATABASE out")
        finally:
            src.close()
        out = dbapi.connect(str(tmp))
        try:
            if dst_key is not None:
                out.execute(f"PRAGMA key = {_key_pragma(dst_key)}")
            check = out.execute("PRAGMA integrity_check").fetchone()[0]
            if check != "ok":
                raise SystemExit(f"converted copy failed integrity_check ({check}); {path} is unchanged")
            counts = tuple(out.execute(f"SELECT count(*) FROM {t}").fetchone()[0]
                           for t in ("items", "documents", "links"))
        finally:
            out.close()
        with open(tmp, "rb") as f:
            os.fsync(f.fileno())
        os.replace(tmp, path)
    except BaseException:
        tmp.unlink(missing_ok=True)
        raise
    # Checkpointed and closed above: these are empty leftovers of the old file.
    for suffix in ("-wal", "-shm"):
        path.with_name(path.name + suffix).unlink(missing_ok=True)
    return counts  # type: ignore[return-value]
