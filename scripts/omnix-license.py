#!/usr/bin/env python3
"""Issue and check OMNIX license keys (vendor tool, not shipped in the app).

A license key is one line:

    OMNIX1.<base64url(payload JSON)>.<base64url(Ed25519 signature)>

The signature covers the ASCII bytes ``omnix-license-v1.<payload part>`` (a
domain prefix, so the signing key can never be tricked into signing anything
else that happens to verify). The app checks it offline against the public
keys compiled into ``src-tauri/src/license.rs``; nothing phones home.

The private signing key lives only in the OS keyring (Secret Service,
Keychain or Credential Manager) under service ``omnix-license-signing``,
user ``<key id>``. It is never printed, written to a file or logged. Back it
up with your keyring's own tools (e.g. Seahorse → Passwords → Login): if it
is lost, issued licenses keep verifying but no new ones can be signed until
a new key is generated and its public half added to the app.

Needs the ``cryptography`` and ``keyring`` packages. The kb-core venv has
both:

    ~/.local/share/omnix/kb-core/venv/bin/python scripts/omnix-license.py keygen
    ~/.local/share/omnix/kb-core/venv/bin/python scripts/omnix-license.py issue \\
        --tier pro --licensee "Jane Doe" --email jane@example.com
    ... issue --tier byok --licensee "Acme" --months 1
    ... verify OMNIX1.eyJ...
"""

from __future__ import annotations

import argparse
import base64
import calendar
import datetime as dt
import json
import sys
import uuid

try:
    import keyring
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey, Ed25519PublicKey
    from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat
except ImportError as e:  # pragma: no cover - environment message
    sys.exit(f"{e}. Run with a Python that has 'cryptography' and 'keyring', e.g. "
             "~/.local/share/omnix/kb-core/venv/bin/python scripts/omnix-license.py …")

SERVICE = "omnix-license-signing"
DEFAULT_KID = "k1"
PREFIX = "OMNIX1"
DOMAIN = b"omnix-license-v1."
TIERS = ("pro", "byok", "enterprise")


def b64u(b: bytes) -> str:
    return base64.urlsafe_b64encode(b).rstrip(b"=").decode("ascii")


def unb64u(s: str) -> bytes:
    return base64.urlsafe_b64decode(s + "=" * (-len(s) % 4))


def load_private(kid: str) -> Ed25519PrivateKey:
    raw = keyring.get_password(SERVICE, kid)
    if not raw:
        sys.exit(f"no signing key '{kid}' in the OS keyring ({SERVICE}); run 'keygen' first")
    return Ed25519PrivateKey.from_private_bytes(bytes.fromhex(raw))


def public_b64(key: Ed25519PrivateKey) -> str:
    return base64.b64encode(key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)).decode("ascii")


def add_months(d: dt.date, months: int) -> dt.date:
    m = d.month - 1 + months
    y, m = d.year + m // 12, m % 12 + 1
    return dt.date(y, m, min(d.day, calendar.monthrange(y, m)[1]))


def cmd_keygen(a: argparse.Namespace) -> None:
    if keyring.get_password(SERVICE, a.kid):
        sys.exit(f"a signing key '{a.kid}' already exists; refusing to replace it "
                 "(licenses signed with it would stop being re-issuable). Use --kid for a new one.")
    key = Ed25519PrivateKey.generate()
    from cryptography.hazmat.primitives.serialization import NoEncryption, PrivateFormat

    raw = key.private_bytes(Encoding.Raw, PrivateFormat.Raw, NoEncryption())
    keyring.set_password(SERVICE, a.kid, raw.hex())
    print(f"stored signing key '{a.kid}' in the OS keyring ({SERVICE}).")
    print("Add this line to TRUSTED_KEYS in src-tauri/src/license.rs:")
    print(f'    ("{a.kid}", "{public_b64(key)}"),')


def cmd_pubkey(a: argparse.Namespace) -> None:
    print(f'("{a.kid}", "{public_b64(load_private(a.kid))}")')


def cmd_issue(a: argparse.Namespace) -> None:
    today = dt.date.today()
    expires = None
    if a.expires:
        expires = dt.date.fromisoformat(a.expires)
    elif a.months:
        expires = add_months(today, a.months)
    if a.tier == "byok" and expires is None:
        sys.exit("byok is a subscription: give --months or --expires")
    if expires is not None and expires <= today:
        sys.exit("expiry must be in the future")
    payload = {
        "v": 1,
        "id": str(uuid.uuid4()),
        "kid": a.kid,
        "tier": a.tier,
        "licensee": a.licensee.strip(),
        "email": (a.email or "").strip() or None,
        "issued": today.isoformat(),
        "expires": expires.isoformat() if expires else None,
        "seats": a.seats,
    }
    if not payload["licensee"]:
        sys.exit("--licensee is required")
    body = b64u(json.dumps(payload, separators=(",", ":"), sort_keys=True).encode("utf-8"))
    sig = load_private(a.kid).sign(DOMAIN + body.encode("ascii"))
    print(f"{PREFIX}.{body}.{b64u(sig)}")


def cmd_verify(a: argparse.Namespace) -> None:
    try:
        prefix, body, sig = a.key.strip().split(".")
        assert prefix == PREFIX
        payload = json.loads(unb64u(body))
    except Exception:  # noqa: BLE001
        sys.exit("not an OMNIX license key")
    pub = Ed25519PublicKey.from_public_bytes(
        load_private(payload.get("kid", DEFAULT_KID)).public_key().public_bytes(Encoding.Raw, PublicFormat.Raw))
    try:
        pub.verify(unb64u(sig), DOMAIN + body.encode("ascii"))
    except Exception:  # noqa: BLE001
        sys.exit("signature does NOT verify")
    print(json.dumps(payload, indent=2))


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("--kid", default=DEFAULT_KID, help="signing key id (default k1)")
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("keygen", help="create a signing key in the OS keyring").set_defaults(fn=cmd_keygen)
    sub.add_parser("pubkey", help="print the public key line for license.rs").set_defaults(fn=cmd_pubkey)
    i = sub.add_parser("issue", help="sign a license key")
    i.add_argument("--tier", choices=TIERS, required=True)
    i.add_argument("--licensee", required=True)
    i.add_argument("--email")
    i.add_argument("--expires", help="YYYY-MM-DD")
    i.add_argument("--months", type=int, help="expire this many months from today")
    i.add_argument("--seats", type=int, help="enterprise seat count")
    i.set_defaults(fn=cmd_issue)
    v = sub.add_parser("verify", help="check a license key against the keyring's key")
    v.add_argument("key")
    v.set_defaults(fn=cmd_verify)
    a = p.parse_args()
    a.fn(a)


if __name__ == "__main__":
    main()
